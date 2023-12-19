use std::{
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
    thread,
    time::{Duration, Instant},
};

use tokio::time::sleep;

use crate::WaitTimeoutResult;

use super::Latch;

/// Asynchronous timer may not be accurate.
macro_rules! assert_time {
    ($time:expr, $mills:literal $(,)?) => {
        assert!((Duration::from_millis($mills - 1)
            ..Duration::from_millis($mills + std::cmp::max($mills >> 1, 50)))
            .contains(&$time))
    };
}

/// A wrapper for `tokio::time::Sleep`.
///
/// It contains a hack way to test the latch double check within timers.
struct Timer {
    sleep: tokio::time::Sleep,
    latch: Option<Arc<Latch>>,
}

impl Timer {
    fn new(dur: Duration) -> Self {
        Self {
            sleep: sleep(dur),
            latch: None,
        }
    }

    fn hack(mut self, latch: &Arc<Latch>) -> Self {
        self.latch = Some(latch.clone());
        self
    }

    fn reset(self: Pin<&mut Self>, dur: Duration) {
        let Timer { sleep, .. } = unsafe { self.get_unchecked_mut() };
        let sleep = unsafe { Pin::new_unchecked(sleep) };

        sleep.reset(tokio::time::Instant::now() + dur);
    }
}

impl Future for Timer {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Timer { sleep, latch } = unsafe { self.get_unchecked_mut() };
        let sleep = unsafe { Pin::new_unchecked(sleep) };

        let r = sleep.poll(cx);

        if r.is_ready() {
            if let Some(latch) = latch.take() {
                // Hack count_down to simulate the race condition.
                latch.stat.fetch_sub(1, Ordering::Relaxed);
            }
        }

        r
    }
}

#[tokio::test]
async fn test_count_down() {
    let latch = Arc::new(Latch::new(3));

    latch.count_down();
    latch.count_down();
    latch.count_down();

    assert_eq!(latch.count(), 0);
}

#[tokio::test]
async fn test_arrive() {
    let latch = Latch::new(3);

    latch.arrive(3);

    assert_eq!(latch.count(), 0);
}

#[test]
fn test_arrive_zero() {
    let latch = Latch::new(2);

    latch.arrive(0);

    assert_eq!(latch.count(), 2);
}

#[test]
fn test_try_wait() {
    let latch = Latch::new(0);

    assert_eq!(latch.try_wait(), Ok(()));
}

#[test]
fn test_try_wait_err() {
    let latch = Latch::new(3);

    assert_eq!(latch.try_wait(), Err(3));
}

#[tokio::test]
async fn test_last_one_signal() {
    let latch = Arc::new(Latch::new(3));
    let l1 = latch.clone();

    latch.count_down();
    latch.count_down();

    let start = Instant::now();

    tokio::spawn(async move {
        sleep(Duration::from_millis(32)).await;
        l1.count_down()
    });

    latch.wait().await;
    assert_time!(start.elapsed(), 32);
    assert_eq!(latch.count(), 0);
}

#[tokio::test]
async fn test_gate_wait() {
    let latch = Arc::new(Latch::new(1));
    let tasks: Vec<_> = (0..4)
        .map(|_| {
            let latch = latch.clone();
            let start = Instant::now();

            tokio::spawn(async move {
                latch.wait().await;
                start.elapsed()
            })
        })
        .collect();

    sleep(Duration::from_millis(20)).await;
    latch.count_down();

    for t in tasks {
        assert_time!(t.await.unwrap(), 20);
    }
}

#[tokio::test]
async fn test_gate_watch() {
    let latch = Arc::new(Latch::new(1));
    let tasks: Vec<_> = (0..4)
        .map(|_| {
            let latch = latch.clone();
            let start = Instant::now();

            tokio::spawn(async move {
                let timer = Timer::new(Duration::from_millis(100));

                assert!(latch.watch(timer).await.is_reached());
                start.elapsed()
            })
        })
        .collect();

    sleep(Duration::from_millis(20)).await;
    latch.count_down();

    for t in tasks {
        assert_time!(t.await.unwrap(), 20);
    }
}

#[tokio::test]
async fn test_multi_tasks() {
    const SIZE: u32 = 16;
    let latch = Arc::new(Latch::new(SIZE as usize));
    let counter = Arc::new(AtomicU32::new(0));

    for _ in 0..SIZE {
        let latch = latch.clone();
        let counter = counter.clone();

        tokio::spawn(async move {
            sleep(Duration::from_millis(20)).await;
            counter.fetch_add(1, Ordering::Relaxed);
            latch.count_down()
        });
    }

    let start = Instant::now();

    latch.wait().await;
    assert_time!(start.elapsed(), 20);
    assert_eq!(counter.load(Ordering::Relaxed), SIZE);
    assert_eq!(latch.count(), 0);
}

#[tokio::test]
async fn test_more_count_down() {
    const SIZE: u32 = 16;
    let latch = Arc::new(Latch::new(SIZE as usize));
    let counter = Arc::new(AtomicU32::new(0));

    for _ in 0..(SIZE + (SIZE >> 1)) {
        let latch = latch.clone();
        let counter = counter.clone();

        tokio::spawn(async move {
            sleep(Duration::from_millis(10)).await;
            counter.fetch_add(1, Ordering::Relaxed);
            latch.count_down()
        });
    }

    latch.wait().await;
    assert!(counter.load(Ordering::Relaxed) >= SIZE);
    assert_eq!(latch.count(), 0);
    latch.count_down();
    assert_eq!(latch.count(), 0);
}

#[tokio::test]
async fn test_more_arrive() {
    let latch = Latch::new(10);

    for _ in 0..4 {
        latch.arrive(3);
    }

    assert_eq!(latch.count(), 0);
}

#[tokio::test]
async fn test_select_two_wait() {
    let latch1 = Arc::new(Latch::new(1));
    let latch2 = Arc::new(Latch::new(1));
    let l1 = latch1.clone();
    let l2 = latch2.clone();

    tokio::spawn(async move {
        sleep(Duration::from_millis(50)).await;
        l1.count_down();
    });
    tokio::spawn(async move {
        sleep(Duration::from_millis(10)).await;
        l2.count_down();
    });

    assert!(tokio::select! {
        _ = latch1.wait() => false,
        _ = latch2.wait() => true,
    });

    latch1.wait().await;
}

static VTABLE: RawWakerVTable = RawWakerVTable::new(v_c, wake, wake, noop);

unsafe fn v_c(ptr: *const ()) -> RawWaker {
    RawWaker::new(ptr, &VTABLE)
}
unsafe fn wake(ptr: *const ()) {
    (*(ptr as *const AtomicU32)).fetch_add(1, Ordering::Relaxed);
}
const unsafe fn noop(_: *const ()) {}

fn new_waker(data: Pin<&AtomicU32>) -> Waker {
    unsafe {
        Waker::from_raw(RawWaker::new(
            data.get_ref() as *const _ as *const (),
            &VTABLE,
        ))
    }
}
fn pin<R, F: Future<Output = R>>(f: &mut F) -> Pin<&mut F> {
    unsafe { Pin::new_unchecked(f) }
}

#[test]
fn test_wait_only_last_waked() {
    let latch = Latch::new(1);
    let good = Box::pin(AtomicU32::new(0));
    let bad = Box::pin(AtomicU32::new(0));
    let good_waker = new_waker(good.as_ref());
    let bad_waker = new_waker(bad.as_ref());
    let wait = &mut latch.wait();
    let bad_cx = &mut Context::from_waker(&bad_waker);
    let good_cx = &mut Context::from_waker(&good_waker);

    assert_eq!(pin(wait).poll(bad_cx), Poll::Pending);
    assert_eq!(pin(wait).poll(good_cx), Poll::Pending);

    latch.count_down();

    assert_eq!(latch.count(), 0);
    assert_eq!(pin(wait).poll(bad_cx), Poll::Ready(()));
    assert_eq!(good.load(Ordering::Relaxed), 1);
    assert_eq!(bad.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn test_watch_only_last_waked() {
    let latch = Latch::new(1);
    let good = Box::pin(AtomicU32::new(0));
    let bad = Box::pin(AtomicU32::new(0));
    let good_waker = new_waker(good.as_ref());
    let bad_waker = new_waker(bad.as_ref());
    let fut = &mut latch.watch(Timer::new(Duration::from_millis(100)));
    let bad_cx = &mut Context::from_waker(&bad_waker);
    let good_cx = &mut Context::from_waker(&good_waker);

    assert_eq!(pin(fut).poll(bad_cx), Poll::Pending);
    assert_eq!(pin(fut).poll(good_cx), Poll::Pending);

    latch.count_down();

    assert_eq!(latch.count(), 0);
    assert_eq!(
        pin(fut).poll(bad_cx),
        Poll::Ready(WaitTimeoutResult::Reached)
    );
    assert_eq!(good.load(Ordering::Relaxed), 1);
    assert_eq!(bad.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn test_watch_timed_out_only_waked_by_timer() {
    let latch = Latch::new(1);
    let counter = Box::pin(AtomicU32::new(0));
    let waker = new_waker(counter.as_ref());
    let fut = &mut latch.watch(Timer::new(Duration::from_millis(10)));
    let cx = &mut Context::from_waker(&waker);

    assert_eq!(pin(fut).poll(cx), Poll::Pending);

    sleep(Duration::from_millis(24)).await;

    assert_eq!(
        pin(fut).poll(cx),
        Poll::Ready(WaitTimeoutResult::TimedOut(()))
    );
    assert_eq!(counter.load(Ordering::Relaxed), 1);

    latch.count_down();

    assert_eq!(counter.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn test_watch() {
    let latch = Arc::new(Latch::new(1));
    let l = latch.clone();
    let start = Instant::now();
    let r = latch.watch(Timer::new(Duration::from_millis(100)));

    assert!(start.elapsed() < Duration::from_millis(1));
    tokio::spawn(async move { l.count_down() });

    let x = r.await;

    assert!(x.is_reached());
}

#[tokio::test]
async fn test_watch_timed_out() {
    let latch = Arc::new(Latch::new(1));
    let start = Instant::now();
    let r = latch.watch(Timer::new(Duration::from_millis(50)));

    assert!(start.elapsed() < Duration::from_millis(1));

    let x = r.await;

    assert_time!(start.elapsed(), 50);
    assert!(x.is_timed_out());
}

#[tokio::test]
async fn test_watch_timed_out_before() {
    let latch = Arc::new(Latch::new(1));
    let x = latch.watch(Timer::new(Duration::ZERO)).await;

    assert!(x.is_timed_out());
}

#[tokio::test]
async fn test_wait_timed_out_reset() {
    let latch = Arc::new(Latch::new(1));
    let start = Instant::now();
    let r = latch.watch(Timer::new(Duration::from_millis(1)));

    assert!(start.elapsed() < Duration::from_millis(1));
    tokio::pin!(r);
    r.as_mut().timer().reset(Duration::from_millis(50));

    let x = r.await;

    assert_time!(start.elapsed(), 50);
    assert!(x.is_timed_out());
}

#[tokio::test]
async fn test_watch_await_twice() {
    let latch = Arc::new(Latch::new(1));
    let l = latch.clone();
    let start = Instant::now();
    let r = latch.watch(Timer::new(Duration::from_millis(100)));

    assert!(start.elapsed() < Duration::from_millis(1));
    tokio::pin!(r);
    tokio::spawn(async move { l.count_down() });

    let x = r.as_mut().await;
    let y = r.as_mut().await;

    assert!(x.is_reached());
    assert!(y.is_reached());
}

#[tokio::test]
async fn test_wait_timed_out_await_twice() {
    let latch = Arc::new(Latch::new(1));
    let start = Instant::now();
    let r = latch.watch(Timer::new(Duration::from_millis(50)));

    assert!(start.elapsed() < Duration::from_millis(1));
    tokio::pin!(r);

    let x = r.as_mut().await;
    let y = r.as_mut().await;

    assert_time!(start.elapsed(), 50);
    assert!(x.is_timed_out());
    assert!(y.is_timed_out());
}

#[tokio::test]
async fn test_select_two_watch() {
    let latch = Arc::new(Latch::new(1));
    let l1 = latch.clone();

    let mut slow = tokio::spawn(async move {
        let start = Instant::now();
        let timer = Timer::new(Duration::from_millis(500));
        let r = latch.watch(timer).await;

        assert!(r.is_timed_out());
        start.elapsed()
    });
    let mut slow = Pin::new(&mut slow);
    let fast = tokio::spawn(async move {
        let start = Instant::now();
        let timer = Timer::new(Duration::from_millis(50));
        let r = l1.watch(timer).await;

        assert!(r.is_timed_out());
        start.elapsed()
    });

    tokio::select! {
        _ = slow.as_mut() => panic!("slow should not be ready"),
        time = fast => assert_time!(time.unwrap(), 50),
    }

    slow.await.unwrap();
}

#[test]
fn test_debug() {
    let latch = Latch::new(3);
    let stringified = format!("{:?}", latch);

    assert!(stringified.contains("Latch"));
    assert!(stringified.contains('3'));
}

#[test]
fn test_wait_debug() {
    let latch = Latch::new(1);
    let stringified = format!("{:?}", latch.wait());

    assert!(stringified.contains("LatchWait"));
}

#[tokio::test]
async fn test_watch_debug() {
    let latch = Latch::new(1);
    let r = latch.watch(Timer::new(Duration::from_millis(10)));
    let stringified = format!("{:?}", r);

    assert!(stringified.contains("LatchWatch"));
    assert!(r.await.is_timed_out());
}

#[tokio::test]
async fn test_watch_timed_out_reached() {
    let latch = Arc::new(Latch::new(1));

    assert!(latch
        .watch(Timer::new(Duration::ZERO).hack(&latch))
        .await
        .is_reached());
}

#[tokio::test]
async fn test_watch_timed_out_locked() {
    let latch = Arc::new(Latch::new(1));
    let l = latch.clone();
    let h = tokio::spawn(async move {
        let x = l.watch(Timer::new(Duration::from_millis(10))).await;
        assert!(x.is_timed_out());
    });

    sleep(Duration::from_millis(2)).await;
    drop(latch.lock.lock());

    h.await.unwrap();
}

#[test]
fn test_wait_double_check() {
    use tokio::runtime::Builder;

    let runtime = Builder::new_multi_thread().build().unwrap();
    let latch = Arc::new(Latch::new(1));
    let l1 = latch.clone();
    let lock = latch.lock.lock();
    let t = runtime.spawn(async move { l1.wait().await });

    thread::sleep(Duration::from_millis(8));
    latch.stat.fetch_sub(1, Ordering::Release);
    drop(lock);
    runtime.block_on(t).unwrap();
}

#[test]
fn test_watch_double_check() {
    use tokio::runtime::Builder;

    fn current_runtime() -> tokio::runtime::Runtime {
        Builder::new_current_thread().enable_time().build().unwrap()
    }

    let latch = Arc::new(Latch::new(1));
    let l1 = latch.clone();
    let lock = latch.lock.lock();
    let t = thread::spawn(move || {
        let runtime = current_runtime();
        let t = runtime.spawn(async move {
            let timer = Timer::new(Duration::from_millis(32));
            l1.watch(timer).await
        });
        assert!(runtime.block_on(t).unwrap().is_reached());
    });

    thread::sleep(Duration::from_millis(8));
    latch.stat.fetch_sub(1, Ordering::Release);
    drop(lock);
    t.join().unwrap();
}
