#![cfg(feature = "comparison")]

pub mod async_std;
pub mod mutex;
pub mod tokio;

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
pub(crate) use spin_try_wait;
