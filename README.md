# Latches

[![Crates.io](https://img.shields.io/crates/d/latches)](https://crates.io/crates/latches)
[![docs.rs](https://img.shields.io/docsrs/latches/latest)](https://docs.rs/latches)
[![build status](https://img.shields.io/github/actions/workflow/status/mirromutth/latches/tests.yml?style=flat-square&logo=github)](https://github.com/mirromutth/latches/actions/workflows/tests.yml)
[![codecov](https://codecov.io/github/mirromutth/latches/graph/badge.svg?token=0F0XY09UVG)](https://codecov.io/github/mirromutth/latches)
[![license](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue?style=flat-square)](#license)

A latch is a downward counter which can be used to synchronize threads or coordinate tasks. The value of the counter is initialized on creation. Threads/tasks may block/suspend on the latch until the counter is decremented to 0.

In contrast to [`std::sync::Barrier`][std-barrier], it is a one-shot phenomenon, that mean the counter will not be reset after reaching 0. Instead, it has the useful property that it does not make them wait for the counter to reach 0 by calling `count_down()` or `arrive()`. This also means that it can be decremented by a participating thread/task more than once.

See also [`std::latch`][cpp-latch] in C++ 20, [`java.util.concurrent.CountDownLatch`][java-latch] in Java, [`Concurrent::CountDownLatch`][ruby-latch] in [`concurrent-ruby`][concurrent-ruby].

## Quick Start

The `sync` implementation with `atomic-wait` by default features:

```sh
cargo add latches
```

The `task` implementation with `std`:

```sh
cargo add latches --no-default-features --features task --features std
```

The `futex` implementation:

```sh
cargo add latches --no-default-features --features futex
```

See also [Which One Should Be Used?](#which-one-should-be-used)

It can be used with `no_std` when the `std` feature is not enabled.

## Usage

### Wait For Completion

```rust
use std::{sync::Arc, thread};

// Naming rule: `latches::{implementation-name}::Latch`.
use latches::sync::Latch;

let latch = Arc::new(Latch::new(10));

for _ in 0..10 {
    let latch = latch.clone();
    thread::spawn(move || latch.count_down());
}

// Waits 10 threads complete their works.
// Requires `.await` if it is the `task` implementation.
latch.wait();
```

### Gate

```rust
use std::{sync::Arc, thread};

// Naming rule: `latches::{implementation-name}::Latch`.
use latches::sync::Latch;

let gate = Arc::new(Latch::new(1));

for _ in 0..10 {
    let gate = gate.clone();
    thread::spawn(move || {
        // Waits for the gate signal.
        // Requires `.await` if it is the `task` implementation.
        gate.wait();
        // Do some work after gate.
    });
}

// Allows 10 threads start their works.
gate.count_down();
```

## Implementations

### Sync

The `sync` implementation is the default implementation of threads.

Feature dependencies:

- Add `std` feature will make it use [`std::sync::Mutex`][std-mutex] and [`std::sync::Condvar`][std-condvar] as condition variables, it supports timeouts
- If `std` is disabled, add `atomic-wait` feature will make it use [`atomic-wait`][repo-atomic-wait] as condition variables, it does not support timeouts
- If both `std` and `atomic-wait` are disabled, it will throw a compile error

Both `atomic-wait` and `sync` are enabled in default features for easy-to-use. So if you want to use `sync` with `std` and don't want to import the unnecessary crate `atomic-wait`, please disable default features.

### Futex

The `futex` implementation is similar to popular implementations of C++20 [`std::latch`][cpp-latch], which provides slightly better performance compared to the `sync` implementation.

It does not support timeouts for waiting.

Feature dependencies:

- It depends on feature `atomic-wait` and crate [`atomic-wait`][repo-atomic-wait], and cannot be disabled

### Task

The `task` implementation is typically used to coordinate asynchronous tasks.

It requires extern crate `alloc` if in `no_std`.

Feature dependencies:

- Add `std` feature will make it use [`std::sync::Mutex`][std-mutex] as thread mutexes on waker collection
- If `std` is disabled, add `atomic-wait` feature will make it use [`atomic-wait`][atomic-wait] as thread mutexes on waker collection
- If both `std` and `atomic-wait` are disabled, it will use spinlocks as thread mutexes on waker collection

### Similarities and differences

Similarities:

- All implementations are based on atomic, so will not work on platforms that do not support atomic, i.e. `#[cfg(target_has_atomic)]`
- All implementations can be used on `no_std` if the `std` feature does not be enabled
- All methods except waits: do not need `async`/`await`

Differences:

|              | `sync`                 | `task`                            | `futex`       |
|-------------:|------------------------|-----------------------------------|---------------|
| counter type | `usize`                | `usize`                           | `u32`         |
|      mutexes | `std` or `atomic-wait` | `std`, `atomic-wait`, or spinlock | No mutex\*    |
|        waits | Blocking               | Futures                           | Blocking      |
|     timeouts | Requries `std`         | Requries an async timer           | Not support   |

\* No mutex doesn't mean it doesn't need to eliminate race conditions, in fact it uses Futex (i.e. [atomic-wait][repo-atomic-wait]) instead

### Which One Should Be Used?

If your project is using `async`/`await` tasks, use the `task` implementation. Add it with `std` or `atomic-wait` feature might make it more concurrency-friendly for [gate](#gate) scenarios. Like following:

```sh
cargo add latches --no-default-features --features task --feature atomic-wait
```

If the amount of concurrency in your project is small, use the `futex` implementation. Like following:

```sh
cargo add latches --no-default-features --features futex
```

Otherwise, use the `sync` implementation. It has the same counter type `usize` as the `task` implementation and [`std::sync::Barrier`][std-barrier]. Add it with `std` feature will make it supports timeouts. Note that it should be used with one of the `std` or `atomic-wait` features, otherwise a compilation error will be thrown. Like following:

```sh
# Both `sync` and `atomic-wait` are enabled in default features
cargo add latches
```

Or enable `std` feature for timeouts support:

```sh
cargo add latches --no-default-features --features sync --features std
```

Under a large amount of concurrency, there is no obvious performance gap between the `futex` implementation and the `sync` implementation.

Additionally, if you are migrating C++ code to Rust, using a `futex` implementation may be an approach which makes more conservative, e.g. similar memory usage, ABI calls, etc. Note that the `futex` implementation has no undefined behavior, which is not like the [`std::latch`][cpp-latch] in C++.

## Performance

### Run Benchmarks

Run benchmarks for all implementations with `atomic-wait` (the `futex` implementation depends on `atomic-wait`):

```sh
cargo bench --package benches
```

Run benchmarks with the `sync` implementation with `std`:

```sh
cargo bench --package benches --no-default-features --features sync --features std
```

Run benchmarks with the `task` implementation with `atomic-wait`:

```sh
cargo bench --package benches --no-default-features --features task --features atomic-wait
```

Or run benchmarks with `std` and comparison group:

```sh
cargo bench --package benches --features std --features comparison
```

etc.

Overall benchmarks include thread scheduling overhead, and `Latch` is much faster than thread scheduling, so there may be timing jitter and large standard deviations. All overall benchmarks will have name postfix `-overall`.

The asynchronous comparison groups are also atomic-based and depend on specific async libraries such as `tokio` and `async-std`.

The synchronous comparison group uses [`Mutex`][std-mutex] state instead of atomic.

## License

Latches is released under the terms of either the [MIT License][mit-license] or the [Apache License Version 2.0][apache-license], at your option.

[cpp-latch]: https://en.cppreference.com/w/cpp/thread/latch
[java-latch]: https://docs.oracle.com/en/java/javase/21/docs/api/java.base/java/util/concurrent/CountDownLatch.html
[ruby-latch]: https://www.rubydoc.info/gems/concurrent-ruby/Concurrent/CountDownLatch
[concurrent-ruby]: https://github.com/ruby-concurrency/concurrent-ruby
[std-barrier]: https://doc.rust-lang.org/std/sync/struct.Barrier.html
[std-mutex]: https://doc.rust-lang.org/std/sync/struct.Mutex.html
[std-condvar]: https://doc.rust-lang.org/std/sync/struct.Condvar.html
[repo-atomic-wait]: https://github.com/m-ou-se/atomic-wait
[mit-license]: https://mit-license.org
[apache-license]: https://www.apache.org/licenses/LICENSE-2.0
