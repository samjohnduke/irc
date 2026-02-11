//! RwLock extension for error propagation.

use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::error::{Error, Result};

/// Extension trait for RwLock that converts poison errors to Result.
pub trait RwLockExt<T> {
    /// Acquire a read lock, returning an error if the lock is poisoned.
    fn read_lock(&self, context: &str) -> Result<RwLockReadGuard<'_, T>>;

    /// Acquire a write lock, returning an error if the lock is poisoned.
    fn write_lock(&self, context: &str) -> Result<RwLockWriteGuard<'_, T>>;
}

impl<T> RwLockExt<T> for RwLock<T> {
    fn read_lock(&self, context: &str) -> Result<RwLockReadGuard<'_, T>> {
        self.read()
            .map_err(|_| Error::LockPoisoned(context.to_string()))
    }

    fn write_lock(&self, context: &str) -> Result<RwLockWriteGuard<'_, T>> {
        self.write()
            .map_err(|_| Error::LockPoisoned(context.to_string()))
    }
}
