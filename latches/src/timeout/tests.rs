use super::WaitTimeoutResult;

#[test]
fn test_is_reached() {
    assert!(WaitTimeoutResult::<()>::Reached.is_reached());
    assert!(WaitTimeoutResult::<i32>::Reached.is_reached());
    assert!(!WaitTimeoutResult::TimedOut(()).is_reached());
    assert!(!WaitTimeoutResult::TimedOut(1).is_reached());
}

#[test]
fn test_is_timed_out() {
    assert!(!WaitTimeoutResult::<()>::Reached.is_timed_out());
    assert!(!WaitTimeoutResult::<i32>::Reached.is_timed_out());
    assert!(WaitTimeoutResult::TimedOut(()).is_timed_out());
    assert!(WaitTimeoutResult::TimedOut(1).is_timed_out());
}

#[test]
fn test_ok() {
    assert_eq!(WaitTimeoutResult::<()>::Reached.ok(1), Ok(1));
    assert_eq!(WaitTimeoutResult::<i32>::Reached.ok(1), Ok(1));
    assert_eq!(WaitTimeoutResult::TimedOut(()).ok(1), Err(()));
    assert_eq!(WaitTimeoutResult::TimedOut(2).ok(1), Err(2));
}

#[test]
fn test_ok_by() {
    const fn lazy_one() -> i32 {
        1
    }

    assert_eq!(WaitTimeoutResult::<()>::Reached.ok_by(lazy_one), Ok(1));
    assert_eq!(WaitTimeoutResult::<i32>::Reached.ok_by(lazy_one), Ok(1));
    assert_eq!(WaitTimeoutResult::TimedOut(()).ok_by(lazy_one), Err(()));
    assert_eq!(WaitTimeoutResult::TimedOut(2).ok_by(lazy_one), Err(2));
}

#[test]
fn test_timed_out() {
    assert_eq!(WaitTimeoutResult::<()>::Reached.timed_out(), None);
    assert_eq!(WaitTimeoutResult::<i32>::Reached.timed_out(), None);
    assert_eq!(WaitTimeoutResult::TimedOut(()).timed_out(), Some(()));
    assert_eq!(WaitTimeoutResult::TimedOut(2).timed_out(), Some(2));
}

#[test]
fn test_clone() {
    let s = WaitTimeoutResult::TimedOut("test".to_string());
    let r = s.clone();

    assert_eq!(r, s);
}

#[test]
fn test_copy() {
    let s = WaitTimeoutResult::TimedOut(1);
    let r = s;

    assert_eq!(r, s);
}

#[test]
fn test_debug() {
    let s = format!("{:?}", WaitTimeoutResult::<()>::Reached);
    assert!(s.contains("Reached"));

    let s = format!("{:?}", WaitTimeoutResult::TimedOut(3));

    assert!(s.contains("TimedOut"));
    assert!(s.contains('3'));
}
