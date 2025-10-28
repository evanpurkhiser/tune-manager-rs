use std::{
    collections::HashMap,
    sync::{
        Arc, RwLock,
        atomic::{AtomicU64, Ordering},
    },
};

use super::batch::{BatchId, StatusEvent};

/// Unique identifier for a registered callback
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
struct CallbackId(u64);

/// Trait for callbacks that receive batch status updates
pub trait StatusCallback: Send + Sync {
    fn on_status(&self, event: &StatusEvent);
}

// Implement StatusCallback for any closure that matches the signature
impl<F> StatusCallback for F
where
    F: Fn(&StatusEvent) + Send + Sync,
{
    fn on_status(&self, event: &StatusEvent) {
        self(event)
    }
}

/// Helper to create a callback from a closure with explicit types
pub fn callback<F>(f: F) -> impl StatusCallback + 'static
where
    F: Fn(&StatusEvent) + Send + Sync + 'static,
{
    f
}

/// Wrapper that filters callbacks to a specific batch
pub struct BatchFilteredCallback {
    batch_id: BatchId,
    callback: Arc<dyn StatusCallback>,
}

impl BatchFilteredCallback {
    pub fn new(batch_id: BatchId, callback: Arc<dyn StatusCallback>) -> Self {
        Self { batch_id, callback }
    }
}

impl StatusCallback for BatchFilteredCallback {
    fn on_status(&self, event: &StatusEvent) {
        let event_batch_id = match event {
            StatusEvent::TrackStageUpdate { batch, .. } => &batch.id,
            StatusEvent::BatchCompleted { batch } => &batch.id,
        };

        if event_batch_id == &self.batch_id {
            self.callback.on_status(event);
        }
    }
}

/// Registry for managing status callbacks
pub struct CallbackRegistry {
    callbacks: Arc<RwLock<HashMap<CallbackId, Arc<dyn StatusCallback>>>>,
    next_id: AtomicU64,
}

impl CallbackRegistry {
    pub fn new() -> Self {
        Self {
            callbacks: Arc::new(RwLock::new(HashMap::new())),
            next_id: AtomicU64::new(0),
        }
    }

    /// Register a new callback, returning a handle that unregisters on drop
    pub fn register<C: StatusCallback + 'static>(&self, callback: C) -> CallbackHandle {
        let id = CallbackId(self.next_id.fetch_add(1, Ordering::Relaxed));
        let callback: Arc<dyn StatusCallback> = Arc::new(callback);
        self.callbacks.write().unwrap().insert(id, callback);

        CallbackHandle {
            id,
            registry_callbacks: self.callbacks.clone(),
        }
    }

    pub fn invoke_all(&self, event: &StatusEvent) {
        self.callbacks
            .read()
            .unwrap()
            .values()
            .cloned()
            .for_each(|cb| cb.on_status(event));
    }
}

/// Handle to a registered callback that unregisters when dropped
pub struct CallbackHandle {
    id: CallbackId,
    registry_callbacks: Arc<RwLock<HashMap<CallbackId, Arc<dyn StatusCallback>>>>,
}

impl Drop for CallbackHandle {
    fn drop(&mut self) {
        self.registry_callbacks.write().unwrap().remove(&self.id);
    }
}
