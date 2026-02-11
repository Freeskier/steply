use std::sync::{Arc, Mutex};

use super::state::FileBrowserState;

#[derive(Clone)]
pub(crate) struct SharedFileBrowserState {
    inner: Arc<Mutex<FileBrowserState>>,
}

impl SharedFileBrowserState {
    pub(crate) fn new(inner: Arc<Mutex<FileBrowserState>>) -> Self {
        Self { inner }
    }

    pub(crate) fn arc(&self) -> Arc<Mutex<FileBrowserState>> {
        Arc::clone(&self.inner)
    }

    pub(crate) fn with<R>(&self, f: impl FnOnce(&FileBrowserState) -> R) -> R {
        let guard = match self.inner.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        f(&guard)
    }

    pub(crate) fn with_mut<R>(&self, f: impl FnOnce(&mut FileBrowserState) -> R) -> R {
        let mut guard = match self.inner.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        f(&mut guard)
    }
}
