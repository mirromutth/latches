#[cfg(feature = "std")]
use std::{
    fmt, hint,
    sync::atomic::{
        AtomicU32,
        Ordering::{Acquire, Relaxed, Release},
    },
};

#[cfg(not(feature = "std"))]
use core::{
    fmt, hint,
    sync::atomic::{
        AtomicU32,
        Ordering::{Acquire, Relaxed, Release},
    },
};

use crate::macros;

#[cfg(test)]
mod tests;

/// A latch is a downward counter which can be used to synchronize threads. The
/// value of the counter is initialized on creation. Threads may block on the
/// latch until the counter is decremented to 0.
///
/// In contrast to [`Barrier`], it is a one-shot phenomenon, that mean the
/// counter will not be reset after reaching 0. However, it has a useful
/// property in that it does not make threads wait for the counter to reach 0
/// by calling [`count_down()`] or [`arrive()`].
///
/// It spins before each futex wait.
///
/// It is based on atomic futex, so:
///
/// - It doesn't support timeout for wait. Because MacOS doesn't yet have a
///   stable ABI for futex wait timeout.
/// - It only supports `u32` because the futex support only 32-bits. Of course,
///   we could make two atomic integers, an atomic `usize` for the counter and
///   an atomic `u32` for the futex wait/wake. See the `sync` implementation
///   with `atomic-wait` feature.
/// - Before each futex wait, it spins to wait.
///
/// From the above we can see how similar this latch implementation is to
/// popular `std::latch` implementations in C++. Therefore any C++ programmer
/// can smoothly migrate from `std::latch` to this latch implementation.
///
/// Note: the ordering of the futex wait/wake corresponds to [`SeqCst`].
///
/// # Examples
///
/// Created by `1` can be used as a simple gate, all threads calling [`wait()`]
/// will be blocked until a thread calls [`count_down()`].
///
/// Created by `N` can be used to make one or more threads wait until `N`
/// operations have completed, or an operation has completed 'N' times.
///
/// [`Barrier`]: std::sync::Barrier
/// [`SeqCst`]: std::sync::atomic::Ordering::SeqCst
/// [`arrive()`]: Latch::arrive
/// [`count_down()`]: Latch::count_down
/// [`wait()`]: Latch::wait
///
/// ```
/// // The std for example, the futex implementation can be used in no-std.
/// use std::{
///     sync::{
///         atomic::{AtomicU32, Ordering},
///         Arc, RwLock,
///     },
///     thread,
/// };
///
/// use latches::futex::Latch;
///
/// let init_gate = Arc::new(Latch::new(1));
/// let operation = Arc::new(Latch::new(30));
/// let results = Arc::new(RwLock::new(Vec::<AtomicU32>::new()));
///
/// for i in 0..10 {
///     let gate = init_gate.clone();
///     let part = operation.clone();
///     let res = results.clone();
///
///     // Each thread need to process 3 operations
///     thread::spawn(move || {
///         gate.wait();
///
///         let db = res.read().unwrap();
///         for j in 0..3 {
///             db[i * 3 + j].store((i * 3 + j) as u32, Ordering::Relaxed);
///             part.count_down();
///         }
///     });
/// }
///
/// let res = results.clone();
/// thread::spawn(move || {
///     // Init some statuses, e.g. DB, File System, etc.
///     let mut db = res.write().unwrap();
///     for _ in 0..30 {
///         db.push(AtomicU32::new(0));
///     }
///     init_gate.count_down();
/// });
///
/// // All 30 operations will be done after this line
/// // Or use operation.wait_timeout(Duration) to set the timeout
/// operation.wait();
///
/// let res: Vec<_> = results.read()
///     .unwrap()
///     .iter()
///     .map(|i| i.load(Ordering::Relaxed))
///     .collect();
/// assert_eq!(res, Vec::from_iter(0..30));
/// ```
#[repr(transparent)]
pub struct Latch {
    stat: AtomicU32,
}

impl Latch {
    /// Creates a new latch initialized with the given count.
    ///
    /// # Examples
    ///
    /// ```
    /// use latches::futex::Latch;
    ///
    /// let latch = Latch::new(10);
    /// # drop(latch);
    /// ```
    #[must_use]
    #[inline]
    pub const fn new(count: u32) -> Self {
        Self {
            stat: AtomicU32::new(count),
        }
    }

    /// Decrements the latch count, re-enable all waiting threads if the
    /// counter reaches 0 after decrement.
    ///
    /// - If the counter has reached 0 then do nothing.
    /// - If the current count is greater than 0 then it is decremented.
    /// - If the new count is 0 then all waiting threads are re-enabled.
    ///
    /// # Examples
    ///
    /// ```
    /// use latches::futex::Latch;
    ///
    /// let latch = Latch::new(1);
    /// latch.count_down();
    /// ```
    pub fn count_down(&self) {
        macros::decrement!(self, 1);
    }

    /// Decrements the latch count by `n`, re-enable all waiting threads if the
    /// counter reaches 0 after decrement.
    ///
    /// It will not cause an overflow by decrement the counter.
    ///
    /// - If the `n` is 0 or the counter has reached 0 then do nothing.
    /// - If the current count is greater than `n` then decremented by `n`.
    /// - If the current count is greater than 0 and less than or equal to `n`,
    ///   then the new count will be 0, and all waiting threads are re-enabled.
    ///
    /// # Examples
    ///
    /// ```
    /// use latches::futex::Latch;
    ///
    /// let latch = Latch::new(10);
    ///
    /// // Do a batch upsert SQL and return `updatedRows` = 10 in runtime.
    /// # let updatedRows = 10;
    /// latch.arrive(updatedRows);
    /// assert_eq!(latch.count(), 0);
    /// ```
    pub fn arrive(&self, n: u32) {
        if n == 0 {
            return;
        }

        macros::decrement!(self, n);
    }

    /// Acquires the current count.
    ///
    /// It is typically used for debugging and testing.
    ///
    /// # Examples
    ///
    /// ```
    /// use latches::futex::Latch;
    ///
    /// let latch = Latch::new(3);
    /// assert_eq!(latch.count(), 3);
    /// ```
    #[must_use]
    #[inline]
    pub fn count(&self) -> u32 {
        self.stat.load(Acquire)
    }

    /// Checks that the current counter has reached 0.
    ///
    /// # Errors
    ///
    /// This function will return an error with the current count if the
    /// counter has not reached 0.
    ///
    /// # Examples
    ///
    /// ```
    /// use latches::futex::Latch;
    ///
    /// let latch = Latch::new(1);
    /// assert_eq!(latch.try_wait(), Err(1));
    /// latch.count_down();
    /// assert_eq!(latch.try_wait(), Ok(()));
    /// ```
    #[inline]
    pub fn try_wait(&self) -> Result<(), u32> {
        macros::once_try_wait!(self)
    }

    /// Blocks the current thread to wait until the current counter reaches 0.
    ///
    /// - If the current count is 0 then return immediately.
    /// - If the current count is greater than 0 then block the current thread
    ///   until awakened by a [`count_down()`]/[`arrive()`] invocation which
    ///   causes the counter reaches 0.
    ///
    /// [`count_down()`]: Latch::count_down
    /// [`arrive()`]: Latch::arrive
    ///
    /// # Examples
    ///
    /// ```
    /// use std::{sync::Arc, thread};
    ///
    /// use latches::futex::Latch;
    ///
    /// let latch = Arc::new(Latch::new(1));
    /// let l1 = latch.clone();
    ///
    /// thread::spawn(move || l1.count_down());
    /// latch.wait();
    /// ```
    pub fn wait(&self) {
        loop {
            let s = self.spin();

            if s == 0 {
                break;
            }

            atomic_wait::wait(&self.stat, s);
        }
    }

    fn spin(&self) -> u32 {
        macros::spin_try_wait!(self, s, 0, s);
    }

    #[cold]
    fn done(&self) {
        atomic_wait::wake_all(&self.stat);
    }
}

impl fmt::Debug for Latch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Latch")
            .field("count", &self.stat)
            .finish_non_exhaustive()
    }
}
