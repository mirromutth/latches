pub(crate) use std::sync::MutexGuard;

use std::sync::Mutex as StdMutex;

#[cfg(feature = "sync")]
use std::{sync::Condvar as StdCondvar, time::Duration};

/// Ignore poison error for [`std::sync::Mutex`].
pub(crate) struct Mutex<T>(StdMutex<T>);

#[cfg(feature = "sync")]
pub(crate) struct EmptyCondvar(Mutex<()>, StdCondvar);

#[cfg(feature = "sync")]
pub(crate) type CondvarGuard<'a> = MutexGuard<'a, ()>;

impl<T> Mutex<T> {
    #[must_use]
    #[inline]
    pub(crate) const fn new(t: T) -> Self {
        Self(StdMutex::new(t))
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, T> {
        match self.0.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        }
    }
}

#[cfg(feature = "sync")]
impl EmptyCondvar {
    #[must_use]
    #[inline]
    pub(crate) const fn new() -> Self {
        Self(Mutex::new(()), StdCondvar::new())
    }

    /// Notifies all threads waiting on this condition variable.
    ///
    /// Acquiring the lock is required. If do not, it will be like:
    ///
    /// | Thread X       | Thread CountDown |
    /// |----------------|------------------|
    /// | condition==0   |                  |
    /// | acquired lock  |                  |
    /// | condition==0   |                  |
    /// |                | make condition=1 |
    /// |                | notify all       |
    /// | released lock  |                  |
    /// | condvar wait   |                  |
    /// | (HANG FOREVER) |                  |
    ///
    /// Therefore, acquiring the lock here acts like a barrier to ensure that
    /// the last locking block has completed and subsequent locking blocks have
    /// not started yet.
    ///
    /// | Thread X      | Thread CountDown | Thread Y       |
    /// |---------------|------------------|----------------|
    /// | condition==0  |                  |                |
    /// | acquired lock |                  |                |
    /// | condition==0  |                  | condition==0   |
    /// |               | make condition=1 |                |
    /// |               | acquiring lock   | acquiring lock |
    /// | released lock |                  |                |
    /// | condvar wait  | acquired lock    |                |
    /// |               | released lock    |                |
    /// |               |                  | acquired lock  |
    /// |               | notify all       | condition==1   |
    /// |               |                  | released lock  |
    pub(crate) fn notify_all(&self) {
        drop(self.monitor());
        self.1.notify_all()
    }

    pub(crate) fn monitor(&self) -> CondvarGuard<'_> {
        self.0.lock()
    }

    pub(crate) fn wait<'a>(&self, m: CondvarGuard<'a>) -> CondvarGuard<'a> {
        match self.1.wait(m) {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        }
    }

    pub(crate) fn wait_timeout<'a>(
        &self,
        guard: CondvarGuard<'a>,
        dur: Duration,
    ) -> CondvarGuard<'a> {
        match self.1.wait_timeout(guard, dur) {
            Ok(g) => g.0,
            Err(e) => e.into_inner().0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Mutex;

    use std::{sync::Arc, thread, time::Duration};

    #[test]
    fn test_posion_mutex() {
        let mutex = Arc::new(Mutex::new(()));
        let m = mutex.clone();

        thread::spawn(move || {
            let _guard = m.lock();
            panic!();
        });

        thread::sleep(Duration::from_millis(4));

        let guard = mutex.lock();

        *guard
    }

    #[cfg(feature = "sync")]
    #[test]
    fn test_posion_condvar_wait() {
        let condvar = Arc::new(super::EmptyCondvar::new());
        let c1 = condvar.clone();
        let c2 = condvar.clone();

        thread::spawn(move || {
            let _guard = c1.monitor();
            panic!();
        });

        thread::sleep(Duration::from_millis(4));

        thread::spawn(move || {
            thread::sleep(Duration::from_millis(8));
            c2.notify_all();
        });

        let guard = condvar.monitor();
        let guard = condvar.wait(guard);

        *guard
    }

    #[cfg(feature = "sync")]
    #[test]
    fn test_posion_condvar_wait_timeout() {
        let condvar = Arc::new(super::EmptyCondvar::new());
        let c1 = condvar.clone();
        let c2 = condvar.clone();

        thread::spawn(move || {
            let _guard = c1.monitor();
            panic!();
        });

        thread::sleep(Duration::from_millis(4));

        thread::spawn(move || {
            thread::sleep(Duration::from_millis(8));
            c2.notify_all();
        });

        let guard = condvar.monitor();
        let guard = condvar.wait_timeout(guard, Duration::from_millis(4));

        *guard
    }
}
