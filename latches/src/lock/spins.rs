#![cfg(feature = "task")]

use core::{
    cell::UnsafeCell,
    hint,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    sync::atomic::{
        AtomicU8,
        Ordering::{Acquire, Relaxed, Release},
    },
};

/// An alternative to [`std::sync::Mutex`] for `no_std`.
///
/// It's based on a spinlock since it should be released as soon as possible.
///
/// It is based on the futex atomic wake/wait.
pub(crate) struct Mutex<T: ?Sized> {
    /// The status of spinlock.
    ///
    /// - 0: unlocked
    /// - 1: locked
    stat: AtomicU8,
    data: UnsafeCell<T>,
}

unsafe impl<T: ?Sized + Send> Send for Mutex<T> {}
unsafe impl<T: ?Sized + Send> Sync for Mutex<T> {}

/// A locked guard.
#[must_use]
pub(crate) struct MutexGuard<'a, T: ?Sized> {
    lock: &'a Mutex<T>,
    _mrk: PhantomData<*const ()>,
}

unsafe impl<T: ?Sized + Sync> Sync for MutexGuard<'_, T> {}

impl<T> Mutex<T> {
    pub(crate) const fn new(data: T) -> Self {
        Self {
            stat: AtomicU8::new(0),
            data: UnsafeCell::new(data),
        }
    }
}

impl<T: ?Sized> Mutex<T> {
    pub(crate) fn lock(&self) -> MutexGuard<'_, T> {
        if self.stat.compare_exchange(0, 1, Acquire, Relaxed).is_err() {
            self.spinlock()
        }

        MutexGuard {
            lock: self,
            _mrk: PhantomData,
        }
    }

    /// Spins the current thread until the lock is acquired.
    #[cold]
    fn spinlock(&self) {
        loop {
            hint::spin_loop();

            if self
                .stat
                .compare_exchange_weak(0, 1, Acquire, Relaxed)
                .is_ok()
            {
                break;
            }
        }
    }

    unsafe fn unlock(&self) {
        self.stat.store(0, Release);
    }
}

impl<T: ?Sized> Deref for MutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T: ?Sized> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T: ?Sized> Drop for MutexGuard<'_, T> {
    fn drop(&mut self) {
        unsafe { self.lock.unlock() }
    }
}
