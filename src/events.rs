use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Weak,
    },
};

use parking_lot::RwLock;

use crate::{Tree, TreeError};

/// An event that is emitted when the tree changes.
#[derive(Debug, Clone)]
pub enum TreeEvent {
    TreeUpdated(Arc<Tree>),
    TreePoisoned(Arc<Tree>, TreeError),
}

/// An observer that can subscribe to tree update events.
pub struct TreeObserver {
    next_id: AtomicUsize,
    listeners: RwLock<HashMap<usize, Box<dyn Fn(&TreeEvent) + Send + Sync>>>,
}

/// A subscription to a tree update event.
/// When dropped, the subscription is automatically cancelled.
pub struct Subscription {
    id: usize,
    observer: Weak<TreeObserver>,
}

impl Default for TreeObserver {
    fn default() -> Self {
        Self::new()
    }
}

impl TreeObserver {
    pub fn new() -> Self {
        Self {
            next_id: AtomicUsize::new(0),
            listeners: RwLock::new(HashMap::new()),
        }
    }

    pub fn subscribe(
        self: &Arc<Self>,
        callback: impl Fn(&TreeEvent) + Send + Sync + 'static,
    ) -> Subscription {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.listeners.write().insert(id, Box::new(callback));

        Subscription {
            id,
            observer: Arc::downgrade(self),
        }
    }

    pub fn notify(&self, event: &TreeEvent) {
        let listeners = self.listeners.read();
        for callback in listeners.values() {
            callback(event);
        }
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        if let Some(observer) = self.observer.upgrade() {
            observer.listeners.write().remove(&self.id);
        }
    }
}
