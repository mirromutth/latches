#![no_std]

#[cfg(test)]
mod tests {

    #[test]
    fn test_futex_no_std() {
        use latches::futex::Latch;

        let latch = Latch::new(1);

        latch.count_down();
        assert_eq!(latch.try_wait(), Ok(()));
    }

    #[test]
    fn test_sync_no_std() {
        use latches::sync::Latch;

        let latch = Latch::new(1);

        latch.count_down();
        assert_eq!(latch.try_wait(), Ok(()));
    }

    #[test]
    fn test_task_no_std() {
        use latches::task::Latch;

        let latch = Latch::new(1);

        latch.count_down();
        assert_eq!(latch.try_wait(), Ok(()));
    }
}
