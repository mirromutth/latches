use std::{sync::Arc, thread, time::Duration};

use benches::mutex::Latch;
use criterion::{criterion_group, criterion_main};

fn bench_count_down(c: &mut criterion::Criterion) {
    c.benchmark_group("Comparison-Mutex")
        .bench_function("count_down", |b| {
            b.iter_batched(
                || Latch::new(16),
                |l| l.count_down(),
                criterion::BatchSize::SmallInput,
            )
        });
}

fn bench_arrive(c: &mut criterion::Criterion) {
    c.benchmark_group("Comparison-Mutex")
        .bench_function("arrive", |b| {
            b.iter_batched(
                || Latch::new(16),
                |l| l.arrive(2),
                criterion::BatchSize::SmallInput,
            )
        });
}

fn bench_wait(c: &mut criterion::Criterion) {
    c.benchmark_group("Comparison-Mutex")
        .bench_function("wait", |b| {
            b.iter_batched(
                || Latch::new(0),
                |l| l.wait(),
                criterion::BatchSize::SmallInput,
            )
        });
    case_wait(c, 1);
    case_wait(c, 32);
}

fn bench_wait_timeout(c: &mut criterion::Criterion) {
    const DUR: Duration = Duration::from_millis(24);

    c.benchmark_group("Comparison-Mutex")
        .bench_function("wait_timeout", |b| {
            b.iter_batched(
                || Latch::new(0),
                |l| l.wait_timeout(DUR),
                criterion::BatchSize::SmallInput,
            )
        });
    case_wait_timeout(c, 1);
    case_wait_timeout(c, 32);
}

fn bench_gate(c: &mut criterion::Criterion) {
    case_gate(c, 1);
    case_gate(c, 16);
}

criterion_group!(
    benches,
    bench_count_down,
    bench_arrive,
    bench_wait,
    bench_wait_timeout,
    bench_gate
);
criterion_main!(benches);

fn case_wait(c: &mut criterion::Criterion, threads: usize) {
    c.benchmark_group("Comparison-Mutex").bench_with_input(
        criterion::BenchmarkId::new("wait-overall", threads),
        &threads,
        |b, t| {
            b.iter_batched(
                || {
                    let l = Arc::new(Latch::new(*t));

                    for _ in 0..*t {
                        let l = l.clone();
                        thread::spawn(move || l.count_down());
                    }

                    l
                },
                |l| l.wait(),
                criterion::BatchSize::SmallInput,
            )
        },
    );
}

fn case_wait_timeout(c: &mut criterion::Criterion, threads: usize) {
    const DUR: Duration = Duration::from_millis(24);

    c.benchmark_group("Comparison-Mutex").bench_with_input(
        criterion::BenchmarkId::new("wait_timeout-overall", threads),
        &threads,
        |b, t| {
            b.iter_batched(
                || {
                    let l = Arc::new(Latch::new(*t));

                    for _ in 0..*t {
                        let l = l.clone();
                        thread::spawn(move || l.count_down());
                    }

                    l
                },
                |l| l.wait_timeout(DUR),
                criterion::BatchSize::SmallInput,
            )
        },
    );
}

fn case_gate(c: &mut criterion::Criterion, threads: usize) {
    c.benchmark_group("Comparison-Mutex").bench_with_input(
        criterion::BenchmarkId::new("gate-overall", threads),
        &threads,
        |b, t| {
            b.iter_batched(
                || {
                    let l = Arc::new(Latch::new(1));
                    let d = Arc::new(Latch::new(*t));

                    for _ in 0..*t {
                        let l = l.clone();
                        let d = d.clone();

                        thread::spawn(move || {
                            l.wait();
                            d.count_down();
                        });
                    }

                    l.count_down();

                    d
                },
                |d| d.wait(),
                criterion::BatchSize::SmallInput,
            )
        },
    );
}
