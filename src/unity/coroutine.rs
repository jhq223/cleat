//! Frame-driven Rust futures and Unity-managed coroutines.
use crate::{Args, Class, Error, GcHandle, HookSlot, Object, Result};
use async_task::Runnable;
use concurrent_queue::ConcurrentQueue;
use futures_util::{
    FutureExt,
    future::{AbortHandle, Abortable},
};
use std::{
    future::Future,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context, Poll},
    time::{Duration, Instant},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoroutineStatus {
    Pending,
    Completed,
    Cancelled,
    Failed(String),
}

/// Cancellation and completion state. Dropping this handle leaves the task running.
#[derive(Clone)]
pub struct CoroutineTask {
    state: Arc<Mutex<CoroutineStatus>>,
    abort: AbortHandle,
}
impl CoroutineTask {
    pub fn cancel(&self) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if *state == CoroutineStatus::Pending {
            *state = CoroutineStatus::Cancelled;
            self.abort.abort();
        }
    }
    pub fn status(&self) -> CoroutineStatus {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
    pub fn is_finished(&self) -> bool {
        self.status() != CoroutineStatus::Pending
    }
}

/// Runs woken tasks once per frame on the Unity thread.
///
/// Task ownership and wakers use async-task; cancellation and panic isolation
/// use futures-util. Only the frame boundary is specific to Unity.
/// Use CoroutineRunner for automatic Update integration, or call tick yourself.
pub struct CoroutineScheduler {
    queue: Arc<ConcurrentQueue<Runnable>>,
    tasks: Mutex<Vec<CoroutineTask>>,
    polling: AtomicBool,
    stopped: AtomicBool,
    thread: Mutex<Option<std::thread::ThreadId>>,
}
impl Default for CoroutineScheduler {
    fn default() -> Self {
        Self {
            queue: Arc::new(ConcurrentQueue::unbounded()),
            tasks: Mutex::new(Vec::new()),
            polling: AtomicBool::new(false),
            stopped: AtomicBool::new(false),
            thread: Mutex::new(None),
        }
    }
}
impl CoroutineScheduler {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn spawn(
        &self,
        future: impl Future<Output = Result<()>> + Send + 'static,
    ) -> CoroutineTask {
        let (abort, registration) = AbortHandle::new_pair();
        let handle = CoroutineTask {
            state: Arc::new(Mutex::new(CoroutineStatus::Pending)),
            abort,
        };
        let mut tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        if self.stopped.load(Ordering::Acquire) {
            handle.cancel();
            return handle;
        }
        let state = handle.state.clone();
        let future = async move {
            let result = Abortable::new(
                std::panic::AssertUnwindSafe(future).catch_unwind(),
                registration,
            )
            .await;
            let status = match result {
                Ok(Ok(Ok(()))) => CoroutineStatus::Completed,
                Ok(Ok(Err(error))) => CoroutineStatus::Failed(error.to_string()),
                Ok(Err(_)) => CoroutineStatus::Failed("coroutine panicked".into()),
                Err(_) => CoroutineStatus::Cancelled,
            };
            let mut state = state.lock().unwrap_or_else(|e| e.into_inner());
            if *state == CoroutineStatus::Pending {
                *state = status;
            }
        };
        let queue = self.queue.clone();
        let (runnable, task) = async_task::spawn(future, move |runnable| {
            let _ = queue.push(runnable);
        });
        tasks.push(handle.clone());
        task.detach();
        runnable.schedule();
        handle
    }
    /// Run the work ready at the start of this tick. Wakes and spawns occurring
    /// during execution are deferred until the next tick, preventing busy loops.
    /// Call on Unity's main thread, once per frame. A different thread is rejected.
    pub fn tick(&self) -> Result<()> {
        let current = std::thread::current().id();
        {
            let mut thread = self.thread.lock().unwrap_or_else(|e| e.into_inner());
            if thread.is_some_and(|id| id != current) {
                return Err(Error::Bridge("coroutine scheduler changed threads".into()));
            }
            *thread = Some(current);
        }
        if self.stopped.load(Ordering::Acquire) || self.polling.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        struct PollGuard<'a>(&'a AtomicBool);
        impl Drop for PollGuard<'_> {
            fn drop(&mut self) {
                self.0.store(false, Ordering::Release);
            }
        }
        let _polling = PollGuard(&self.polling);
        let batch: Vec<_> = (0..self.queue.len())
            .filter_map(|_| self.queue.pop().ok())
            .collect();
        for runnable in batch {
            runnable.run();
        }
        self.tasks
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|task| !task.is_finished());
        Ok(())
    }
    pub fn cancel_all(&self) {
        let tasks = std::mem::take(&mut *self.tasks.lock().unwrap_or_else(|e| e.into_inner()));
        for task in tasks {
            task.cancel();
        }
    }
    /// Permanently stop accepting work and release cancelled futures.
    /// A task already being polled finishes its current poll before cancellation.
    pub fn shutdown(&self) {
        self.stopped.store(true, Ordering::Release);
        self.cancel_all();
        // These runnables observe Abortable's cancellation before polling user
        // futures. Draining also releases resources when an external Arc lives on.
        while let Ok(runnable) = self.queue.pop() {
            runnable.run();
        }
    }
}
impl Drop for CoroutineScheduler {
    fn drop(&mut self) {
        self.shutdown();
        self.queue.close();
    }
}

static UPDATE_HOOK: HookSlot = HookSlot::new();
static RUNNER: Mutex<Option<Arc<RunnerState>>> = Mutex::new(None);
struct RunnerState {
    owner: GcHandle,
    scheduler: Arc<CoroutineScheduler>,
}

/// Drives Rust coroutines after one selected MonoBehaviour's Update method.
/// Keep the runner alive. Dropping it cancels its tasks and removes its hook.
pub struct CoroutineRunner {
    state: Arc<RunnerState>,
}
impl CoroutineRunner {
    pub fn install(owner: Object) -> Result<Self> {
        let method = owner.method(("Update", 0usize))?;
        if !method.inner.return_type.matches_name("System.Void") {
            return Err(Error::Bridge("Update must return void".into()));
        }
        let mut runner = RUNNER.lock().unwrap_or_else(|e| e.into_inner());
        if runner.is_some() {
            return Err(Error::Hook(
                "a coroutine Update driver is already installed".into(),
            ));
        }
        let state = Arc::new(RunnerState {
            owner: GcHandle::pinned(owner)?,
            scheduler: Arc::new(CoroutineScheduler::new()),
        });
        unsafe {
            UPDATE_HOOK.install(method.native()?, update as *const std::ffi::c_void)?;
        }
        *runner = Some(state.clone());
        Ok(Self { state })
    }
    pub fn spawn(
        &self,
        future: impl Future<Output = Result<()>> + Send + 'static,
    ) -> CoroutineTask {
        self.state.scheduler.spawn(future)
    }
    pub fn scheduler(&self) -> Arc<CoroutineScheduler> {
        self.state.scheduler.clone()
    }
}
impl Drop for CoroutineRunner {
    fn drop(&mut self) {
        self.state.scheduler.shutdown();
        let mut runner = RUNNER.lock().unwrap_or_else(|e| e.into_inner());
        if runner
            .as_ref()
            .is_some_and(|state| Arc::ptr_eq(state, &self.state))
        {
            *runner = None;
            let _ = UPDATE_HOOK.uninstall();
        }
    }
}
unsafe extern "C" fn update(this: Object, method: *mut std::ffi::c_void) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<()> {
        let _invocation = UPDATE_HOOK.enter(method)?;
        UPDATE_HOOK.with_original(|original| {
            let call: unsafe extern "C" fn(Object, *mut std::ffi::c_void) =
                unsafe { std::mem::transmute(original.function) };
            unsafe { call(this, original.method_info) };
        })?;
        let state = RUNNER.lock().unwrap_or_else(|e| e.into_inner()).clone();
        if let Some(state) = state
            && state.owner.object() == this
        {
            state.scheduler.tick()?;
        }
        Ok(())
    }));
    match result {
        Ok(Ok(())) => {}
        Ok(Err(error)) => log::error!("coroutine Update: {error}"),
        Err(_) => log::error!("coroutine Update panicked"),
    }
}

/// Wait for the next scheduler tick (normally the next frame).
pub fn next_frame() -> WaitFrames {
    wait_frames(1)
}
pub fn wait_frames(frames: u32) -> WaitFrames {
    WaitFrames { remaining: frames }
}
pub struct WaitFrames {
    remaining: u32,
}
impl Future for WaitFrames {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if self.remaining == 0 {
            Poll::Ready(())
        } else {
            self.remaining -= 1;
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}
/// Real elapsed time; unaffected by Time.timeScale. Resumes on a later tick.
pub fn wait_realtime(duration: Duration) -> WaitRealtime {
    WaitRealtime {
        duration,
        deadline: None,
    }
}
pub struct WaitRealtime {
    duration: Duration,
    deadline: Option<Instant>,
}
impl Future for WaitRealtime {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        let now = Instant::now();
        let duration = self.duration;
        let deadline = self
            .deadline
            .get_or_insert_with(|| now.checked_add(duration).unwrap_or(now));
        if now >= *deadline {
            Poll::Ready(())
        } else {
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}
/// Wait using Unity's scaled time, like WaitForSeconds.
pub async fn wait_seconds(seconds: f32) -> Result<()> {
    if !seconds.is_finite() || seconds < 0.0 {
        return Err(Error::Bridge(
            "seconds must be finite and non-negative".into(),
        ));
    }
    let time = Class::find("UnityEngine.Time")?;
    let start: f32 = time.get("time")?;
    loop {
        next_frame().await;
        let now: f32 = time.get("time")?;
        if now - start >= seconds {
            return Ok(());
        }
    }
}
pub async fn wait_until(mut condition: impl FnMut() -> Result<bool>) -> Result<()> {
    loop {
        if condition()? {
            return Ok(());
        }
        next_frame().await;
    }
}
pub async fn wait_while(mut condition: impl FnMut() -> Result<bool>) -> Result<()> {
    wait_until(move || condition().map(|value| !value)).await
}
pub async fn wait_for_operation(operation: Object) -> Result<()> {
    let root = GcHandle::new(operation)?;
    wait_until(move || root.object().get("isDone")).await
}

/// A coroutine scheduled by Unity from an existing managed IEnumerator.
/// Start/stop must run on the Unity main thread. Drop only releases GC roots;
/// call stop explicitly to stop Unity's coroutine.
pub struct ManagedCoroutine {
    owner: GcHandle,
    coroutine: GcHandle,
    _enumerator: GcHandle,
}
impl ManagedCoroutine {
    pub fn start(owner: Object, enumerator: Object) -> Result<Self> {
        let owner = GcHandle::new(owner)?;
        let enumerator = GcHandle::new(enumerator)?;
        let coroutine: Object = owner.object().call(
            ("StartCoroutine", ["System.Collections.IEnumerator"]),
            (enumerator.object(),),
        )?;
        Ok(Self {
            owner,
            coroutine: GcHandle::new(coroutine)?,
            _enumerator: enumerator,
        })
    }
    pub fn start_method(
        owner: Object,
        method: impl crate::MethodSelector,
        args: impl Args,
    ) -> Result<Self> {
        let enumerator = owner.call(method, args)?;
        Self::start(owner, enumerator)
    }
    pub fn stop(self) -> Result<()> {
        self.owner.object().call(
            ("StopCoroutine", ["UnityEngine.Coroutine"]),
            (self.coroutine.object(),),
        )
    }
    pub fn as_object(&self) -> Object {
        self.coroutine.object()
    }
}
