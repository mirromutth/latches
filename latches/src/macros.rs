/// Decrements the counter. If the counter reaches 0, call the done method.
///
/// Do not to use `fetch_sub` for avoid undefined behavior due to overflow.
macro_rules! decrement {
    ($self:ident, $n:tt) => {
        let mut cr = $self.stat.load(Relaxed);

        while cr != 0 {
            let nc = if cr < $n { 0 } else { cr - $n };

            match $self.stat.compare_exchange_weak(cr, nc, Release, Relaxed) {
                Ok(_) => {
                    if nc == 0 {
                        $self.done();
                    }
                    break;
                }
                Err(x) => cr = x,
            }
        }
    };
}

/// Default implementation of `try_wait`.
macro_rules! once_try_wait {
    ($self:ident) => {
        match $self.stat.load(Acquire) {
            0 => Ok(()),
            s => Err(s),
        }
    };
}

macro_rules! spin_try_wait {
    ($self:ident, $stat:ident, $zero:literal, $($res:tt)+ $(,)?) => {
        let mut $stat = $self.stat.load(Acquire);

        for _ in 0..16 {
            if $stat == 0 {
                return $zero;
            }

            hint::spin_loop();

            $stat = $self.stat.load(Acquire);
        }

        return $($res)+
    };
}

pub(crate) use decrement;
pub(crate) use once_try_wait;
pub(crate) use spin_try_wait;
