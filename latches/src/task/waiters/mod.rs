#[cfg(feature = "std")]
use std::{mem, task::Waker};

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
#[cfg(not(feature = "std"))]
use core::{mem, task::Waker};

use crate::lock::Mutex;

#[cfg(test)]
mod tests;

/// A lightweight and [`Waker`]-optimized version of [`Slab`].
///
/// - It has only `upsert`, `remove` and `wake` operation.
/// - The `wake` will wake and remove all wakers in the collection.
/// - It does not clone [`Waker`] when `upsert` is not required.
/// - It requires a mutable [`Option<usize>`] identifier refernce for all
///   operations, the identifier should be managed correctly externally.
/// - It will not panic when removing from a non-existent key.
///
/// [`Slab`]: https://crates.io/crates/slab
pub(super) struct Waiters {
    data: Vec<Waiter>,
    next: usize,
}

enum Waiter {
    Occupied(Waker),
    Vacant(usize),
}

impl Waiters {
    pub(super) const fn new() -> Self {
        Self {
            data: Vec::new(),
            next: 0,
        }
    }

    pub(super) fn upsert(&mut self, id: &mut Option<usize>, waker: &Waker) {
        match *id {
            Some(key) => match self.data.get_mut(key) {
                Some(Waiter::Occupied(w)) => {
                    if !w.will_wake(waker) {
                        *w = waker.clone()
                    }
                }
                // It will be ensured by crate::task::Latch
                _ => unreachable!("update non-existent waker"),
            },
            None => {
                let key = self.next;

                if self.data.len() == key {
                    self.data.push(Waiter::Occupied(waker.clone()));
                    self.next = key + 1;
                } else if let Some(&Waiter::Vacant(n)) = self.data.get(key) {
                    self.data[key] = Waiter::Occupied(waker.clone());
                    self.next = n;
                } else {
                    unreachable!();
                }

                *id = Some(key);
            }
        }
    }

    pub(super) fn remove(&mut self, id: &mut Option<usize>) {
        if let Some(key) = id.take() {
            if let Some(waiter) = self.data.get_mut(key) {
                if let Waiter::Occupied(_) = waiter {
                    *waiter = Waiter::Vacant(self.next);
                    self.next = key;
                }
            }
        }
    }

    /// Wakes up all wakers and empty the collection.
    ///
    /// Wake up after releasing the lock, minimizing lock-in effects.
    pub(super) fn wake_all(mutex: &Mutex<Self>) {
        let mut lock = mutex.lock();
        let waiters = lock.take();

        drop(lock);

        for waiter in waiters {
            if let Waiter::Occupied(w) = waiter {
                w.wake();
            }
        }
    }

    fn take(&mut self) -> Vec<Waiter> {
        self.next = 0;

        mem::take(&mut self.data)
    }
}
