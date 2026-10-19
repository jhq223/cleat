//! A native IEnumerator drives a genawaiter coroutine using Unity's own scheduler.
use super::{ClassBuilder, InjectionRuntime};
use crate::{Class, CoroutineStatus, Error, GcHandle, ManagedCoroutine, Object, Result};
use genawaiter::{
    GeneratorState,
    sync::{Co, Gen},
};
use std::{
    collections::HashMap,
    future::Future,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

pub enum YieldInstruction {
    NextFrame,
    EndOfFrame,
    FixedUpdate,
    Seconds(f32),
    Realtime(Duration),
    /// An existing yield instruction, AsyncOperation, Coroutine, or IEnumerator.
    Managed(GcHandle),
}
impl YieldInstruction {
    pub fn managed(object: Object) -> Result<Self> {
        Ok(Self::Managed(GcHandle::new(object)?))
    }
    fn into_object(self) -> Result<Option<GcHandle>> {
        let object = match self {
            Self::NextFrame => return Ok(None),
            Self::Managed(root) => return Ok(Some(root)),
            Self::EndOfFrame => Class::find("UnityEngine.WaitForEndOfFrame")?.construct(())?,
            Self::FixedUpdate => Class::find("UnityEngine.WaitForFixedUpdate")?.construct(())?,
            Self::Seconds(seconds) => {
                if !seconds.is_finite() || seconds < 0.0 {
                    return Err(Error::Bridge(
                        "seconds must be finite and non-negative".into(),
                    ));
                }
                Class::find("UnityEngine.WaitForSeconds")?.construct((seconds,))?
            }
            Self::Realtime(duration) => {
                let seconds = duration.as_secs_f32();
                if !seconds.is_finite() {
                    return Err(Error::Bridge("duration is too large".into()));
                }
                Class::find("UnityEngine.WaitForSecondsRealtime")?.construct((seconds,))?
            }
        };
        Ok(Some(GcHandle::new(object)?))
    }
}

/// Await these methods inside UnityCoroutine::start. They yield to Unity's scheduler.
pub struct YieldContext(Co<YieldInstruction>);
impl YieldContext {
    pub async fn yield_instruction(&self, instruction: YieldInstruction) {
        self.0.yield_(instruction).await;
    }
    pub async fn next_frame(&self) {
        self.yield_instruction(YieldInstruction::NextFrame).await;
    }
    pub async fn end_of_frame(&self) {
        self.yield_instruction(YieldInstruction::EndOfFrame).await;
    }
    pub async fn fixed_update(&self) {
        self.yield_instruction(YieldInstruction::FixedUpdate).await;
    }
    pub async fn seconds(&self, seconds: f32) {
        self.yield_instruction(YieldInstruction::Seconds(seconds))
            .await;
    }
    pub async fn realtime(&self, duration: Duration) {
        self.yield_instruction(YieldInstruction::Realtime(duration))
            .await;
    }
    pub async fn managed(&self, object: Object) -> Result<()> {
        self.yield_instruction(YieldInstruction::managed(object)?)
            .await;
        Ok(())
    }
    pub async fn until(&self, mut condition: impl FnMut() -> Result<bool>) -> Result<()> {
        while !condition()? {
            self.next_frame().await;
        }
        Ok(())
    }
    pub async fn while_true(&self, mut condition: impl FnMut() -> Result<bool>) -> Result<()> {
        self.until(move || condition().map(|value| !value)).await
    }
}

struct State {
    iterator: Mutex<Option<Box<dyn Iterator<Item = Result<YieldInstruction>> + Send>>>,
    status: Mutex<CoroutineStatus>,
}
static STATES: OnceLock<Mutex<HashMap<usize, Arc<State>>>> = OnceLock::new();
static NEXT: AtomicUsize = AtomicUsize::new(1);
static ENUMERATOR_CLASS: OnceLock<Class> = OnceLock::new();
fn states() -> &'static Mutex<HashMap<usize, Arc<State>>> {
    STATES.get_or_init(Default::default)
}

/// A Rust coroutine represented as a real managed IEnumerator.
/// Unity controls Update/FixedUpdate/end-of-frame timing. Call stop to cancel;
/// dropping the handle leaves it running, like MonoBehaviour.StartCoroutine.
pub struct UnityCoroutine {
    coroutine: ManagedCoroutine,
    enumerator: GcHandle,
    state: Arc<State>,
}
impl UnityCoroutine {
    pub fn start<F: Future<Output = Result<()>> + Send + 'static>(
        runtime: &InjectionRuntime,
        owner: Object,
        producer: impl FnOnce(YieldContext) -> F,
    ) -> Result<Self> {
        let mut generator = Gen::new(|co| producer(YieldContext(co)));
        let mut done = false;
        Self::from_iterator(
            runtime,
            owner,
            std::iter::from_fn(move || {
                if done {
                    return None;
                }
                match generator.resume() {
                    GeneratorState::Yielded(instruction) => Some(Ok(instruction)),
                    GeneratorState::Complete(result) => {
                        done = true;
                        result.err().map(Err)
                    }
                }
            }),
        )
    }
    pub fn from_iterator(
        runtime: &InjectionRuntime,
        owner: Object,
        iterator: impl Iterator<Item = Result<YieldInstruction>> + Send + 'static,
    ) -> Result<Self> {
        let class = enumerator_class(runtime)?;
        let object = class.new_object()?;
        let enumerator = GcHandle::new(object)?;
        let state = Arc::new(State {
            iterator: Mutex::new(Some(Box::new(iterator))),
            status: Mutex::new(CoroutineStatus::Pending),
        });
        let id = NEXT
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .map_err(|_| Error::Bridge("coroutine identifiers exhausted".into()))?;
        object.set("stateId", id)?;
        states()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id, state.clone());
        match ManagedCoroutine::start(owner, object) {
            Ok(coroutine) => Ok(Self {
                coroutine,
                enumerator,
                state,
            }),
            Err(error) => {
                release(&object, CoroutineStatus::Failed(error.to_string()))?;
                Err(error)
            }
        }
    }
    pub fn status(&self) -> CoroutineStatus {
        self.state
            .status
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
    pub fn is_finished(&self) -> bool {
        self.status() != CoroutineStatus::Pending
    }
    pub fn as_object(&self) -> Object {
        self.coroutine.as_object()
    }
    pub fn stop(self) -> Result<()> {
        self.coroutine.stop()?;
        release(&self.enumerator.object(), CoroutineStatus::Cancelled)
    }
}

fn enumerator_class(runtime: &InjectionRuntime) -> Result<Class> {
    if let Some(class) = ENUMERATOR_CLASS.get() {
        return Ok(class.clone());
    }
    let object = Class::find("System.Object")?;
    let class = ClassBuilder::new(object.assembly_name(), "Cleat.Runtime.Enumerator", &object)
        .implements(&Class::find("System.Collections.IEnumerator")?)
        .implements(&Class::find("System.IDisposable")?)
        .field::<usize>("stateId")?
        .field::<Object>("current")?
        .method(move_next::definition()?)
        .method(current::definition()?)
        .method(reset::definition()?)
        .method(dispose::definition()?)
        .method(finalize::definition()?)
        .register(runtime)?;
    let _ = ENUMERATOR_CLASS.set(class.clone());
    Ok(class)
}
fn release(this: &Object, status: CoroutineStatus) -> Result<()> {
    let id: usize = this.get("stateId")?;
    let state = states()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&id);
    if let Some(state) = state {
        let mut current = state.status.lock().unwrap_or_else(|e| e.into_inner());
        if *current == CoroutineStatus::Pending {
            *current = status;
        }
        drop(current);
        if let Ok(mut iterator) = state.iterator.try_lock() {
            iterator.take();
        }
    }
    this.set("stateId", 0usize)?;
    this.set("current", Object::default())
}

#[cleat::managed_method("MoveNext")]
fn move_next(this: &Object) -> Result<bool> {
    let id: usize = this.get("stateId")?;
    let state = states()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&id)
        .cloned();
    let Some(state) = state else {
        return Ok(false);
    };
    let next = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
        || -> Result<Option<YieldInstruction>> {
            let mut iterator = state
                .iterator
                .try_lock()
                .map_err(|_| Error::Bridge("IEnumerator.MoveNext was reentered".into()))?;
            iterator
                .as_mut()
                .and_then(|iterator| iterator.next())
                .transpose()
        },
    ));
    let next = match next {
        Ok(next) => next,
        Err(_) => Err(Error::Bridge("Unity coroutine panicked".into())),
    };
    if *state.status.lock().unwrap_or_else(|e| e.into_inner()) != CoroutineStatus::Pending {
        state
            .iterator
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        return Ok(false);
    }
    let next = next.and_then(|instruction| match instruction {
        Some(instruction) => {
            let root = instruction.into_object()?;
            this.set(
                "current",
                root.as_ref().map(GcHandle::object).unwrap_or_default(),
            )?;
            Ok(true)
        }
        None => Ok(false),
    });
    match next {
        Ok(true) => Ok(true),
        Ok(false) => {
            release(this, CoroutineStatus::Completed)?;
            Ok(false)
        }
        Err(error) => {
            release(this, CoroutineStatus::Failed(error.to_string()))?;
            Err(error)
        }
    }
}
#[cleat::managed_method("get_Current")]
fn current(this: &Object) -> Result<Object> {
    this.get("current")
}
#[cleat::managed_method("Reset")]
fn reset(_this: &Object) -> Result<()> {
    Err(Error::Bridge("coroutines cannot be reset".into()))
}
#[cleat::managed_method("Dispose")]
fn dispose(this: &Object) -> Result<()> {
    release(this, CoroutineStatus::Cancelled)
}
#[cleat::managed_method("Finalize")]
fn finalize(this: &Object) -> Result<()> {
    release(this, CoroutineStatus::Cancelled)
}
