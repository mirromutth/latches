#[cfg(test)]
mod tests;

/// A type indicating whether a timed wait on a latch returned due to a time
/// out or not.
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum WaitTimeoutResult<T> {
    /// The current count has reached 0.
    Reached,
    /// The wait has timed out before the current count reaches 0.
    ///
    /// It contains the asynchronous timer result.
    TimedOut(T),
}

impl<T> WaitTimeoutResult<T> {
    /// Returns `true` if the current count was known to have reached 0 after
    /// the wait.
    ///
    /// # Examples
    ///
    /// ```
    /// use latches::WaitTimeoutResult;
    ///
    /// let x = WaitTimeoutResult::<()>::Reached;
    /// assert!(x.is_reached());
    ///
    /// let x = WaitTimeoutResult::TimedOut(());
    /// assert!(!x.is_reached());
    /// ```
    #[must_use]
    #[inline]
    pub const fn is_reached(&self) -> bool {
        matches!(self, WaitTimeoutResult::Reached)
    }

    /// Returns `true` if the wait was known to have timed out.
    ///
    /// # Examples
    ///
    /// ```
    /// use latches::WaitTimeoutResult;
    ///
    /// let x = WaitTimeoutResult::<()>::Reached;
    /// assert!(!x.is_timed_out());
    ///
    /// let x = WaitTimeoutResult::TimedOut(());
    /// assert!(x.is_timed_out());
    /// ```
    #[must_use]
    #[inline]
    pub const fn is_timed_out(&self) -> bool {
        matches!(self, WaitTimeoutResult::TimedOut(_))
    }

    /// Transforms the `WaitTimeoutResult<T>` into a [`Option<T>`], mapping
    /// [`Reached`] to [`None`] and [`TimedOut(t)`] to [`Some(t)`].
    ///
    /// [`Some(t)`]: Some
    /// [`Reached`]: WaitTimeoutResult::Reached
    /// [`TimedOut(t)`]: WaitTimeoutResult::TimedOut
    ///
    /// # Examples
    ///
    /// ```
    /// use latches::WaitTimeoutResult;
    ///
    /// let x = WaitTimeoutResult::<isize>::Reached;
    /// assert_eq!(x.timed_out(), None);
    ///
    /// let x = WaitTimeoutResult::TimedOut(2);
    /// assert_eq!(x.timed_out(), Some(2));
    /// ```
    #[must_use]
    #[inline]
    pub fn timed_out(self) -> Option<T> {
        match self {
            WaitTimeoutResult::Reached => None,
            WaitTimeoutResult::TimedOut(t) => Some(t),
        }
    }

    /// Transforms the `WaitTimeoutResult<T>` into a [`Result<V, T>`], mapping
    /// [`Reached`] to [`Ok(v)`] and [`TimedOut(t)`] to [`Err(t)`].
    ///
    /// Arguments passed to `ok` are eagerly evaluated; if you are passing the
    /// result of a function call, it is recommended to use [`ok_by`], which
    /// is lazily evaluated.
    ///
    /// # Errors
    ///
    /// This function will return an error with the content of [`TimedOut(t)`]
    /// if it is timed out.
    ///
    /// [`Ok(v)`]: Ok
    /// [`Err(t)`]: Err
    /// [`Reached`]: WaitTimeoutResult::Reached
    /// [`TimedOut(t)`]: WaitTimeoutResult::TimedOut
    /// [`ok_by`]: WaitTimeoutResult::ok_by
    ///
    /// # Examples
    ///
    /// ```
    /// use latches::WaitTimeoutResult;
    ///
    /// let x = WaitTimeoutResult::<()>::Reached;
    /// assert_eq!(x.ok(1), Ok(1));
    ///
    /// let x = WaitTimeoutResult::TimedOut("foo");
    /// assert_eq!(x.ok(2), Err("foo"));
    /// ```
    #[inline]
    pub fn ok<V>(self, v: V) -> Result<V, T> {
        match self {
            WaitTimeoutResult::Reached => Ok(v),
            WaitTimeoutResult::TimedOut(e) => Err(e),
        }
    }

    /// Transforms the `WaitTimeoutResult<T>` into a [`Result<V, T>`], mapping
    /// [`Reached`] to [`Ok(v)`] and [`TimedOut(t)`] to [`Err(t)`].
    ///
    /// [`Ok(v)`]: Ok
    /// [`Err(t)`]: Err
    /// [`Reached`]: WaitTimeoutResult::Reached
    /// [`TimedOut(t)`]: WaitTimeoutResult::TimedOut
    ///
    /// # Errors
    ///
    /// This function will return an error with the content of [`TimedOut(t)`]
    /// if it is timed out.
    ///
    /// # Examples
    ///
    /// ```
    /// use latches::WaitTimeoutResult;
    ///
    /// let x = WaitTimeoutResult::<()>::Reached;
    /// assert_eq!(x.ok_by(|| 1), Ok(1));
    ///
    /// let x = WaitTimeoutResult::TimedOut("bar");
    /// assert_eq!(x.ok_by(|| 2), Err("bar"));
    /// ```
    #[inline]
    pub fn ok_by<V, F: FnOnce() -> V>(self, v: F) -> Result<V, T> {
        match self {
            WaitTimeoutResult::Reached => Ok(v()),
            WaitTimeoutResult::TimedOut(e) => Err(e),
        }
    }
}
