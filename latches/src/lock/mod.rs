#[cfg(all(feature = "task", feature = "std"))]
pub(crate) use stds::Mutex;
#[cfg(all(feature = "sync", feature = "std"))]
pub(crate) use stds::EmptyCondvar;
#[cfg(feature = "std")]
mod stds;

#[cfg(all(feature = "task", all(not(feature = "std"), feature = "atomic-wait")))]
pub(crate) use futexes::Mutex;
#[cfg(all(feature = "sync", all(not(feature = "std"), feature = "atomic-wait")))]
pub(crate) use futexes::EmptyCondvar;
#[cfg(all(not(feature = "std"), feature = "atomic-wait"))]
mod futexes;

#[cfg(all(feature = "task", all(not(feature = "std"), not(feature = "atomic-wait"))))]
pub(crate) use spins::Mutex;
#[cfg(all(feature = "sync", all(not(feature = "std"), not(feature = "atomic-wait"))))]
pub(crate) use spins::EmptyCondvar;
#[cfg(all(not(feature = "std"), not(feature = "atomic-wait")))]
mod spins;
