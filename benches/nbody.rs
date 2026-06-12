use criterion::{BatchSize, BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use rayon::prelude::*;

use nebula_quanta::{
    config::{InitProfile, MassProfile},
    direct::{
        compute_direct_accel, compute_direct_accel_with_g_scalar,
        compute_direct_accel_with_g_unrolled,
    },
    particle::ParticleSoa,
    sim::{
        build_tree, build_tree_with_threads, compute_accel_barnes_hut_with_threshold,
        preflight_node_capacity,
    },
    tree::{Node, QuadTree},
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
    for n in [100_000usize, 10_000, 1_000] {
        let particles = plummer_particles(n);
        let capacity = preflight_node_capacity(n).expect("node capacity");
        for threads in [12usize, 1] {
            group.bench_with_input(
                BenchmarkId::new(format!("threads={threads}"), n),
                &(n, threads),
                |b, (_, threads)| {
                    b.iter_batched(
                        || QuadTree::with_capacity(capacity),
                        |mut tree| {
                            build_tree_with_threads(
                                black_box(&mut tree),
                                black_box(&particles),
                                black_box(*threads),
                            )
                            .unwrap()
                        },
                        BatchSize::SmallInput,
                    );
                },
            );
        }
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

struct ParticleSoaF32Mirror {
    x: Vec<f32>,
    y: Vec<f32>,
    m: Vec<f32>,
}

impl ParticleSoaF32Mirror {
    fn from_particles(particles: &ParticleSoa) -> Self {
        Self {
            x: particles.x.iter().map(|value| *value as f32).collect(),
            y: particles.y.iter().map(|value| *value as f32).collect(),
            m: particles.m.iter().map(|value| *value as f32).collect(),
        }
    }

    fn len(&self) -> usize {
        self.x.len()
    }
}

#[derive(Clone, Copy)]
struct NodeF32Mirror {
    x_min: f32,
    x_max: f32,
    mass: f32,
    com_x: f32,
    com_y: f32,
    body_idx: i32,
    children: [i32; 4],
}

impl NodeF32Mirror {
    fn from_node(node: &Node) -> Self {
        Self {
            x_min: node.x_min as f32,
            x_max: node.x_max as f32,
            mass: node.mass as f32,
            com_x: node.com_x as f32,
            com_y: node.com_y as f32,
            body_idx: node.body_idx,
            children: node.children,
        }
    }

    // Mirrors production `Node::size()` exactly (x extent only) so both
    // kernels apply the same opening criterion to the same tree.
    fn size(self) -> f32 {
        self.x_max - self.x_min
    }
}

#[allow(clippy::too_many_arguments)]
fn compute_accel_barnes_hut_f32_candidate(
    particles: &ParticleSoaF32Mirror,
    nodes: &[NodeF32Mirror],
    theta: f64,
    epsilon: f64,
    g: f64,
    thread_count: usize,
    pool: Option<&rayon::ThreadPool>,
    stack: &mut Vec<usize>,
    ax: &mut [f32],
    ay: &mut [f32],
) -> Result<(), String> {
    let n = particles.len();
    if ax.len() < n || ay.len() < n {
        return Err(format!(
            "f32 acceleration buffer too short: n={n}, ax_len={}, ay_len={}",
            ax.len(),
            ay.len()
        ));
    }
    let theta2 = (theta * theta) as f32;
    let eps2 = (epsilon * epsilon) as f32;
    let g = g as f32;
    let active_threads = thread_count.max(1).min(n.max(1));
    if active_threads <= 1 {
        for i in 0..n {
            let (fx, fy) =
                compute_particle_force_f32_candidate(i, particles, nodes, theta2, eps2, g, stack);
            ax[i] = fx;
            ay[i] = fy;
        }
        return Ok(());
    }

    let pool = pool.ok_or_else(|| "f32 candidate parallel pool missing".to_string())?;
    pool.install(|| {
        ax[..n]
            .par_chunks_mut(64)
            .zip(ay[..n].par_chunks_mut(64))
            .enumerate()
            .for_each_init(
                || Vec::with_capacity(4096),
                |local_stack, (chunk_idx, (chunk_ax, chunk_ay))| {
                    let start = chunk_idx * 64;
                    for (offset, (ax_slot, ay_slot)) in
                        chunk_ax.iter_mut().zip(chunk_ay.iter_mut()).enumerate()
                    {
                        let particle_idx = start + offset;
                        let (fx, fy) = compute_particle_force_f32_candidate(
                            particle_idx,
                            particles,
                            nodes,
                            theta2,
                            eps2,
                            g,
                            local_stack,
                        );
                        *ax_slot = fx;
                        *ay_slot = fy;
                    }
                },
            );
    });
    Ok(())
}

fn compute_particle_force_f32_candidate(
    i: usize,
    particles: &ParticleSoaF32Mirror,
    nodes: &[NodeF32Mirror],
    theta2: f32,
    eps2: f32,
    g: f32,
    stack: &mut Vec<usize>,
) -> (f32, f32) {
    let mut force_x = 0.0f32;
    let mut force_y = 0.0f32;
    let xi = particles.x[i];
    let yi = particles.y[i];

    stack.clear();
    stack.push(0);
    while let Some(node_idx) = stack.pop() {
        let node = nodes[node_idx];
        if node.mass <= 0.0 {
            continue;
        }
        let dx = node.com_x - xi;
        let dy = node.com_y - yi;
        let dist2_soft = dx * dx + dy * dy + eps2;
        if dist2_soft <= 0.0 {
            continue;
        }
        if node.body_idx >= 0 {
            let body = node.body_idx as usize;
            if body != i {
                let inv_r3 = 1.0 / (dist2_soft * dist2_soft.sqrt());
                let coeff = g * particles.m[body] * inv_r3;
                force_x += coeff * dx;
                force_y += coeff * dy;
            }
            continue;
        }
        let size = node.size();
        if size * size <= theta2 * dist2_soft {
            let inv_r3 = 1.0 / (dist2_soft * dist2_soft.sqrt());
            let coeff = g * node.mass * inv_r3;
            force_x += coeff * dx;
            force_y += coeff * dy;
            continue;
        }
        for child in &node.children {
            if *child >= 0 {
                stack.push(*child as usize);
            }
        }
    }
    (force_x, force_y)
}

fn bh_force_precision_decision(c: &mut Criterion) {
    let mut group = c.benchmark_group("bh_force_precision_decision");
    group.sample_size(10);
    let n = 100_000usize;
    let particles = plummer_particles(n);
    let capacity = preflight_node_capacity(n).expect("node capacity");
    let mut tree = QuadTree::with_capacity(capacity);
    build_tree(&mut tree, &particles).unwrap();
    let particles_f32 = ParticleSoaF32Mirror::from_particles(&particles);
    let nodes_f32: Vec<NodeF32Mirror> = tree.nodes.iter().map(NodeF32Mirror::from_node).collect();
    for threads in [1usize, 12] {
        group.bench_with_input(
            BenchmarkId::new(format!("f64_threads={threads}"), n),
            &(n, threads),
            |b, (_, threads)| {
                let mut ax = vec![0.0; n];
                let mut ay = vec![0.0; n];
                let mut stack = Vec::with_capacity(capacity);
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
                        black_box(&mut ax),
                        black_box(&mut ay),
                    )
                    .unwrap()
                });
            },
        );
        group.bench_with_input(
            BenchmarkId::new(format!("f32_threads={threads}"), n),
            &(n, threads),
            |b, (_, threads)| {
                let mut ax = vec![0.0f32; n];
                let mut ay = vec![0.0f32; n];
                let mut stack = Vec::with_capacity(capacity);
                let pool = if *threads > 1 {
                    Some(
                        rayon::ThreadPoolBuilder::new()
                            .num_threads(*threads)
                            .build()
                            .expect("f32 candidate pool"),
                    )
                } else {
                    None
                };
                b.iter(|| {
                    compute_accel_barnes_hut_f32_candidate(
                        black_box(&particles_f32),
                        black_box(&nodes_f32),
                        black_box(THETA),
                        black_box(EPSILON),
                        black_box(G),
                        black_box(*threads),
                        pool.as_ref(),
                        black_box(&mut stack),
                        black_box(&mut ax),
                        black_box(&mut ay),
                    )
                    .unwrap()
                });
            },
        );
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

criterion_group!(
    benches,
    tree_build,
    bh_force_eval,
    bh_force_precision_decision,
    direct_force_eval
);
criterion_main!(benches);
