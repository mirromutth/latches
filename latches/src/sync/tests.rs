use std::{sync::Arc, thread, time::Duration};

#[cfg(feature = "std")]
use std::time::Instant;

#[cfg(feature = "std")]
macro_rules! assert_time {
    ($time:expr, $mills:literal $(,)?) => {
        assert!((Duration::from_millis($mills - 1)
            ..Duration::from_millis($mills + std::cmp::max($mills >> 1, 50)))
            .contains(&$time))
    };
}

use super::Latch;

#[test]
fn test_single_thread() {
    let latch = Latch::new(3);

    latch.count_down();
    latch.count_down();
    latch.count_down();

    latch.wait();
    assert_eq!(latch.count(), 0);
}

#[test]
fn test_arrive() {
    let latch = Latch::new(3);

    latch.arrive(3);
    latch.wait();

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

#[test]
fn test_last_one_signal() {
    let latch = Arc::new(Latch::new(3));
    let l1 = latch.clone();

    latch.count_down();
    latch.count_down();

    let t = thread::spawn(move || {
        thread::sleep(Duration::from_millis(32));
        l1.count_down()
    });

    latch.wait();
    assert_eq!(latch.count(), 0);
    t.join().unwrap();
}

#[test]
fn test_gate_wait() {
    let latch = Arc::new(Latch::new(1));
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let latch = latch.clone();

            thread::spawn(move || latch.wait())
        })
        .collect();

    latch.count_down();

    for t in handles {
        t.join().unwrap();
    }
}

#[cfg(feature = "std")]
#[test]
fn test_gate_wait_timeout() {
    let latch = Arc::new(Latch::new(1));
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let latch = latch.clone();
            let dur = Duration::from_millis(32);

            thread::spawn(move || latch.wait_timeout(dur))
        })
        .collect();

    latch.count_down();

    for t in handles {
        assert!(t.join().unwrap().is_reached());
    }
}

#[test]
fn test_multi_threads() {
    let latch = Arc::new(Latch::new(5));
    let handles: Vec<_> = (0..5)
        .map(|_| {
            let latch = latch.clone();

            thread::spawn(move || {
                thread::sleep(Duration::from_millis(10));
                latch.count_down()
            })
        })
        .collect();

    latch.wait();
    assert_eq!(latch.count(), 0);

    for t in handles {
        t.join().unwrap();
    }
}

#[test]
fn test_more_count_down() {
    let latch = Arc::new(Latch::new(3));
    let handles: Vec<_> = (0..5)
        .map(|_| {
            let latch = latch.clone();

            thread::spawn(move || {
                thread::sleep(Duration::from_millis(10));
                latch.count_down()
            })
        })
        .collect();

    latch.wait();
    assert_eq!(latch.count(), 0);
    latch.count_down();
    assert_eq!(latch.count(), 0);
    latch.count_down();

    for t in handles {
        t.join().unwrap();
    }

    assert_eq!(latch.count(), 0);
}

#[test]
fn test_more_arrive() {
    let latch = Latch::new(10);

    for _ in 0..4 {
        latch.arrive(3);
    }
    latch.wait();

    assert_eq!(latch.count(), 0);
}

#[cfg(feature = "std")]
#[test]
fn test_wait_timeout() {
    let latch = Arc::new(Latch::new(1));

    latch.count_down();

    let r = latch.wait_timeout(Duration::from_millis(100));

    assert!(r.is_reached());
}

#[cfg(feature = "std")]
#[test]
fn test_wait_timed_out() {
    let latch = Arc::new(Latch::new(1));
    let start = Instant::now();
    let r = latch.wait_timeout(Duration::from_millis(50));

    assert_time!(start.elapsed(), 50);
    assert!(r.is_timed_out());
}

#[cfg(feature = "std")]
#[test]
fn test_two_wait_timeout() {
    let latch = Arc::new(Latch::new(1));
    let l1 = latch.clone();

    let slow = thread::spawn(move || {
        let start = Instant::now();
        let r = latch.wait_timeout(Duration::from_millis(100));

        assert!(r.is_timed_out());
        start.elapsed()
    });

    let fast = thread::spawn(move || {
        let start = Instant::now();
        let r = l1.wait_timeout(Duration::from_millis(20));

        assert!(r.is_timed_out());
        start.elapsed()
    });

    assert_time!(slow.join().unwrap(), 100);
    assert_time!(fast.join().unwrap(), 20);
}

#[cfg(feature = "std")]
#[test]
fn test_wait_timeout_reached() {
    const DUR: Duration = Duration::from_millis(50);

    let latch = Arc::new(Latch::new(1));
    let l1 = latch.clone();
    let a = thread::spawn(move || latch.wait_timeout(DUR));
    let b = thread::spawn(move || {
        thread::sleep(DUR / 10);
        l1.count_down();
    });

    assert!(a.join().unwrap().is_reached());
    b.join().unwrap()
}

#[test]
fn test_debug() {
    let latch = Latch::new(3);
    let stringified = format!("{:?}", latch);

    assert!(stringified.contains("Latch"));
    assert!(stringified.contains('3'));
}
