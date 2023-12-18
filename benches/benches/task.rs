use std::sync::Arc;

use criterion::{criterion_group, criterion_main};
use latches::task::Latch;
use tokio::{
    runtime::{Builder, Runtime},
    time::{sleep, Duration},
};

fn bench_count_down(c: &mut criterion::Criterion) {
    c.benchmark_group("Task").bench_function("count_down", |b| {
        b.iter_batched(
            || Latch::new(16),
            |l| l.count_down(),
            criterion::BatchSize::SmallInput,
        )
    });
}

fn bench_arrive(c: &mut criterion::Criterion) {
    c.benchmark_group("Task").bench_function("arrive", |b| {
        b.iter_batched(
            || Latch::new(16),
            |l| l.arrive(2),
            criterion::BatchSize::SmallInput,
        )
    });
}

fn bench_wait(c: &mut criterion::Criterion) {
    c.benchmark_group("Task").bench_function("wait", |b| {
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

fn bench_watch(c: &mut criterion::Criterion) {
    const DUR: Duration = Duration::from_millis(24);

    c.benchmark_group("Task").bench_function("watch", |b| {
        b.to_async(time_runtime()).iter_batched(
            || (Latch::new(0), sleep(DUR)),
            |(l, t)| async move { l.watch(t).await },
            criterion::BatchSize::SmallInput,
        )
    });
    case_watch(c, 1);
    case_watch(c, 1024);
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
    bench_watch,
    bench_gate
);
criterion_main!(benches);

fn case_wait(c: &mut criterion::Criterion, tasks: usize) {
    c.benchmark_group("Task").bench_with_input(
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

fn case_watch(c: &mut criterion::Criterion, tasks: usize) {
    const DUR: Duration = Duration::from_millis(24);

    c.benchmark_group("Task").bench_with_input(
        criterion::BenchmarkId::new("watch-overall", tasks),
        &tasks,
        |b, t| {
            b.to_async(time_runtime()).iter_batched(
                || {
                    let l = Arc::new(Latch::new(*t));
                    for _ in 0..*t {
                        let l = l.clone();

                        tokio::spawn(async move { l.count_down() });
                    }

                    (l, sleep(DUR))
                },
                |(l, t)| async move { l.watch(t).await },
                criterion::BatchSize::SmallInput,
            )
        },
    );
}

fn case_gate(c: &mut criterion::Criterion, tasks: usize) {
    c.benchmark_group("Task").bench_with_input(
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
