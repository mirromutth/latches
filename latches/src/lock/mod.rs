macro_rules! cfg_impl {
    ($f:meta => $m:ident $(,)?) => {
        #[cfg(all(feature = "task", $f))]
        pub(crate) use $m::Mutex;

        #[cfg(all(feature = "sync", $f))]
        pub(crate) use $m::EmptyCondvar;

        #[cfg($f)]
        mod $m;
    };
}

cfg_impl!(feature = "std" => stds);
cfg_impl!(all(not(feature = "std"), feature = "atomic-wait") => futexes);
cfg_impl!(all(not(feature = "std"), not(feature = "atomic-wait")) => spins);
