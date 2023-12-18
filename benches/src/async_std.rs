use std::{
    fmt, hint,
    sync::atomic::{
        AtomicUsize,
        Ordering::{Acquire, Relaxed, Release},
    },
    time::Duration,
};

use async_std::{
    future,
    sync::{Condvar, Mutex},
    task,
};
use latches::WaitTimeoutResult;

use crate::{decrement, spin_try_wait};

pub struct Latch {
    stat: AtomicUsize,
    lock: Mutex<()>,
    cvar: Condvar,
}

impl Latch {
    #[must_use]
    #[inline]
    pub fn new(count: usize) -> Self {
        Self {
            stat: AtomicUsize::new(count),
            lock: Mutex::new(()),
            cvar: Condvar::new(),
        }
    }

    // Note: async count_down will make no fair and performance slower.
    pub fn count_down(&self) {
        decrement!(self, 1);
    }

    pub fn arrive(&self, n: usize) {
        if n == 0 {
            return;
        }

        decrement!(self, n);
    }

    pub async fn wait(&self) {
        if self.spin() {
            return;
        }

        let mut lock = self.lock.lock().await;

        while self.stat.load(Acquire) != 0 {
            lock = self.cvar.wait(lock).await;
        }
    }

    pub async fn wait_timeout(&self, dur: Duration) -> WaitTimeoutResult<()> {
        match future::timeout(dur, self.wait()).await {
            Ok(_) => WaitTimeoutResult::Reached,
            Err(_) => WaitTimeoutResult::TimedOut(()),
        }
    }

    fn spin(&self) -> bool {
        spin_try_wait!(self, s, true, s == 0);
    }

    #[cold]
    fn done(&self) {
        task::block_on(async move {
            let _lock = self.lock.lock().await;
            self.cvar.notify_all();
        });
    }
}

impl fmt::Debug for Latch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Latch")
            .field("count", &self.stat)
            .finish_non_exhaustive()
    }
}
