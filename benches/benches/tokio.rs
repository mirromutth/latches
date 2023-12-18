use std::sync::Arc;

use benches::tokio::Latch;
use criterion::{criterion_group, criterion_main};
use tokio::{
    runtime::{Builder, Runtime},
    time::Duration,
};

fn bench_count_down(c: &mut criterion::Criterion) {
    c.benchmark_group("Comparison-Tokio")
        .bench_function("count_down", |b| {
            b.iter_batched(
                || Latch::new(16),
                |l| l.count_down(),
                criterion::BatchSize::SmallInput,
            )
        });
}

fn bench_arrive(c: &mut criterion::Criterion) {
    c.benchmark_group("Comparison-Tokio")
        .bench_function("arrive", |b| {
            b.iter_batched(
                || Latch::new(16),
                |l| l.arrive(2),
                criterion::BatchSize::SmallInput,
            )
        });
}

fn bench_wait(c: &mut criterion::Criterion) {
    c.benchmark_group("Comparison-Tokio")
        .bench_function("wait", |b| {
            b.to_async(Builder::new_multi_thread().build().unwrap())
                .iter_batched(
                    || Latch::new(0),
                    |l| async move { l.wait().await },
                    criterion::BatchSize::SmallInput,
                )
        });
    case_wait(c, 1);
    case_wait(c, 1024);
}

fn bench_wait_timeout(c: &mut criterion::Criterion) {
    const DUR: Duration = Duration::from_millis(24);

    c.benchmark_group("Comparison-Tokio")
        .bench_function("wait_timeout", |b| {
            b.to_async(time_runtime()).iter_batched(
                || Latch::new(0),
                |l| async move { l.wait_timeout(DUR).await },
                criterion::BatchSize::SmallInput,
            )
        });
    case_wait_timeout(c, 1);
    case_wait_timeout(c, 1024);
}

fn bench_gate(c: &mut criterion::Criterion) {
    case_gate(c, 1);
    case_gate(c, 256);
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

fn case_wait(c: &mut criterion::Criterion, tasks: usize) {
    c.benchmark_group("Comparison-Tokio").bench_with_input(
        criterion::BenchmarkId::new("wait-overall", tasks),
        &tasks,
        |b, t| {
            b.to_async(Builder::new_multi_thread().build().unwrap())
                .iter_batched(
                    || {
                        let l = Arc::new(Latch::new(*t));

                        for _ in 0..*t {
                            let l = l.clone();
                            tokio::spawn(async move { l.count_down() });
                        }

                        l
                    },
                    |l| async move { l.wait().await },
                    criterion::BatchSize::SmallInput,
                )
        },
    );
}

fn case_wait_timeout(c: &mut criterion::Criterion, tasks: usize) {
    const DUR: Duration = Duration::from_millis(24);

    c.benchmark_group("Comparison-Tokio").bench_with_input(
        criterion::BenchmarkId::new("wait_timeout-overall", tasks),
        &tasks,
        |b, t| {
            b.to_async(time_runtime()).iter_batched(
                || {
                    let l = Arc::new(Latch::new(*t));

                    for _ in 0..*t {
                        let l = l.clone();
                        tokio::spawn(async move { l.count_down() });
                    }

                    (l, DUR)
                },
                |(l, d)| async move { l.wait_timeout(d).await },
                criterion::BatchSize::SmallInput,
            )
        },
    );
}

fn case_gate(c: &mut criterion::Criterion, tasks: usize) {
    c.benchmark_group("Comparison-Tokio").bench_with_input(
        criterion::BenchmarkId::new("gate-overall", tasks),
        &tasks,
        |b, t| {
            b.to_async(Builder::new_multi_thread().build().unwrap())
                .iter_batched(
                    || {
                        let l = Arc::new(Latch::new(1));
                        let d = Arc::new(Latch::new(*t));

                        for _ in 0..*t {
                            let l = l.clone();
                            let d = d.clone();

                            tokio::spawn(async move {
                                l.wait().await;
                                d.count_down();
                            });
                        }

                        l.count_down();

                        d
                    },
                    |d| async move { d.wait().await },
                    criterion::BatchSize::SmallInput,
                )
        },
    );
}

fn time_runtime() -> Runtime {
    Builder::new_multi_thread().enable_time().build().unwrap()
}
