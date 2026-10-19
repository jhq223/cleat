use crate::*;
use std::sync::{Arc, Mutex};

#[test]
fn uninitialized_lookups_and_null_objects_fail_cleanly() {
    assert!(matches!(
        Class::find("Unknown"),
        Err(Error::ClassNotFound(_))
    ));
    let object = Object::default();
    assert!(matches!(object.get::<i32>("value"), Err(Error::NullObject)));
    assert!(matches!(object.set("value", 1), Err(Error::NullObject)));
    assert!(matches!(
        object.call::<()>("Run", ()),
        Err(Error::NullObject)
    ));
}

#[test]
fn managed_handles_have_pointer_abi() {
    let pointer = std::mem::size_of::<usize>();
    assert_eq!(
        std::mem::size_of::<Il2CppDictionary<i32, Object>>(),
        pointer
    );
    assert_eq!(std::mem::size_of::<Il2CppList<Object>>(), pointer);
    assert_eq!(std::mem::size_of::<Il2CppArray<i32>>(), pointer);
    assert_eq!(std::mem::size_of::<Il2CppString>(), pointer);
}

#[test]
fn unity_math_has_scalar_layout() {
    assert_eq!(std::mem::size_of::<Vector3>(), 12);
    assert_eq!(std::mem::size_of::<Vector4>(), 16);
    assert_eq!(std::mem::size_of::<Quaternion>(), 16);
    assert_eq!(std::mem::align_of::<Vector4>(), 4);
    assert_eq!(std::mem::align_of::<Quaternion>(), 4);
    assert_eq!(std::mem::size_of::<Matrix4x4>(), 64);
    assert_eq!(std::mem::align_of::<Matrix4x4>(), 4);
}

#[test]
fn managed_string_arguments_use_object_pointer() {
    let ptr = std::ptr::dangling_mut::<std::ffi::c_void>();
    let string = unsafe { Il2CppString::from_raw_ptr(ptr) };
    assert_eq!(unsafe { string.to_arg_ptr() }, ptr);
    let mut args = (123i32,).prepare().unwrap();
    assert_eq!(unsafe { *args.to_arg_ptrs()[0].cast::<i32>() }, 123);
}

#[test]
fn null_array_is_empty() {
    let array = unsafe { Il2CppArray::<i32>::from_raw(std::ptr::null_mut()) };
    assert!(array.is_empty());
    assert_eq!(array.get(0), None);
    assert!(array.to_vec().is_empty());
}

#[test]
fn coroutines_resume_on_later_frames() {
    let scheduler = CoroutineScheduler::new();
    let events = Arc::new(Mutex::new(Vec::new()));
    let captured = events.clone();
    let task = scheduler.spawn(async move {
        captured.lock().unwrap().push(1);
        coroutine::next_frame().await;
        captured.lock().unwrap().push(2);
        coroutine::wait_frames(2).await;
        captured.lock().unwrap().push(3);
        Ok(())
    });
    assert!(events.lock().unwrap().is_empty());
    scheduler.tick().unwrap();
    assert_eq!(*events.lock().unwrap(), [1]);
    scheduler.tick().unwrap();
    assert_eq!(*events.lock().unwrap(), [1, 2]);
    scheduler.tick().unwrap();
    assert_eq!(*events.lock().unwrap(), [1, 2]);
    scheduler.tick().unwrap();
    assert_eq!(*events.lock().unwrap(), [1, 2, 3]);
    assert_eq!(task.status(), CoroutineStatus::Completed);
}

#[test]
fn cancellation_drops_pending_futures() {
    struct Dropped(Arc<Mutex<bool>>);
    impl Drop for Dropped {
        fn drop(&mut self) {
            *self.0.lock().unwrap() = true;
        }
    }
    let dropped = Arc::new(Mutex::new(false));
    let resource = Dropped(dropped.clone());
    let scheduler = CoroutineScheduler::new();
    let task = scheduler.spawn(async move {
        let _resource = resource;
        std::future::pending::<()>().await;
        Ok(())
    });
    scheduler.tick().unwrap();
    task.cancel();
    scheduler.tick().unwrap();
    assert!(*dropped.lock().unwrap());
    assert_eq!(task.status(), CoroutineStatus::Cancelled);
}

#[test]
fn failing_and_panicking_tasks_do_not_stop_other_tasks() {
    let scheduler = CoroutineScheduler::new();
    let failed = scheduler.spawn(async { Err(Error::Bridge("failure".into())) });
    let panicked = scheduler.spawn(async {
        panic!("test panic");
        #[allow(unreachable_code)]
        Ok(())
    });
    let completed = scheduler.spawn(async { Ok(()) });
    scheduler.tick().unwrap();
    assert!(matches!(failed.status(), CoroutineStatus::Failed(_)));
    assert!(matches!(panicked.status(), CoroutineStatus::Failed(_)));
    assert_eq!(completed.status(), CoroutineStatus::Completed);
}

#[test]
fn shutdown_releases_futures_even_when_scheduler_is_retained() {
    let scheduler = Arc::new(CoroutineScheduler::new());
    let resource = Arc::new(());
    let weak = Arc::downgrade(&resource);
    let task = scheduler.spawn(async move {
        let _resource = resource;
        std::future::pending::<()>().await;
        Ok(())
    });
    scheduler.tick().unwrap();
    scheduler.shutdown();
    assert!(weak.upgrade().is_none());
    assert_eq!(task.status(), CoroutineStatus::Cancelled);
    assert_eq!(
        scheduler.spawn(async { Ok(()) }).status(),
        CoroutineStatus::Cancelled
    );
    scheduler.tick().unwrap();
}

#[test]
fn shutdown_from_inside_a_task_does_not_deadlock() {
    let scheduler = Arc::new(CoroutineScheduler::new());
    let captured = scheduler.clone();
    let task = scheduler.spawn(async move {
        captured.shutdown();
        Ok(())
    });
    scheduler.tick().unwrap();
    assert_eq!(task.status(), CoroutineStatus::Cancelled);
}

#[test]
fn spawning_during_tick_and_reentrant_tick_do_not_deadlock() {
    let scheduler = Arc::new(CoroutineScheduler::new());
    let inner = scheduler.clone();
    let result = Arc::new(Mutex::new(None));
    let captured = result.clone();
    scheduler.spawn(async move {
        *captured.lock().unwrap() = Some(inner.spawn(async { Ok(()) }));
        inner.tick()?;
        Ok(())
    });
    scheduler.tick().unwrap();
    assert_eq!(
        result.lock().unwrap().as_ref().unwrap().status(),
        CoroutineStatus::Pending
    );
    scheduler.tick().unwrap();
    assert_eq!(
        result.lock().unwrap().as_ref().unwrap().status(),
        CoroutineStatus::Completed
    );
}

#[test]
fn ticking_from_another_thread_is_rejected() {
    let scheduler = Arc::new(CoroutineScheduler::new());
    scheduler.tick().unwrap();
    assert!(
        std::thread::spawn(move || scheduler.tick().is_err())
            .join()
            .unwrap()
    );
}

#[test]
fn cancel_all_from_inside_a_task_cancels_the_current_batch() {
    let scheduler = Arc::new(CoroutineScheduler::new());
    let captured = scheduler.clone();
    let first = scheduler.spawn(async move {
        captured.cancel_all();
        Ok(())
    });
    let second = scheduler.spawn(async {
        panic!("cancelled task must not run");
        #[allow(unreachable_code)]
        Ok(())
    });
    scheduler.tick().unwrap();
    assert_eq!(first.status(), CoroutineStatus::Cancelled);
    assert_eq!(second.status(), CoroutineStatus::Cancelled);
    let next = scheduler.spawn(async { Ok(()) });
    scheduler.tick().unwrap();
    assert_eq!(next.status(), CoroutineStatus::Completed);
}

#[cfg(not(target_os = "android"))]
#[test]
fn unsupported_hooks_never_report_success() {
    assert!(matches!(
        unsafe { HookGuard::install(std::ptr::null(), std::ptr::null()) },
        Err(Error::UnsupportedPlatform(_))
    ));
    let slot = HookSlot::new();
    assert!(slot.with_original(|_| ()).is_err());
    slot.uninstall().unwrap();
    assert!(!slot.is_installed());
}
