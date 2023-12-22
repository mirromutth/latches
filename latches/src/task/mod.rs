use core::{
    fmt,
    future::Future,
    hint,
    pin::Pin,
    sync::atomic::{
        AtomicUsize,
        Ordering::{Acquire, Relaxed, Release},
    },
    task::{Context, Poll},
};

use crate::{lock::Mutex, macros, WaitTimeoutResult};

use self::waiters::Waiters;

#[cfg(test)]
mod tests;

mod waiters;

/// A latch is a downward counter which can be used to coordinate tasks. The
/// value of the counter is initialized on creation. Tasks may suspend on the
/// latch until the counter is decremented to 0.
///
/// In contrast to [`Barrier`], it is a one-shot phenomenon, that mean the
/// counter will not be reset after reaching 0. However, it has a useful
/// property in that it does not make tasks wait for the counter to reach 0 by
/// calling [`count_down()`] or [`arrive()`].
///
/// It spins on every polling of waiting futures.
///
/// # Examples
///
/// Created by `1` can be used as a simple gate, all tasks calling [`wait()`]
/// will be suspended until a task calls [`count_down()`].
///
/// Created by `N` can be used to make one or more tasks wait until `N`
/// operations have completed, or an operation has completed 'N' times.
///
/// [`Barrier`]: std::sync::Barrier
/// [`Future`]: std::future::Future
/// [`arrive()`]: Latch::arrive
/// [`count_down()`]: Latch::count_down
/// [`wait()`]: Latch::wait
///
/// ```
/// # use tokio::{runtime::Builder, task};
/// use std::sync::{
///     atomic::{AtomicU32, Ordering},
///     Arc, RwLock,
/// };
///
/// use latches::task::Latch;
///
/// # Builder::new_multi_thread().build().unwrap().block_on(async move {
/// let init_gate = Arc::new(Latch::new(1));
/// let operation = Arc::new(Latch::new(30));
/// let results = Arc::new(RwLock::new(Vec::<AtomicU32>::new()));
///
/// for i in 0..10 {
///     let gate = init_gate.clone();
///     let part = operation.clone();
///     let res = results.clone();
///
///     // Each task need to process 3 operations
///     task::spawn(async move {
///         gate.wait().await;
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
/// task::spawn(async move {
///     // Init some statuses, e.g. DB, File System, etc.
///     let mut db = res.write().unwrap();
///     for _ in 0..30 {
///         db.push(AtomicU32::new(0));
///     }
///     init_gate.count_down();
/// });
///
/// // All 30 operations will be done after this line
/// // Or use operation.watch(T) to set the timeout
/// operation.wait().await;
///
/// let res: Vec<_> = results.read()
///     .unwrap()
///     .iter()
///     .map(|i| i.load(Ordering::Relaxed))
///     .collect();
/// assert_eq!(res, Vec::from_iter(0..30));
/// # });
/// ```
pub struct Latch {
    stat: AtomicUsize,
    lock: Mutex<Waiters>,
}

impl Latch {
    /// Creates a new latch initialized with the given count.
    ///
    /// # Examples
    ///
    /// ```
    /// use latches::task::Latch;
    ///
    /// let latch = Latch::new(10);
    /// # drop(latch);
    /// ```
    #[must_use]
    #[inline]
    pub const fn new(count: usize) -> Self {
        Self {
            stat: AtomicUsize::new(count),
            lock: Mutex::new(Waiters::new()),
        }
    }

    /// Decrements the latch count, wake up all pending tasks if the counter
    /// reaches 0 after decrement.
    ///
    /// - If the counter has reached 0 then do nothing.
    /// - If the current count is greater than 0 then it is decremented.
    /// - If the new count is 0 then all pending tasks are waked up.
    ///
    /// # Examples
    ///
    /// ```
    /// use latches::task::Latch;
    ///
    /// let latch = Latch::new(1);
    /// latch.count_down();
    /// ```
    pub fn count_down(&self) {
        macros::decrement!(self, 1);
    }

    /// Decrements the latch count by `n`, wake up all pending tasks if the
    /// counter reaches 0 after decrement.
    ///
    /// It will not cause an overflow by decrement the counter.
    ///
    /// - If the `n` is 0 or the counter has reached 0 then do nothing.
    /// - If the current count is greater than `n` then decremented by `n`.
    /// - If the current count is greater than 0 and less than or equal to `n`,
    ///   then the new count will be 0, and all pending tasks are waked up.
    ///
    /// # Examples
    ///
    /// ```
    /// use latches::task::Latch;
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
    /// use latches::task::Latch;
    ///
    /// let latch = Latch::new(3);
    /// assert_eq!(latch.count(), 3);
    /// ```
    #[must_use]
    #[inline]
    pub fn count(&self) -> usize {
        self.stat.load(Acquire)
    }

    /// Checks that the counter has reached 0.
    ///
    /// # Errors
    ///
    /// This function will return an error with the current count if the
    /// counter has not reached 0.
    ///
    /// # Examples
    ///
    /// ```
    /// use latches::task::Latch;
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

    /// Returns a future that suspends the current task to wait until the
    /// counter reaches 0.
    ///
    /// When the future is polled:
    ///
    /// - If the current count is 0 then ready immediately.
    /// - If the current count is greater than 0 then pending with a waker that
    ///   will be awakened by a [`count_down()`]/[`arrive()`] invocation which
    ///   causes the counter reaches 0.
    ///
    /// [`count_down()`]: Latch::count_down
    /// [`arrive()`]: Latch::arrive
    ///
    /// # Examples
    ///
    /// ```
    /// # use tokio::runtime::Builder;
    /// use std::{sync::Arc, thread};
    ///
    /// use latches::task::Latch;
    ///
    /// # Builder::new_multi_thread().build().unwrap().block_on(async move {
    /// let latch = Arc::new(Latch::new(1));
    /// let l1 = latch.clone();
    ///
    /// thread::spawn(move || l1.count_down());
    /// latch.wait().await;
    /// # });
    /// ```
    #[inline]
    pub const fn wait(&self) -> LatchWait<'_> {
        LatchWait {
            id: None,
            latch: self,
        }
    }

    /// Returns a future that suspends the current task to wait until the
    /// counter reaches 0 or the timer done.
    ///
    /// It requires an asynchronous timer, which provides greater flexibility
    /// for optimization. For example, some implementations provide higher
    /// precision timers, while other implementations sacrifice timing accuracy
    /// for performance. Some async libraries provide a global timer pool, if
    /// your project is using these libraries you should consider using their
    /// built-in timers first.
    ///
    /// When the future is polled:
    ///
    /// - If the current count is 0 then [`Reached`] ready immediately.
    /// - If the timer is done then [`TimedOut(timer_res)`] ready immediately.
    /// - If the current count is greater than 0 then pending with a waker that
    ///   will be awakened by a [`count_down()`]/[`arrive()`] invocation which
    ///   causes the counter reaches 0, or awakened by the timer.
    ///
    /// [`Reached`]: WaitTimeoutResult::Reached
    /// [`TimedOut(timer_res)`]: WaitTimeoutResult::TimedOut
    /// [`count_down()`]: Latch::count_down
    /// [`arrive()`]: Latch::arrive
    ///
    /// # Examples
    ///
    /// This example shows how to extend your own `wait_timeout`.
    ///
    /// It is based on tokio, you can use other implementations that your
    /// prefers, like async-std, futures-timer, async-io, gloo-timers, etc.
    ///
    /// ```
    /// # use tokio::runtime::Builder;
    /// use std::ops::Deref;
    ///
    /// use tokio::time::{sleep, Duration};
    /// use latches::{task::Latch as Inner, WaitTimeoutResult as Res};
    ///
    /// #[repr(transparent)]
    /// struct Latch(Inner);
    ///
    /// impl Latch {
    ///     const fn new(count: usize) -> Latch {
    ///         Latch(Inner::new(count))
    ///     }
    /// }
    ///
    /// impl Latch {
    ///     async fn wait_timeout(&self, dur: Duration) -> Res<()> {
    ///         self.0.watch(sleep(dur)).await
    ///     }
    /// }
    ///
    /// impl Deref for Latch {
    ///     type Target = Inner;
    ///
    ///     fn deref(&self) -> &Self::Target {
    ///         &self.0
    ///     }
    /// }
    ///
    /// # Builder::new_multi_thread().enable_time().build().unwrap()
    /// # .block_on(async move {
    /// let latch = Latch::new(3);
    /// let dur = Duration::from_millis(10);
    ///
    /// latch.count_down();
    /// assert!(latch.wait_timeout(dur).await.is_timed_out());
    /// latch.arrive(2);
    /// assert!(latch.wait_timeout(dur).await.is_reached());
    /// # });
    /// ```
    #[inline]
    pub const fn watch<T>(&self, timer: T) -> LatchWatch<'_, T> {
        LatchWatch {
            id: None,
            latch: self,
            timer,
        }
    }

    fn spin(&self) -> bool {
        macros::spin_try_wait!(self, s, true, s == 0);
    }

    #[cold]
    fn done(&self) {
        Waiters::wake_all(&self.lock);
    }
}

/// Future returned by [`Latch::wait`].
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct LatchWait<'a> {
    id: Option<usize>,
    latch: &'a Latch,
}

impl Future for LatchWait<'_> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self { latch, id } = self.get_mut();

        if latch.spin() {
            Poll::Ready(())
        } else {
            let mut lock = latch.lock.lock();

            if latch.stat.load(Acquire) == 0 {
                Poll::Ready(())
            } else {
                lock.upsert(id, cx.waker());

                Poll::Pending
            }
        }
    }
}

/// Future returned by [`Latch::watch`].
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct LatchWatch<'a, T> {
    id: Option<usize>,
    latch: &'a Latch,
    timer: T,
}

impl<T> LatchWatch<'_, T> {
    /// Gets the pinned timer.
    ///
    /// It is typically used to reset, cancel or pre-boot the timer, this
    /// depends on the timer implementation.
    ///
    /// # Examples
    ///
    /// This example shows how to reset a tokio timer, other libraries may or
    /// may not have other ways to resetting timers.
    ///
    /// ```
    /// # use tokio::runtime::Builder;
    /// use std::pin::Pin;
    ///
    /// use tokio::time::{sleep, Duration, Instant};
    /// use latches::task::Latch;
    ///
    /// # Builder::new_multi_thread().enable_time().build().unwrap()
    /// # .block_on(async move {
    /// let init_dur = Duration::from_millis(100);
    /// let reset_dur = Duration::from_millis(10);
    /// let latch = Latch::new(1);
    /// let start = Instant::now();
    /// let mut result = latch.watch(sleep(init_dur));
    /// let mut result = unsafe { Pin::new_unchecked(&mut result) };
    ///
    /// result.as_mut()
    ///     .timer() // Get `Pin<&mut tokio::time::Sleep>` here
    ///     .reset(start + reset_dur);
    /// result.await;
    /// assert!((reset_dur..init_dur).contains(&start.elapsed()));
    /// # });
    /// ```
    #[must_use]
    #[inline]
    pub fn timer(self: Pin<&mut Self>) -> Pin<&mut T> {
        // SAFETY: LatchWatch does not implement Drop, not repr(packed),
        // auto implement Unpin if T is Unpin cuz other fields are Unpin.
        unsafe {
            let Self { timer, .. } = self.get_unchecked_mut();
            Pin::new_unchecked(timer)
        }
    }
}

impl<T: Future> Future for LatchWatch<'_, T> {
    type Output = WaitTimeoutResult<T::Output>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // SAFETY: LatchWatch does not implement Drop, not repr(packed),
        // auto implement Unpin if T is Unpin cuz other fields are Unpin.
        let Self { id, latch, timer } = unsafe { self.get_unchecked_mut() };
        let timer = unsafe { Pin::new_unchecked(timer) };

        if latch.spin() {
            Poll::Ready(WaitTimeoutResult::Reached)
        } else {
            // Acquire lock after pulling timer, minimizing lock-in effects.
            let out = timer.poll(cx);
            let mut lock = latch.lock.lock();

            if latch.stat.load(Acquire) == 0 {
                Poll::Ready(WaitTimeoutResult::Reached)
            } else {
                match out {
                    Poll::Ready(t) => {
                        lock.remove(id);

                        Poll::Ready(WaitTimeoutResult::TimedOut(t))
                    }
                    Poll::Pending => {
                        lock.upsert(id, cx.waker());

                        Poll::Pending
                    }
                }
            }
        }
    }
}

impl fmt::Debug for Latch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Latch")
            .field("count", &self.stat)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for LatchWait<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LatchWait").finish_non_exhaustive()
    }
}

impl<T> fmt::Debug for LatchWatch<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LatchWatch").finish_non_exhaustive()
    }
}
