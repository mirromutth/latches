use std::{
    fmt, hint,
    pin::Pin,
    sync::atomic::{
        AtomicUsize,
        Ordering::{Acquire, Relaxed, Release},
    },
};

use latches::WaitTimeoutResult;
use tokio::{
    sync::Notify,
    time::{timeout, Duration},
};

use crate::{decrement, spin_try_wait};

pub struct Latch {
    stat: AtomicUsize,
    ntfy: Notify,
}

impl Latch {
    #[must_use]
    #[inline]
    pub const fn new(count: usize) -> Self {
        Self {
            stat: AtomicUsize::new(count),
            ntfy: Notify::const_new(),
        }
    }

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

        while self.stat.load(Acquire) != 0 {
            let mut fut = self.ntfy.notified();
            let mut fut = unsafe { Pin::new_unchecked(&mut fut) };

            fut.as_mut().enable();

            if self.stat.load(Acquire) == 0 {
                break;
            }

            fut.await;
        }
    }

    pub async fn wait_timeout(&self, dur: Duration) -> WaitTimeoutResult<()> {
        match timeout(dur, self.wait()).await {
            Ok(_) => WaitTimeoutResult::Reached,
            Err(_) => WaitTimeoutResult::TimedOut(()),
        }
    }

    fn spin(&self) -> bool {
        spin_try_wait!(self, s, true, s == 0);
    }

    #[cold]
    fn done(&self) {
        self.ntfy.notify_waiters();
    }
}

impl fmt::Debug for Latch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Latch")
            .field("count", &self.stat)
            .finish_non_exhaustive()
    }
}
