use criterion::{BatchSize, BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use nebula_quanta::{
    config::{InitProfile, MassProfile},
    direct::{
        compute_direct_accel, compute_direct_accel_with_g_scalar,
        compute_direct_accel_with_g_unrolled,
    },
    particle::ParticleSoa,
    sim::{build_tree, compute_accel_barnes_hut_with_threshold, preflight_node_capacity},
    tree::QuadTree,
};

const SEED: u64 = 42;
const THETA: f64 = 0.7;
const EPSILON: f64 = 0.01;
const G: f64 = 1.0;

fn plummer_particles(n: usize) -> ParticleSoa {
    ParticleSoa::random_with_profiles(
        n,
        SEED,
        InitProfile::Plummer,
        1.0,
        1.0,
        0.05,
        1.0,
        0.0,
        0.0,
        MassProfile::Uniform,
        1.0,
        0.25,
        0.5,
        2.0,
        2.0,
    )
}

fn tree_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("tree build");
    group.sample_size(20);
    for n in [1_000usize, 10_000] {
        let particles = plummer_particles(n);
        let capacity = preflight_node_capacity(n).expect("node capacity");
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter_batched(
                || QuadTree::with_capacity(capacity),
                |mut tree| build_tree(black_box(&mut tree), black_box(&particles)).unwrap(),
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

fn bh_force_eval(c: &mut Criterion) {
    let mut group = c.benchmark_group("bh_force");
    group.sample_size(10);
    for n in [1_000usize, 10_000, 100_000] {
        let particles = plummer_particles(n);
        let capacity = preflight_node_capacity(n).expect("node capacity");
        let mut tree = QuadTree::with_capacity(capacity);
        build_tree(&mut tree, &particles).unwrap();
        for threads in [1usize, 12] {
            group.bench_with_input(
                BenchmarkId::new(format!("threads={threads}"), n),
                &(n, threads),
                |b, (_, threads)| {
                    let mut ax = vec![0.0; n];
                    let mut ay = vec![0.0; n];
                    let mut stack = Vec::with_capacity(capacity);
                    let mut thread_stacks: Vec<Vec<usize>> = Vec::new();
                    b.iter(|| {
                        compute_accel_barnes_hut_with_threshold(
                            black_box(&particles),
                            black_box(&tree),
                            black_box(THETA),
                            black_box(EPSILON),
                            black_box(G),
                            black_box(*threads),
                            black_box(1),
                            black_box(&mut stack),
                            black_box(&mut thread_stacks),
                            black_box(&mut ax),
                            black_box(&mut ay),
                        )
                        .unwrap()
                    });
                },
            );
        }
    }
    group.finish();
}

fn direct_force_eval(c: &mut Criterion) {
    let mut group = c.benchmark_group("direct force eval");
    group.sample_size(10);
    for n in [1_000usize, 4_000] {
        let particles = plummer_particles(n);
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            let mut ax = vec![0.0; n];
            let mut ay = vec![0.0; n];
            b.iter(|| {
                compute_direct_accel(
                    black_box(&particles),
                    black_box(EPSILON),
                    black_box(&mut ax),
                    black_box(&mut ay),
                )
            });
        });
    }

    let n = 4_096usize;
    let particles = plummer_particles(n);
    for (name, kernel) in [
        (
            "scalar-n4096",
            compute_direct_accel_with_g_scalar
                as fn(&ParticleSoa, f64, f64, &mut [f64], &mut [f64]),
        ),
        ("unrolled-n4096", compute_direct_accel_with_g_unrolled),
    ] {
        group.bench_with_input(BenchmarkId::new(name, n), &n, |b, _| {
            let mut ax = vec![0.0; n];
            let mut ay = vec![0.0; n];
            b.iter(|| {
                kernel(
                    black_box(&particles),
                    black_box(EPSILON),
                    black_box(G),
                    black_box(&mut ax),
                    black_box(&mut ay),
                )
            });
        });
    }
    group.finish();
}

criterion_group!(benches, tree_build, bh_force_eval, direct_force_eval);
criterion_main!(benches);
