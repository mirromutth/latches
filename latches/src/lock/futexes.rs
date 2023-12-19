#[cfg(feature = "task")]
use core::{
    cell::UnsafeCell,
    hint,
    ops::{Deref, DerefMut},
    sync::atomic::Ordering::{Acquire, Release},
};
use core::{
    marker::PhantomData,
    sync::atomic::{AtomicU32, Ordering::Relaxed},
};

/// An alternative to [`std::sync::Mutex`] for `no_std`.
///
/// It should be released as soon as possible. So, prioritizing spin may
/// provide better performance.
///
/// It is based on the futex atomic wake/wait.
#[cfg(feature = "task")]
pub(crate) struct Mutex<T: ?Sized> {
    /// The status of futex.
    ///
    /// - 0: unlocked
    /// - 1: locked, no other threads waiting
    /// - 2: locked, and other threads waiting (contended)
    stat: AtomicU32,
    data: UnsafeCell<T>,
}

#[cfg(feature = "task")]
unsafe impl<T: ?Sized + Send> Send for Mutex<T> {}
#[cfg(feature = "task")]
unsafe impl<T: ?Sized + Send> Sync for Mutex<T> {}

/// A locked guard.
#[cfg(feature = "task")]
#[must_use]
pub(crate) struct MutexGuard<'a, T: ?Sized> {
    lock: &'a Mutex<T>,
    _mrk: PhantomData<*const ()>,
}

#[cfg(feature = "task")]
unsafe impl<T: ?Sized + Sync> Sync for MutexGuard<'_, T> {}

#[cfg(feature = "sync")]
pub(crate) struct EmptyCondvar {
    stat: AtomicU32,
}

#[cfg(feature = "sync")]
pub(crate) struct CondvarGuard {
    flag: u32,
    _mrk: PhantomData<*const ()>,
}

#[cfg(feature = "task")]
impl<T> Mutex<T> {
    pub(crate) const fn new(data: T) -> Self {
        Self {
            stat: AtomicU32::new(0),
            data: UnsafeCell::new(data),
        }
    }
}

#[cfg(feature = "task")]
impl<T: ?Sized> Mutex<T> {
    pub(crate) fn lock(&self) -> MutexGuard<'_, T> {
        if self.stat.compare_exchange(0, 1, Acquire, Relaxed).is_err() {
            self.lock_slow()
        }

        MutexGuard {
            lock: self,
            _mrk: PhantomData,
        }
    }

    /// Blocks the current thread until the lock is acquired.
    ///
    /// See also the `futex_mutex.rs` in the `std`.
    #[cold]
    fn lock_slow(&self) {
        let mut stat = self.spin();

        if stat == 0 {
            match self.stat.compare_exchange(0, 1, Acquire, Relaxed) {
                Ok(_) => return,
                Err(s) => stat = s,
            }
        }

        loop {
            if stat != 2 && self.stat.swap(2, Acquire) == 0 {
                return;
            }

            atomic_wait::wait(&self.stat, 2);
            stat = self.spin();
        }
    }

    fn spin(&self) -> u32 {
        #[cfg(coverage)]
        if let Ok(x) = std::env::var("__LATCHES_SPIN_MOCK") {
            return x.parse().unwrap();
        }
        let mut spin = 100;

        loop {
            let stat = self.stat.load(Relaxed);

            if stat != 1 || spin == 0 {
                return stat;
            }

            hint::spin_loop();
            spin -= 1;
        }
    }

    unsafe fn unlock(&self) {
        if self.stat.swap(0, Release) == 2 {
            atomic_wait::wake_one(&self.stat);
        }
    }
}

#[cfg(feature = "task")]
impl<T: ?Sized> Deref for MutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

#[cfg(feature = "task")]
impl<T: ?Sized> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

#[cfg(feature = "task")]
impl<T: ?Sized> Drop for MutexGuard<'_, T> {
    fn drop(&mut self) {
        unsafe { self.lock.unlock() }
    }
}

#[cfg(feature = "sync")]
impl EmptyCondvar {
    pub(crate) const fn new() -> Self {
        Self {
            stat: AtomicU32::new(0),
        }
    }

    /// Notifies all threads waiting on this condition variable.
    ///
    /// Increment the state will make all monitors invalid, which makes
    /// fail-fast for all waits that loaded monitor but are not in a waiting.
    ///
    /// | Thread X     | Thread CountDown | Thread Y     |
    /// |--------------|------------------|--------------|
    /// | condition==0 |                  | condition==0 |
    /// | load monitor |                  | load monitor |
    /// | condition==0 |                  | condition==0 |
    /// |              | make condition=1 | atomic wait  |
    /// | wait invalid | increment state  | (waiting)    |
    /// |              | notify all       | (waked up)   |
    /// | load monitor |                  | load monitor |
    /// | condition==1 |                  | condition==1 |
    pub(crate) fn notify_all(&self) {
        self.stat.fetch_add(1, Relaxed);
        atomic_wait::wake_all(&self.stat);
    }

    pub(crate) fn monitor(&self) -> CondvarGuard {
        CondvarGuard {
            flag: self.stat.load(Relaxed),
            _mrk: PhantomData,
        }
    }

    pub(crate) fn wait(&self, guard: CondvarGuard) -> CondvarGuard {
        atomic_wait::wait(&self.stat, guard.flag);

        self.monitor()
    }
}

#[cfg(all(test, feature = "task"))]
mod tests {
    use super::Mutex;

    use std::{
        cell::UnsafeCell,
        sync::{atomic::Ordering, Arc},
        thread,
        time::{Duration, Instant},
    };

    macro_rules! assert_time {
        ($time:expr, $m:literal $(,)?) => {
            assert!((Duration::from_millis($m)
                ..Duration::from_millis($m + std::cmp::max($m >> 1, 10)))
                .contains(&$time))
        };
    }

    struct UnsafeSendCell<T>(UnsafeCell<T>);

    unsafe impl<T> Send for UnsafeSendCell<T> {}
    unsafe impl<T> Sync for UnsafeSendCell<T> {}

    impl<T> UnsafeSendCell<T> {
        fn new(t: T) -> Self {
            Self(UnsafeCell::new(t))
        }

        unsafe fn get(&self) -> *mut T {
            self.0.get()
        }
    }

    #[test]
    fn test_lock() {
        let mutex = Mutex::new(());
        let guard = mutex.lock();

        *guard
    }

    #[test]
    fn test_multi_thread() {
        let mutex = Arc::new(Mutex::new(()));
        let counter = Arc::new(UnsafeSendCell::new(0_usize));
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let mutex = mutex.clone();
                let counter = counter.clone();

                thread::spawn(move || {
                    let _guard = mutex.lock();

                    unsafe {
                        *counter.get() += 1;
                    };
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        let lock = mutex.lock();

        assert_eq!(unsafe { *counter.get() }, 4);

        *lock
    }

    #[test]
    fn test_slow_thread() {
        let mutex = Arc::new(Mutex::new(()));
        let counter = Arc::new(UnsafeSendCell::new(0_usize));
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let mutex = mutex.clone();
                let counter = counter.clone();

                thread::spawn(move || {
                    let _guard = mutex.lock();

                    thread::sleep(Duration::from_millis(10));

                    unsafe {
                        *counter.get() += 1;
                    };
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        let lock = mutex.lock();

        assert_eq!(unsafe { *counter.get() }, 4);

        *lock
    }

    #[test]
    fn test_lock_slow() {
        let mutex = Arc::new(Mutex::new(()));
        let m = mutex.clone();

        mutex.lock_slow();

        assert_eq!(mutex.stat.load(Ordering::Relaxed), 1);

        let t = thread::spawn(move || {
            let start = Instant::now();

            m.lock_slow();
            unsafe { m.unlock() };

            start.elapsed()
        });

        thread::sleep(Duration::from_millis(10));
        unsafe { mutex.unlock() };

        assert_time!(t.join().unwrap(), 10);
    }

    #[cfg(coverage)]
    #[test]
    fn test_lock_slow_spin() {
        struct TempEnv {
            name: &'static str,
        }

        impl Drop for TempEnv {
            fn drop(&mut self) {
                std::env::remove_var(self.name);
            }
        }

        let mutex = Arc::new(Mutex::new(()));

        mutex.lock_slow();

        let m = mutex.clone();
        let t = thread::spawn(move || {
            const ENV_NAME: &str = "__LATCHES_SPIN_MOCK";

            let _e = {
                assert_eq!(m.stat.load(Ordering::Relaxed), 1);
                std::env::set_var(ENV_NAME, "0");

                TempEnv { name: ENV_NAME }
            };
            m.lock_slow();
            unsafe { m.unlock() };
        });

        thread::sleep(Duration::from_millis(16));
        unsafe { mutex.unlock() };
        t.join().unwrap();
    }
}
