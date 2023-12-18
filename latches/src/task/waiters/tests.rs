use std::{
    pin::Pin,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
    task::{RawWaker, RawWakerVTable, Waker},
    thread,
};

use crate::lock::Mutex;

use super::{Waiter, Waiters};

static VTABLE: RawWakerVTable = RawWakerVTable::new(v_c, wake, wake, noop);

fn wake_all(waiters: Vec<Waiter>) {
    for waiter in waiters {
        if let Waiter::Occupied(w) = waiter {
            w.wake();
        }
    }
}
unsafe fn v_c(ptr: *const ()) -> RawWaker {
    RawWaker::new(ptr, &VTABLE)
}
unsafe fn wake(ptr: *const ()) {
    (*(ptr as *const AtomicU32)).fetch_add(1, Ordering::Relaxed);
}
unsafe fn noop(_: *const ()) {}

fn new_waker(data: Pin<&AtomicU32>) -> Waker {
    unsafe {
        Waker::from_raw(RawWaker::new(
            data.get_ref() as *const _ as *const (),
            &VTABLE,
        ))
    }
}

#[test]
fn test_upsert_wake() {
    let data = Box::pin(AtomicU32::new(0));
    let mut waiters = Waiters::new();
    let waker = new_waker(data.as_ref());
    let mut id = None;

    waiters.upsert(&mut id, &waker);
    wake_all(waiters.take());

    assert!(id.is_some());
    assert_eq!(data.load(Ordering::Relaxed), 1);
    assert!(waiters.data.is_empty());
}

#[test]
fn test_mutex_waiters_read() {
    let waiters = Arc::new(Mutex::new(Waiters::new()));
    let handles = (0..16)
        .map(|_| {
            let waiters = waiters.clone();

            thread::spawn(move || {
                assert_eq!(waiters.lock().data.len(), 0);
            })
        })
        .collect::<Vec<_>>();

    for t in handles {
        t.join().unwrap();
    }
}

#[test]
fn test_mutex_waitters_upsert() {
    let data = Arc::new(Box::pin(AtomicU32::new(0)));
    let mutex = Arc::new(Mutex::new(Waiters::new()));

    for _ in 0..10 {
        let data = data.clone();
        let mutex = mutex.clone();

        thread::spawn(move || {
            let mut waiters = mutex.lock();
            let waker = new_waker(data.as_ref().as_ref());
            let mut id = None;

            waiters.upsert(&mut id, &waker);
        });
    }

    thread::sleep(std::time::Duration::from_millis(5));

    let mut waiters = mutex.lock();

    assert_eq!(waiters.data.len(), 10);
    wake_all(waiters.take());
    assert!(waiters.data.is_empty());
}

#[test]
fn test_upsert_removed() {
    let data = Box::pin(AtomicU32::new(0));
    let mut waiters = Waiters::new();
    let waker = new_waker(data.as_ref());
    let mut id = None;

    waiters.upsert(&mut id, &waker);
    waiters.remove(&mut id);
    waiters.upsert(&mut id, &waker);
    wake_all(waiters.take());

    assert_eq!(data.load(Ordering::Relaxed), 1);
    assert!(waiters.data.is_empty());
}

#[test]
fn test_upsert_same_waker() {
    let data = Box::pin(AtomicU32::new(0));
    let mut waiters = Waiters::new();
    let waker = new_waker(data.as_ref());
    let mut id = None;

    waiters.upsert(&mut id, &waker);

    assert!(id.is_some());

    let old_id = id;

    waiters.upsert(&mut id, &waker);
    wake_all(waiters.take());

    assert_eq!(id, old_id);
    assert_eq!(data.load(Ordering::Relaxed), 1);
    assert!(waiters.data.is_empty());
}

#[test]
fn test_upsert_same_id() {
    let first = Box::pin(AtomicU32::new(0));
    let second = Box::pin(AtomicU32::new(0));
    let mut waiters = Waiters::new();
    let mut id = None;

    waiters.upsert(&mut id, &new_waker(first.as_ref()));

    assert!(id.is_some());

    let old_id = id;

    waiters.upsert(&mut id, &new_waker(second.as_ref()));
    wake_all(waiters.take());

    assert_eq!(id, old_id);
    assert_eq!(first.load(Ordering::Relaxed), 0);
    assert_eq!(second.load(Ordering::Relaxed), 1);
    assert!(waiters.data.is_empty());
}

#[test]
fn test_remove_bad_id() {
    let mut waiters = Waiters::new();

    waiters.remove(&mut Some(1));
    wake_all(waiters.take());
}

#[test]
fn test_remove_twice() {
    let data = Box::pin(AtomicU32::new(0));
    let mut waiters = Waiters::new();
    let mut id = None;

    waiters.upsert(&mut id, &new_waker(data.as_ref()));

    let mut old_id = id;

    waiters.remove(&mut id);
    waiters.remove(&mut old_id);
    wake_all(waiters.take());

    assert_eq!(data.load(Ordering::Relaxed), 0);
}

#[test]
fn test_remove_none_id() {
    let mut waiters = Waiters::new();

    waiters.remove(&mut None);
    wake_all(waiters.take());
}

#[test]
#[should_panic = "update non-existent waker"]
fn test_upsert_with_wrong_id() {
    let data = Box::pin(AtomicU32::new(0));
    let mut waiters = Waiters::new();
    let mut id = Some(1);

    waiters.upsert(&mut id, &new_waker(data.as_ref()));
}

#[test]
#[should_panic = "update non-existent waker"]
fn test_upsert_with_removed_id() {
    let data = Box::pin(AtomicU32::new(0));
    let mut waiters = Waiters::new();
    let mut id = None;

    waiters.upsert(&mut id, &new_waker(data.as_ref()));

    let mut old_id = id;

    waiters.remove(&mut id);
    waiters.upsert(&mut old_id, &new_waker(data.as_ref()));
}

#[test]
#[should_panic = "internal error: entered unreachable code"]
fn test_unreachable_next_back() {
    let data = Box::pin(AtomicU32::new(0));
    let mut waiters = Waiters::new();

    waiters.upsert(&mut None, &new_waker(data.as_ref()));
    waiters.next = 0;
    waiters.upsert(&mut None, &new_waker(data.as_ref()));
}

#[cfg(coverage)]
#[test]
#[should_panic = "internal error: entered unreachable code"]
fn test_unreachable_out_of_bound() {
    let data = Box::pin(AtomicU32::new(0));
    let mut waiters = Waiters::new();

    waiters.next = 1;
    waiters.upsert(&mut None, &new_waker(data.as_ref()));
}
