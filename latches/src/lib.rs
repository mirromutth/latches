#![deny(
    missing_debug_implementations,
    missing_docs,
    rust_2018_idioms,
    unreachable_pub,
    elided_lifetimes_in_paths,
    clippy::missing_const_for_fn,
    clippy::missing_panics_doc,
    clippy::missing_safety_doc,
    clippy::missing_errors_doc
)]
#![doc(test(no_crate_inject, attr(deny(warnings, rust_2018_idioms))))]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(all(not(feature = "std"), not(test)), no_std)]

//! An implementation collection of latches.
//!
//! A latch is a downward counter which can be used to synchronize threads or
//! coordinate tasks. The value of the counter is initialized on creation.
//!
//! In contrast to [`Barrier`], it is a one-shot phenomenon, that mean the
//! counter will not be reset after reaching 0. Instead, it has the useful
//! property that it does not make them wait for the counter to reach 0 by
//! calling `count_down()` or `arrive()`. This also means that it can be
//! decremented by a participating thread/task more than once.
//!
//! [`Barrier`]: std::sync::Barrier

macro_rules! cfg_item {
    ($feature:literal => $m:item) => {
        #[cfg(feature = $feature)]
        #[cfg_attr(docsrs, doc(cfg(feature = $feature)))]
        $m
    };
    ($mt:meta => $($m:item)+) => {
        $(
        #[cfg($mt)]
        #[cfg_attr(docsrs, doc(cfg($mt)))]
        $m
        )+
    };
}

cfg_item! { "sync" =>
    /// The `sync` implementation is the default implementation of threads.
    ///
    /// It depends on the `atomic-wait` feature by default, enable the `std`
    /// feature will change this.
    ///
    /// It support timeouts if the `std` feature is enabled.
    pub mod sync;
}

cfg_item! { "futex" =>
    /// The `futex` implementation.
    ///
    /// It depends on the `atomic-wait` feature.
    ///
    /// It does not support timeouts.
    pub mod futex;
}

cfg_item! { "task" =>
    /// The `task` implementation.
    ///
    /// Adding the `atomic-wait` feature or `std` feature make it more
    /// concurrency-friendly for gate scenarios.
    ///
    /// It supports timeouts, but this requires a timer. See also [`watch()`].
    ///
    /// [`watch()`]: crate::task::Latch::watch
    pub mod task;
}

mod macros;

cfg_item! { any(all(feature = "sync", feature = "std"), feature = "task") =>
    pub use timeout::*;
    mod timeout;
}

cfg_item! { any(feature = "sync", feature = "task") =>
    mod lock;
}

#[cfg(all(feature = "task", not(feature = "std")))]
extern crate alloc;
