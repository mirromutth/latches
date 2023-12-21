use core::{
    fmt, hint,
    sync::atomic::{
        AtomicUsize,
        Ordering::{Acquire, Relaxed, Release},
    },
};

#[cfg(feature = "std")]
use std::time::{Duration, Instant};

#[cfg(all(not(feature = "std"), not(feature = "atomic-wait")))]
compile_error!("`sync` requires `std` or `atomic-wait` feature for Condvar");

#[cfg(feature = "std")]
use crate::{lock::EmptyCondvar, macros, WaitTimeoutResult};

#[cfg(not(feature = "std"))]
use crate::{lock::EmptyCondvar, macros};

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
/// It spins before locking wait.
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
/// [`arrive()`]: Latch::arrive
/// [`count_down()`]: Latch::count_down
/// [`wait()`]: Latch::wait
///
/// ```
/// use std::{
///     sync::{
///         atomic::{AtomicU32, Ordering},
///         Arc, RwLock,
///     },
///     thread,
/// };
///
/// use latches::sync::Latch;
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
pub struct Latch {
    stat: AtomicUsize,
    cvar: EmptyCondvar,
}

impl Latch {
    /// Creates a new latch initialized with the given count.
    ///
    /// # Examples
    ///
    /// ```
    /// use latches::sync::Latch;
    ///
    /// let latch = Latch::new(10);
    /// # drop(latch);
    /// ```
    #[cfg(not(latches_no_const_sync))]
    #[must_use]
    #[inline]
    pub const fn new(count: usize) -> Self {
        Self {
            stat: AtomicUsize::new(count),
            cvar: EmptyCondvar::new(),
        }
    }
    /// Creates a new latch initialized with the given count.
    #[cfg(latches_no_const_sync)]
    #[must_use]
    #[inline]
    pub fn new(count: usize) -> Self {
        Self {
            stat: AtomicUsize::new(count),
            lock: Mutex::new(()),
            cvar: Condvar::new(),
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
    /// use latches::sync::Latch;
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
    /// use latches::sync::Latch;
    ///
    /// let latch = Latch::new(10);
    ///
    /// // Do a batch upsert SQL and return `updatedRows` = 10 in runtime.
    /// # let updatedRows = 10;
    /// latch.arrive(updatedRows);
    /// assert_eq!(latch.count(), 0);
    /// ```
    pub fn arrive(&self, n: usize) {
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
    /// use latches::sync::Latch;
    ///
    /// let latch = Latch::new(3);
    /// assert_eq!(latch.count(), 3);
    /// ```
    #[must_use]
    #[inline]
    pub fn count(&self) -> usize {
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
    /// use latches::sync::Latch;
    ///
    /// let latch = Latch::new(1);
    /// assert_eq!(latch.try_wait(), Err(1));
    /// latch.count_down();
    /// assert_eq!(latch.try_wait(), Ok(()));
    /// ```
    #[inline]
    pub fn try_wait(&self) -> Result<(), usize> {
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
    /// use latches::sync::Latch;
    ///
    /// let latch = Arc::new(Latch::new(1));
    /// let l1 = latch.clone();
    ///
    /// thread::spawn(move || l1.count_down());
    /// latch.wait();
    /// ```
    pub fn wait(&self) {
        if self.spin() {
            return;
        }

        let mut m = self.cvar.monitor();

        // A double check must be done before each waiting.
        // Do not spin again after acquiring the lock.
        while self.stat.load(Acquire) != 0 {
            m = self.cvar.wait(m);
        }
    }

    /// Blocks the current thread to wait until the current counter reaches 0
    /// or timed out.
    ///
    /// - If the current count is 0 then return [`Reached`] immediately.
    /// - If it has timed out then return [`TimedOut(())`] immediately.
    /// - If the current count is greater than 0 then block the current thread
    ///   until awakened by a [`count_down()`]/[`arrive()`] invocation that
    ///   causes the counter reaches 0, or awakened by timed out.
    ///
    /// [`Reached`]: WaitTimeoutResult::Reached
    /// [`TimedOut(())`]: WaitTimeoutResult::TimedOut
    /// [`count_down()`]: Latch::count_down
    /// [`arrive()`]: Latch::arrive
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// use latches::sync::Latch;
    ///
    /// let latch = Latch::new(1);
    ///
    /// // No-one count it down, so it will be timed out
    /// assert!(latch.wait_timeout(Duration::from_millis(10)).is_timed_out());
    /// ```
    #[cfg(feature = "std")]
    pub fn wait_timeout(&self, dur: Duration) -> WaitTimeoutResult<()> {
        if self.spin() {
            return WaitTimeoutResult::Reached;
        }

        let start = Instant::now();
        let mut m = self.cvar.monitor();

        loop {
            // A double check must be done before each waiting.
            // Do not spin again after acquiring the lock.
            if self.stat.load(Acquire) == 0 {
                break WaitTimeoutResult::Reached;
            }

            let timeout = match dur.checked_sub(start.elapsed()) {
                Some(t) => t,
                None => break WaitTimeoutResult::TimedOut(()),
            };

            m = self.cvar.wait_timeout(m, timeout);
        }
    }

    fn spin(&self) -> bool {
        macros::spin_try_wait!(self, s, true, s == 0);
    }

    /// It will be called by the counter reaches 0.
    ///
    /// RACE CONDITION:
    #[cold]
    fn done(&self) {
        self.cvar.notify_all();
    }
}

impl fmt::Debug for Latch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Latch")
            .field("count", &self.stat)
            .finish_non_exhaustive()
    }
}
