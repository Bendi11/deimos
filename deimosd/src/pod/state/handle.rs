use std::ops::Deref;

use tokio::sync::Mutex;

use super::{PodState, PodStateKnown};



/// Pods state handles are responsible for maintaining a number of invariants across the server.
/// - All pod state mutations must be sent to subscribers
/// - No more than one task may perform mutating operations on a pod's state at a time
pub struct PodStateHandle {
    lock: Mutex<PodStateKnown>,
    tx: tokio::sync::watch::Sender<PodState>,
}

/// A handle allowing mutations to the state of a [Pod].
/// Contains a channel that will broadcast pod state changes to subscribers when they occur.
pub struct PodStateWriteHandle<'a> {
    lock: tokio::sync::MutexGuard<'a, PodStateKnown>,
    tx: tokio::sync::watch::Sender<PodState>,
}

/// A handle that ensures the pod's state will not be changed while held, but does not allow
/// mutations to the state and thus cannot notify status tasks of changes
/// Can be upgraded to a [PodStateWriteHandle] to support patterns of use where we need to
/// inspect the state and optionally perform operations based on the current state.
pub struct PodStateReadHandle<'a>(tokio::sync::MutexGuard<'a, PodStateKnown>);

impl PodStateHandle {
    /// Create a new state handle with the given initial state
    pub fn new(state: PodStateKnown) -> Self {
        let (tx, _) = tokio::sync::watch::channel(PodState::from(&state));
        let lock = Mutex::new(state);

        Self { lock, tx }
    }
    
    /// Subscribe to a stream of pod state changes
    pub fn subscribe(&self) -> tokio_stream::wrappers::WatchStream<PodState> {
        tokio_stream::wrappers::WatchStream::new(self.tx.subscribe())
    }

    /// Lock the handle to allow mutations to the current state
    pub async fn transact(&self) -> PodStateWriteHandle {
        let lock = self.lock.lock().await;
        self.tx.send_replace(PodState::Transit);

        PodStateWriteHandle {
            lock,
            tx: self.tx.clone(),
        }
    }
    
    /// Wait for mutations to the state to finish and return a read-only lock for the state
    pub async fn read(&self) -> PodStateReadHandle {
        PodStateReadHandle(self.lock.lock().await)
    }
    
    /// Upgrade a pod 
    pub fn upgrade<'a>(&self, read: PodStateReadHandle<'a>) -> PodStateWriteHandle<'a> {
        PodStateWriteHandle {
            lock: read.0,
            tx: self.tx.clone(),
        }
    }
    
    /// Wait for all writers to finish and return the most current state of the pod
    pub async fn wait(&self) -> PodStateKnown {
        let lock = self.lock.lock().await;
        lock.clone()
    }

    /// Get the current state
    pub fn current(&self) -> PodState {
        self.lock
            .try_lock()
            .as_deref()
            .map(Into::into)
            .unwrap_or(PodState::Transit)
    }
}

impl<'a> PodStateWriteHandle<'a> {
    /// Get an immutable reference to the current state
    pub fn state(&self) -> &PodStateKnown {
        &self.lock
    }

    /// Set the current state to the given value
    pub fn set(&mut self, state: PodStateKnown) {
        self.tx.send_replace((&state).into());
        *self.lock = state;
    }
}

impl<'a> Deref for PodStateReadHandle<'a> {
    type Target = PodStateKnown;
    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

impl Drop for PodStateWriteHandle<'_> {
    fn drop(&mut self) {
        if *self.tx.borrow() == PodState::Transit {
            let _ = self.tx.send(PodState::from(&*self.lock));
        }
    }
}

