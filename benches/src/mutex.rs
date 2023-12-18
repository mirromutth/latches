use std::{
    fmt,
    sync::{Condvar, LockResult, Mutex},
    time::{Duration, Instant},
};

use latches::WaitTimeoutResult;

pub struct Latch {
    stat: Mutex<usize>,
    cvar: Condvar,
}

impl Latch {
    #[must_use]
    #[inline]
    pub const fn new(count: usize) -> Self {
        Self {
            stat: Mutex::new(count),
            cvar: Condvar::new(),
        }
    }

    pub fn count_down(&self) {
        let mut lock = locked(self.stat.lock());

        *lock = match *lock {
            0 => 0,
            1 => {
                self.done();
                0
            }
            x => x - 1,
        };
    }

    pub fn arrive(&self, n: usize) {
        if n == 0 {
            return;
        }

        let mut lock = locked(self.stat.lock());

        *lock = match *lock {
            0 => 0,
            x if x > n => x - n,
            _ => {
                self.done();
                0
            }
        };
    }

    pub fn wait(&self) {
        let mut lock = locked(self.stat.lock());

        while *lock != 0 {
            lock = locked(self.cvar.wait(lock));
        }
    }

    pub fn wait_timeout(&self, dur: Duration) -> WaitTimeoutResult<()> {
        let mut lock = locked(self.stat.lock());
        let start = Instant::now();

        loop {
            if *lock == 0 {
                break WaitTimeoutResult::Reached;
            }

            let timeout = match dur.checked_sub(start.elapsed()) {
                Some(t) => t,
                None => break WaitTimeoutResult::TimedOut(()),
            };

            // Ignore the native timed out for recheck counter in next loop
            lock = locked(self.cvar.wait_timeout(lock, timeout)).0;
        }
    }

    #[cold]
    fn done(&self) {
        self.cvar.notify_all();
    }
}

fn locked<T>(lock: LockResult<T>) -> T {
    match lock {
        Ok(t) => t,
        Err(e) => e.into_inner(),
    }
}

impl fmt::Debug for Latch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Latch")
            .field("count", &self.stat)
            .finish_non_exhaustive()
    }
}
