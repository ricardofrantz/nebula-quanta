use std::collections::HashMap;
use std::mem::size_of;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use rayon::prelude::*;

use crate::{
    config::Args,
    direct::{compute_direct_accel_with_g, run_direct as run_direct_reference},
    frame::FrameRecorder,
    particle::{ParticleSoa, particle_bounds},
    stats::RunStats,
    tree::{Node, QuadTree},
};

// Historical compatibility constant for tests/benchmarks that pass a threshold
// into helper functions. Barnes-Hut force evaluation no longer has a small-N
// serialization path: every `--threads > 1` run uses the persistent rayon pool,
// while `--threads 1` remains strictly serial.
pub const MIN_PARALLEL_BH_PARTICLES: usize = 1;

// Measured 2026-06-12 on nexus-dev (AMD Ryzen 9 9900X): one-step Plummer
// build_ms at N=12,000 was serial 2.716 ms vs 12T 2.769 ms; at N=16,000 it
// was serial 3.557 ms vs 12T 3.434 ms. Route builds below 16k to serial.
pub const MIN_PARALLEL_TREE_BUILD_PARTICLES: usize = 16_000;

const PARTICLE_CHUNK_LEN: usize = 64;
const TRAVERSAL_STACK_CAPACITY: usize = 4_096;

fn effective_bh_force_threads(
    requested_threads: usize,
    particle_count: usize,
    _min_parallel_particles: usize,
) -> usize {
    requested_threads.max(1).min(particle_count.max(1))
}

pub fn compute_barnes_hut_accel_snapshot(
    particles: &ParticleSoa,
    args: &Args,
) -> Result<(Vec<f64>, Vec<f64>), String> {
    compute_barnes_hut_accel_snapshot_with_threshold(particles, args, MIN_PARALLEL_BH_PARTICLES)
}

fn compute_barnes_hut_accel_snapshot_with_threshold(
    particles: &ParticleSoa,
    args: &Args,
    min_parallel_particles: usize,
) -> Result<(Vec<f64>, Vec<f64>), String> {
    let n = particles.len();
    if n == 0 {
        return Ok((Vec::new(), Vec::new()));
    }

    if args.theta <= 0.0 {
        let mut direct_ax = vec![0.0; n];
        let mut direct_ay = vec![0.0; n];
        let epsilon = args.epsilon_for_step(0, n, particle_bounds(particles).ok());
        compute_direct_accel_with_g(particles, epsilon, args.g, &mut direct_ax, &mut direct_ay);
        return Ok((direct_ax, direct_ay));
    }

    let active_threads = effective_bh_force_threads(args.threads, n, min_parallel_particles);
    let node_capacity = preflight_node_capacity(n)?;
    let mut tree = QuadTree::with_capacity(node_capacity);
    let mut ax = vec![0.0; n];
    let mut ay = vec![0.0; n];
    let mut stack = Vec::with_capacity(node_capacity);
    build_tree_with_threads(&mut tree, particles, args.threads)?;
    let effective_theta = args.theta_for_step(0, n, tree.root_bounds());
    let epsilon = args.epsilon_for_step(0, n, tree.root_bounds());
    compute_accel_barnes_hut_with_threshold_impl(
        particles,
        &tree,
        effective_theta,
        epsilon,
        args.g,
        active_threads,
        min_parallel_particles,
        &mut stack,
        &mut ax,
        &mut ay,
    )?;

    Ok((ax, ay))
}

pub fn run_barnes_hut(
    particles: &mut ParticleSoa,
    args: &Args,
    recorder: Option<&mut FrameRecorder>,
) -> Result<RunStats, String> {
    run_barnes_hut_with_threshold(particles, args, recorder, MIN_PARALLEL_BH_PARTICLES)
}

fn run_barnes_hut_with_threshold(
    particles: &mut ParticleSoa,
    args: &Args,
    recorder: Option<&mut FrameRecorder>,
    min_parallel_particles: usize,
) -> Result<RunStats, String> {
    if particles.is_empty() {
        return Ok(RunStats::zero());
    }

    if args.theta <= 0.0 {
        return run_direct_reference(particles, args, None);
    }

    let n = particles.len();
    let active_threads = effective_bh_force_threads(args.threads, n, min_parallel_particles);
    let node_capacity = preflight_node_capacity(n)?;
    let particle_state_bytes = particle_state_bytes(n);
    let node_pool_bytes = node_pool_bytes(node_capacity);
    let traversal_stack_bytes = traversal_stack_bytes(node_capacity, active_threads);
    let tree_build_transient_bytes = parallel_tree_build_transient_bytes(n, active_threads)?;
    let workspace_bytes =
        particle_state_bytes + node_pool_bytes + traversal_stack_bytes + tree_build_transient_bytes;
    check_memory_budget(args, workspace_bytes)?;

    let mut tree = QuadTree::with_capacity(node_capacity);
    let mut ax = vec![0.0; n];
    let mut ay = vec![0.0; n];
    let mut traversal = Vec::with_capacity(node_capacity);
    let mut recorder = recorder;
    let mut rk2_particles = particles.clone();
    let mut rk2_ax = vec![0.0; n];
    let mut rk2_ay = vec![0.0; n];

    let mut build_elapsed = 0.0;
    let mut force_elapsed = 0.0;
    let mut integrate_elapsed = 0.0;
    let mut peak_node_count = 0usize;
    let mut theta = args.theta_for_step(0, n, tree.root_bounds());
    let mut epsilon = args.epsilon_for_step(0, n, tree.root_bounds());

    let mut step_start = Instant::now();
    build_tree_with_threads(&mut tree, particles, args.threads)?;
    peak_node_count = peak_node_count.max(tree.nodes.len());
    build_elapsed += step_start.elapsed().as_secs_f64() * 1000.0;
    step_start = Instant::now();
    compute_accel_barnes_hut_with_threshold_impl(
        particles,
        &tree,
        theta,
        epsilon,
        args.g,
        active_threads,
        min_parallel_particles,
        &mut traversal,
        &mut ax,
        &mut ay,
    )?;
    force_elapsed += step_start.elapsed().as_secs_f64() * 1000.0;
    if let Some(recorder) = recorder.as_deref_mut()
        && let Some(bounds) = tree.root_bounds()
    {
        recorder.record_step(0, particles, bounds, &ax, &ay)?;
    }

    let mut step = 0;
    while step < args.steps {
        let mut t = Instant::now();
        match args.integrator {
            crate::config::Integrator::Leapfrog | crate::config::Integrator::Verlet => {
                for i in 0..n {
                    particles.vx[i] += 0.5 * ax[i] * args.dt;
                    particles.vy[i] += 0.5 * ay[i] * args.dt;
                    particles.x[i] += particles.vx[i] * args.dt;
                    particles.y[i] += particles.vy[i] * args.dt;
                }
            }
            crate::config::Integrator::Rk2 => {
                integrate_rk2_step(
                    particles,
                    &mut tree,
                    &mut rk2_particles,
                    theta,
                    epsilon,
                    args.g,
                    args.dt,
                    active_threads,
                    &ax,
                    &ay,
                    &mut rk2_ax,
                    &mut rk2_ay,
                    &mut traversal,
                    min_parallel_particles,
                )?;
            }
        }
        integrate_elapsed += t.elapsed().as_secs_f64() * 1000.0;

        t = Instant::now();
        build_tree_with_threads(&mut tree, particles, args.threads)?;
        peak_node_count = peak_node_count.max(tree.nodes.len());
        build_elapsed += t.elapsed().as_secs_f64() * 1000.0;
        theta = args.theta_for_step(step + 1, n, tree.root_bounds());
        epsilon = args.epsilon_for_step(step + 1, n, tree.root_bounds());
        t = Instant::now();
        compute_accel_barnes_hut_with_threshold_impl(
            particles,
            &tree,
            theta,
            epsilon,
            args.g,
            active_threads,
            min_parallel_particles,
            &mut traversal,
            &mut ax,
            &mut ay,
        )?;
        force_elapsed += t.elapsed().as_secs_f64() * 1000.0;
        if let Some(recorder) = recorder.as_deref_mut()
            && let Some(bounds) = tree.root_bounds()
        {
            recorder.record_step(step + 1, particles, bounds, &ax, &ay)?;
        }

        t = Instant::now();
        if args.integrator == crate::config::Integrator::Leapfrog
            || args.integrator == crate::config::Integrator::Verlet
        {
            for i in 0..n {
                particles.vx[i] += 0.5 * ax[i] * args.dt;
                particles.vy[i] += 0.5 * ay[i] * args.dt;
            }
        }
        integrate_elapsed += t.elapsed().as_secs_f64() * 1000.0;

        step += 1;
    }

    Ok(RunStats {
        build_ms: build_elapsed,
        force_ms: force_elapsed,
        integrate_ms: integrate_elapsed,
        peak_node_count,
        node_capacity,
        particle_count: n,
        particle_bytes: particle_state_bytes,
        node_pool_bytes,
        traversal_stack_bytes,
        tree_build_transient_bytes,
    })
}

#[allow(clippy::too_many_arguments)]
fn integrate_rk2_step(
    particles: &mut ParticleSoa,
    tree: &mut QuadTree,
    mid_particles: &mut ParticleSoa,
    theta: f64,
    epsilon: f64,
    g: f64,
    dt: f64,
    thread_count: usize,
    ax: &[f64],
    ay: &[f64],
    mid_ax: &mut [f64],
    mid_ay: &mut [f64],
    traversal: &mut Vec<usize>,
    min_parallel_particles: usize,
) -> Result<(), String> {
    let n = particles.len();
    for i in 0..n {
        let vx_half = particles.vx[i] + 0.5 * ax[i] * dt;
        let vy_half = particles.vy[i] + 0.5 * ay[i] * dt;
        mid_particles.x[i] = particles.x[i] + particles.vx[i] * 0.5 * dt;
        mid_particles.y[i] = particles.y[i] + particles.vy[i] * 0.5 * dt;
        mid_particles.vx[i] = vx_half;
        mid_particles.vy[i] = vy_half;
    }

    build_tree_with_threads(tree, mid_particles, thread_count)?;

    compute_accel_barnes_hut_with_threshold_impl(
        mid_particles,
        tree,
        theta,
        epsilon,
        g,
        thread_count,
        min_parallel_particles,
        traversal,
        mid_ax,
        mid_ay,
    )?;

    for i in 0..particles.len() {
        // Explicit midpoint RK2: advance both position and velocity using the
        // midpoint velocity/acceleration estimated from the start-of-step state.
        particles.x[i] += mid_particles.vx[i] * dt;
        particles.y[i] += mid_particles.vy[i] * dt;
        particles.vx[i] += mid_ax[i] * dt;
        particles.vy[i] += mid_ay[i] * dt;
    }
    Ok(())
}

pub fn build_tree(tree: &mut QuadTree, particles: &ParticleSoa) -> Result<(), String> {
    build_tree_serial(tree, particles)
}

/// Forces from the resulting tree are bitwise-identical to a serial build's;
/// node-pool numbering may differ (subtrees are appended bucket-by-bucket),
/// so consumers of `QuadTree.nodes` must not assume serial insertion order.
pub fn build_tree_with_threads(
    tree: &mut QuadTree,
    particles: &ParticleSoa,
    thread_count: usize,
) -> Result<(), String> {
    build_tree_with_threads_with_threshold(
        tree,
        particles,
        thread_count,
        MIN_PARALLEL_TREE_BUILD_PARTICLES,
    )
}

pub fn build_tree_with_threads_with_threshold(
    tree: &mut QuadTree,
    particles: &ParticleSoa,
    thread_count: usize,
    min_parallel_particles: usize,
) -> Result<(), String> {
    let active_threads = thread_count.max(1).min(particles.len().max(1));
    if active_threads <= 1 || particles.len() < 2 || particles.len() < min_parallel_particles {
        return build_tree_serial(tree, particles);
    }
    if build_tree_parallel_root_quadrants(tree, particles, active_threads).is_err() {
        // Per-bucket worker pools are sized from the bucket's particle count;
        // pathologically tight clusters can need deeper subdivision than that
        // allows. The serial build with the global pool is the backstop.
        return build_tree_serial(tree, particles);
    }
    Ok(())
}

fn build_tree_serial(tree: &mut QuadTree, particles: &ParticleSoa) -> Result<(), String> {
    let n = particles.len();
    if n == 0 {
        return Ok(());
    }

    let mut x_min = particles.x[0];
    let mut x_max = particles.x[0];
    let mut y_min = particles.y[0];
    let mut y_max = particles.y[0];

    for i in 1..n {
        let x = particles.x[i];
        let y = particles.y[i];
        if x < x_min {
            x_min = x;
        }
        if x > x_max {
            x_max = x;
        }
        if y < y_min {
            y_min = y;
        }
        if y > y_max {
            y_max = y;
        }
    }

    let spread = (x_max - x_min).max(y_max - y_min);
    let pad = if spread == 0.0 {
        1.0e-6
    } else {
        spread * 1e-12
    };
    tree.reset(x_min - pad, x_max + pad, y_min - pad, y_max + pad);

    for i in 0..n {
        insert_into_node(tree, particles, 0, i)?;
    }

    Ok(())
}

struct BuildBucket {
    node_idx: usize,
    body_indices: Vec<usize>,
}

struct BuildBucketWithBounds {
    bucket: BuildBucket,
    bounds: Node,
}

#[inline]
fn parallel_tree_prefix_depth(thread_count: usize) -> usize {
    let target_buckets = thread_count.max(1).saturating_mul(4);
    let mut depth = 0usize;
    let mut buckets = 1usize;
    while buckets < target_buckets {
        depth += 1;
        buckets = buckets.saturating_mul(4);
    }
    depth
}

#[inline]
fn build_tree_parallel_root_quadrants(
    tree: &mut QuadTree,
    particles: &ParticleSoa,
    thread_count: usize,
) -> Result<(), String> {
    let n = particles.len();
    if n == 0 {
        return Ok(());
    }

    let mut x_min = particles.x[0];
    let mut x_max = particles.x[0];
    let mut y_min = particles.y[0];
    let mut y_max = particles.y[0];

    for i in 1..n {
        let x = particles.x[i];
        let y = particles.y[i];
        if x < x_min {
            x_min = x;
        }
        if x > x_max {
            x_max = x;
        }
        if y < y_min {
            y_min = y;
        }
        if y > y_max {
            y_max = y;
        }
    }

    let spread = (x_max - x_min).max(y_max - y_min);
    let pad = if spread == 0.0 {
        1.0e-6
    } else {
        spread * 1e-12
    };
    tree.reset(x_min - pad, x_max + pad, y_min - pad, y_max + pad);

    let prefix_depth = parallel_tree_prefix_depth(thread_count);
    let mut body_indices: Vec<usize> = (0..n).collect();
    let mut scratch = vec![0usize; n];
    let mut leaf_buckets = Vec::new();
    build_tree_prefix(
        tree,
        particles,
        0,
        &mut body_indices,
        &mut scratch,
        prefix_depth,
        &mut leaf_buckets,
    )?;

    let mut leaf_buckets: Vec<BuildBucketWithBounds> = leaf_buckets
        .into_iter()
        .map(|bucket| BuildBucketWithBounds {
            bounds: tree.nodes[bucket.node_idx],
            bucket,
        })
        .collect();
    leaf_buckets.sort_by_key(|bucket| bucket.bucket.node_idx);

    let pool = rayon_pool(thread_count)?;
    let subtrees: Result<Vec<(usize, QuadTree)>, String> = pool.install(|| {
        leaf_buckets
            .into_par_iter()
            .map(|bucket| {
                let node_idx = bucket.bucket.node_idx;
                let bounds = bucket.bounds;
                let subtree_capacity =
                    preflight_node_capacity(bucket.bucket.body_indices.len())?.saturating_add(1024);
                let mut subtree = QuadTree::with_capacity(subtree_capacity);
                subtree.reset(bounds.x_min, bounds.x_max, bounds.y_min, bounds.y_max);
                fill_subtree_from_ordered_bodies(
                    &mut subtree,
                    particles,
                    0,
                    bucket.bucket.body_indices,
                )?;
                Ok((node_idx, subtree))
            })
            .collect()
    });

    let mut subtrees = subtrees?;
    subtrees.sort_by_key(|(node_idx, _)| *node_idx);
    for (node_idx, subtree) in subtrees {
        let offset = if subtree.nodes.len() > 1 {
            tree.nodes.len() as i32 - 1
        } else {
            0
        };
        let mut root_node = subtree.nodes[0];
        if offset != 0 {
            for child in &mut root_node.children {
                if *child >= 0 {
                    *child += offset;
                }
            }
        }
        tree.nodes[node_idx] = root_node;
        let extra_nodes = subtree.nodes.len().saturating_sub(1);
        if !tree.can_grow(extra_nodes) {
            return Err(format!(
                "tree node capacity exceeded while merging parallel subtree (used={}, additional={}, capacity={})",
                tree.nodes.len(),
                extra_nodes,
                tree.capacity()
            ));
        }
        tree.nodes
            .extend(subtree.nodes.into_iter().skip(1).map(|mut node| {
                for child in &mut node.children {
                    if *child >= 0 {
                        *child += offset;
                    }
                }
                node
            }));
    }

    Ok(())
}

#[inline]
fn build_tree_prefix(
    tree: &mut QuadTree,
    particles: &ParticleSoa,
    node_idx: usize,
    body_indices: &mut [usize],
    scratch: &mut [usize],
    depth_remaining: usize,
    buckets: &mut Vec<BuildBucket>,
) -> Result<(), String> {
    if body_indices.is_empty() {
        return Ok(());
    }

    if depth_remaining == 0 {
        buckets.push(BuildBucket {
            node_idx,
            body_indices: body_indices.to_vec(),
        });
        return Ok(());
    }

    if body_indices.len() == 1 {
        let body_idx = body_indices[0];
        let node = &mut tree.nodes[node_idx];
        node.mass = particles.m[body_idx];
        node.com_x = particles.x[body_idx];
        node.com_y = particles.y[body_idx];
        node.body_idx = body_idx as i32;
        return Ok(());
    }

    split_leaf(tree, node_idx)?;
    tree.nodes[node_idx].body_idx = -1;
    let node = tree.nodes[node_idx];
    let mut child_counts = [0usize; 4];
    for &body_idx in body_indices.iter() {
        let root = &mut tree.nodes[node_idx];
        let old_mass = root.mass;
        let mass = particles.m[body_idx];
        let new_mass = old_mass + mass;
        if old_mass == 0.0 {
            root.com_x = particles.x[body_idx];
            root.com_y = particles.y[body_idx];
        } else {
            let inv_new = 1.0 / new_mass;
            root.com_x = (root.com_x * old_mass + particles.x[body_idx] * mass) * inv_new;
            root.com_y = (root.com_y * old_mass + particles.y[body_idx] * mass) * inv_new;
        }
        root.mass = new_mass;

        let child = choose_child(
            node.x_min,
            node.x_max,
            node.y_min,
            node.y_max,
            particles.x[body_idx],
            particles.y[body_idx],
        );
        child_counts[child] += 1;
    }

    let starts = [
        0,
        child_counts[0],
        child_counts[0] + child_counts[1],
        child_counts[0] + child_counts[1] + child_counts[2],
    ];
    let mut write_offsets = starts;
    for &body_idx in body_indices.iter() {
        let child = choose_child(
            node.x_min,
            node.x_max,
            node.y_min,
            node.y_max,
            particles.x[body_idx],
            particles.y[body_idx],
        );
        let slot = write_offsets[child];
        scratch[slot] = body_idx;
        write_offsets[child] += 1;
    }
    body_indices.copy_from_slice(scratch);

    for child in 0..4 {
        let start = starts[child];
        let end = start + child_counts[child];
        if start == end {
            continue;
        }
        build_tree_prefix(
            tree,
            particles,
            node.children[child] as usize,
            &mut body_indices[start..end],
            &mut scratch[start..end],
            depth_remaining - 1,
            buckets,
        )?;
    }

    Ok(())
}

#[inline]
fn fill_subtree_from_ordered_bodies(
    tree: &mut QuadTree,
    particles: &ParticleSoa,
    node_idx: usize,
    mut body_indices: Vec<usize>,
) -> Result<(), String> {
    let mut scratch = vec![0usize; body_indices.len()];
    fill_subtree_from_ordered_body_slice(tree, particles, node_idx, &mut body_indices, &mut scratch)
}

#[inline]
fn fill_subtree_from_ordered_body_slice(
    tree: &mut QuadTree,
    particles: &ParticleSoa,
    node_idx: usize,
    body_indices: &mut [usize],
    scratch: &mut [usize],
) -> Result<(), String> {
    if body_indices.is_empty() {
        return Ok(());
    }

    if body_indices.len() < 16 {
        for &body_idx in body_indices.iter() {
            insert_into_node(tree, particles, node_idx, body_idx)?;
        }
        return Ok(());
    }

    split_leaf(tree, node_idx)?;
    tree.nodes[node_idx].body_idx = -1;
    let node = tree.nodes[node_idx];
    let mut child_counts = [0usize; 4];
    for &body_idx in body_indices.iter() {
        let root = &mut tree.nodes[node_idx];
        let old_mass = root.mass;
        let mass = particles.m[body_idx];
        let new_mass = old_mass + mass;
        if old_mass == 0.0 {
            root.com_x = particles.x[body_idx];
            root.com_y = particles.y[body_idx];
        } else {
            let inv_new = 1.0 / new_mass;
            root.com_x = (root.com_x * old_mass + particles.x[body_idx] * mass) * inv_new;
            root.com_y = (root.com_y * old_mass + particles.y[body_idx] * mass) * inv_new;
        }
        root.mass = new_mass;

        let child = choose_child(
            node.x_min,
            node.x_max,
            node.y_min,
            node.y_max,
            particles.x[body_idx],
            particles.y[body_idx],
        );
        child_counts[child] += 1;
    }

    let starts = [
        0,
        child_counts[0],
        child_counts[0] + child_counts[1],
        child_counts[0] + child_counts[1] + child_counts[2],
    ];
    let mut write_offsets = starts;
    for &body_idx in body_indices.iter() {
        let child = choose_child(
            node.x_min,
            node.x_max,
            node.y_min,
            node.y_max,
            particles.x[body_idx],
            particles.y[body_idx],
        );
        let slot = write_offsets[child];
        scratch[slot] = body_idx;
        write_offsets[child] += 1;
    }
    body_indices.copy_from_slice(scratch);

    for child in 0..4 {
        let start = starts[child];
        let end = start + child_counts[child];
        if start == end {
            continue;
        }
        fill_subtree_from_ordered_body_slice(
            tree,
            particles,
            node.children[child] as usize,
            &mut body_indices[start..end],
            &mut scratch[start..end],
        )?;
    }

    Ok(())
}

fn insert_into_node(
    tree: &mut QuadTree,
    particles: &ParticleSoa,
    node_idx: usize,
    body_idx: usize,
) -> Result<(), String> {
    let x = particles.x[body_idx];
    let y = particles.y[body_idx];
    let mass = particles.m[body_idx];

    {
        let node = &mut tree.nodes[node_idx];
        let old_mass = node.mass;
        let new_mass = old_mass + mass;

        if old_mass == 0.0 {
            node.com_x = x;
            node.com_y = y;
        } else {
            let inv_new = 1.0 / new_mass;
            node.com_x = (node.com_x * old_mass + x * mass) * inv_new;
            node.com_y = (node.com_y * old_mass + y * mass) * inv_new;
        }
        node.mass = new_mass;

        if node.body_idx == -1 && node.is_leaf() {
            node.body_idx = body_idx as i32;
            return Ok(());
        }
    }

    let leaf = tree.nodes[node_idx].is_leaf();
    if leaf {
        let existing_body = tree.nodes[node_idx].body_idx;
        if existing_body < 0 {
            return Err("invalid tree state: leaf without body".to_string());
        }

        let existing_body = existing_body as usize;
        split_leaf(tree, node_idx)?;

        let child_indices = tree.nodes[node_idx].children;

        tree.nodes[node_idx].body_idx = -1;

        insert_into_node(
            tree,
            particles,
            child_indices[choose_child_checked(
                tree.nodes[node_idx].x_min,
                tree.nodes[node_idx].x_max,
                tree.nodes[node_idx].y_min,
                tree.nodes[node_idx].y_max,
                particles.x[existing_body],
                particles.y[existing_body],
            )?] as usize,
            existing_body,
        )?;
        insert_into_node(
            tree,
            particles,
            child_indices[choose_child_checked(
                tree.nodes[node_idx].x_min,
                tree.nodes[node_idx].x_max,
                tree.nodes[node_idx].y_min,
                tree.nodes[node_idx].y_max,
                x,
                y,
            )?] as usize,
            body_idx,
        )?;

        return Ok(());
    }

    let child = choose_child_checked(
        tree.nodes[node_idx].x_min,
        tree.nodes[node_idx].x_max,
        tree.nodes[node_idx].y_min,
        tree.nodes[node_idx].y_max,
        x,
        y,
    )?;
    let child_idx = tree.nodes[node_idx].children[child];
    if child_idx < 0 {
        return Err("child missing during insertion".to_string());
    }

    insert_into_node(tree, particles, child_idx as usize, body_idx)
}

#[inline]
fn split_leaf(tree: &mut QuadTree, node_idx: usize) -> Result<(), String> {
    if !tree.nodes[node_idx].is_leaf() {
        return Ok(());
    }

    if !tree.can_grow(4) {
        return Err(format!(
            "tree node capacity exceeded while splitting node {} (used={}, capacity={})",
            node_idx,
            tree.nodes.len(),
            tree.capacity()
        ));
    }

    let (x_min, x_max, y_min, y_max) = {
        let node = &tree.nodes[node_idx];
        (node.x_min, node.x_max, node.y_min, node.y_max)
    };

    let x_mid = 0.5 * (x_min + x_max);
    let y_mid = 0.5 * (y_min + y_max);

    let nw = Node::with_bounds(x_min, x_mid, y_mid, y_max);
    let ne = Node::with_bounds(x_mid, x_max, y_mid, y_max);
    let sw = Node::with_bounds(x_min, x_mid, y_min, y_mid);
    let se = Node::with_bounds(x_mid, x_max, y_min, y_mid);

    let base = tree.nodes.len() as i32;
    tree.nodes.push(nw);
    tree.nodes.push(ne);
    tree.nodes.push(sw);
    tree.nodes.push(se);

    tree.nodes[node_idx].children = [base, base + 1, base + 2, base + 3];

    Ok(())
}

#[inline(always)]
fn choose_child_checked(
    x_min: f64,
    x_max: f64,
    y_min: f64,
    y_max: f64,
    x: f64,
    y: f64,
) -> Result<usize, String> {
    Ok(choose_child(x_min, x_max, y_min, y_max, x, y))
}

#[inline(always)]
fn choose_child(x_min: f64, x_max: f64, y_min: f64, y_max: f64, x: f64, y: f64) -> usize {
    let x_mid = 0.5 * (x_min + x_max);
    let y_mid = 0.5 * (y_min + y_max);

    let east = x >= x_mid;
    let north = y >= y_mid;

    match (east, north) {
        (false, true) => 0,
        (true, true) => 1,
        (false, false) => 2,
        (true, false) => 3,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn compute_accel_barnes_hut(
    particles: &ParticleSoa,
    tree: &QuadTree,
    theta: f64,
    epsilon: f64,
    g: f64,
    thread_count: usize,
    stack: &mut Vec<usize>,
    ax: &mut [f64],
    ay: &mut [f64],
) -> Result<(), String> {
    compute_accel_barnes_hut_with_threshold_impl(
        particles,
        tree,
        theta,
        epsilon,
        g,
        thread_count,
        MIN_PARALLEL_BH_PARTICLES,
        stack,
        ax,
        ay,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn compute_accel_barnes_hut_with_threshold(
    particles: &ParticleSoa,
    tree: &QuadTree,
    theta: f64,
    epsilon: f64,
    g: f64,
    thread_count: usize,
    min_parallel_particles: usize,
    stack: &mut Vec<usize>,
    ax: &mut [f64],
    ay: &mut [f64],
) -> Result<(), String> {
    compute_accel_barnes_hut_with_threshold_impl(
        particles,
        tree,
        theta,
        epsilon,
        g,
        thread_count,
        min_parallel_particles,
        stack,
        ax,
        ay,
    )
}

#[allow(clippy::too_many_arguments)]
fn compute_accel_barnes_hut_with_threshold_impl(
    particles: &ParticleSoa,
    tree: &QuadTree,
    theta: f64,
    epsilon: f64,
    g: f64,
    thread_count: usize,
    min_parallel_particles: usize,
    stack: &mut Vec<usize>,
    ax: &mut [f64],
    ay: &mut [f64],
) -> Result<(), String> {
    let n = particles.len();
    if n == 0 {
        return Ok(());
    }
    if ax.len() < n || ay.len() < n {
        return Err(format!(
            "acceleration buffer too short: n={n}, ax_len={}, ay_len={}",
            ax.len(),
            ay.len()
        ));
    }
    let thread_count = effective_bh_force_threads(thread_count, n, min_parallel_particles);

    let theta2 = theta * theta;
    let eps2 = epsilon * epsilon;

    if thread_count <= 1 {
        for i in 0..n {
            let (force_x, force_y) =
                compute_particle_force(i, particles, &tree.nodes, theta2, eps2, g, stack);
            ax[i] = force_x;
            ay[i] = force_y;
        }
        return Ok(());
    }

    compute_accel_barnes_hut_parallel(
        particles,
        &tree.nodes,
        theta2,
        eps2,
        g,
        thread_count,
        stack,
        ax,
        ay,
    )
}

#[inline]
fn compute_particle_force(
    i: usize,
    particles: &ParticleSoa,
    nodes: &[Node],
    theta2: f64,
    eps2: f64,
    g: f64,
    stack: &mut Vec<usize>,
) -> (f64, f64) {
    let mut force_x = 0.0;
    let mut force_y = 0.0;
    let xi = particles.x[i];
    let yi = particles.y[i];

    stack.clear();
    stack.push(0);

    while let Some(node_idx) = stack.pop() {
        let node = &nodes[node_idx];
        if node.mass <= 0.0 {
            continue;
        }

        let dx = node.com_x - xi;
        let dy = node.com_y - yi;
        let dist2 = dx * dx + dy * dy;
        let dist2_soft = dist2 + eps2;
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

#[allow(clippy::too_many_arguments)]
fn compute_accel_barnes_hut_parallel(
    particles: &ParticleSoa,
    nodes: &[Node],
    theta2: f64,
    eps2: f64,
    g: f64,
    thread_count: usize,
    stack: &mut Vec<usize>,
    ax: &mut [f64],
    ay: &mut [f64],
) -> Result<(), String> {
    if thread_count <= 1 {
        return Ok(());
    }

    let n = particles.len();
    if ax.len() < n || ay.len() < n {
        return Err(format!(
            "acceleration buffer too short: n={n}, ax_len={}, ay_len={}",
            ax.len(),
            ay.len()
        ));
    }

    let active_threads = thread_count.min(n);
    if active_threads <= 1 {
        for i in 0..n {
            let (force_x, force_y) =
                compute_particle_force(i, particles, nodes, theta2, eps2, g, stack);
            ax[i] = force_x;
            ay[i] = force_y;
        }
        return Ok(());
    }

    let _ = stack;

    let ax = &mut ax[..n];
    let ay = &mut ay[..n];
    let pool = rayon_pool(active_threads)?;
    pool.install(|| {
        ax.par_chunks_mut(PARTICLE_CHUNK_LEN)
            .zip(ay.par_chunks_mut(PARTICLE_CHUNK_LEN))
            .enumerate()
            .for_each_init(
                || Vec::with_capacity(TRAVERSAL_STACK_CAPACITY),
                |local_stack, (chunk_idx, (chunk_ax, chunk_ay))| {
                    let start = chunk_idx * PARTICLE_CHUNK_LEN;
                    for (offset, (ax_slot, ay_slot)) in
                        chunk_ax.iter_mut().zip(chunk_ay.iter_mut()).enumerate()
                    {
                        let particle_idx = start + offset;
                        let (force_x, force_y) = compute_particle_force(
                            particle_idx,
                            particles,
                            nodes,
                            theta2,
                            eps2,
                            g,
                            local_stack,
                        );
                        *ax_slot = force_x;
                        *ay_slot = force_y;
                    }
                },
            );
    });

    Ok(())
}

fn rayon_pool(thread_count: usize) -> Result<&'static rayon::ThreadPool, String> {
    static POOLS: OnceLock<Mutex<HashMap<usize, &'static rayon::ThreadPool>>> = OnceLock::new();
    let pools = POOLS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut pools = pools
        .lock()
        .map_err(|_| "rayon thread-pool cache lock poisoned".to_string())?;
    if let Some(pool) = pools.get(&thread_count) {
        return Ok(*pool);
    }

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(thread_count)
        .thread_name(move |idx| format!("nq-bh-{thread_count}-{idx}"))
        .build()
        .map_err(|err| format!("failed to build rayon thread pool: {err}"))?;
    let pool = Box::leak(Box::new(pool));
    pools.insert(thread_count, pool);
    Ok(pool)
}

const F64_BYTES: usize = size_of::<f64>();
const NODE_BYTES: usize = size_of::<Node>();
const USIZE_BYTES: usize = size_of::<usize>();

pub fn preflight_node_capacity(n: usize) -> Result<usize, String> {
    n.checked_mul(4)
        .and_then(|v| v.checked_add(1))
        .ok_or_else(|| "node capacity overflow for requested particle count".to_string())
}

fn check_memory_budget(args: &Args, workspace_bytes: usize) -> Result<(), String> {
    let Some(max_memory_mib) = args.max_memory_mib else {
        return Ok(());
    };

    let max_memory_bytes = max_memory_mib.checked_mul(1024 * 1024).ok_or_else(|| {
        format!(
            "invalid --max-memory-mib value (overflow while converting to bytes): {max_memory_mib}"
        )
    })?;
    if workspace_bytes > max_memory_bytes {
        return Err(format!(
            "memory budget exceeded: workspace estimate {workspace_bytes} bytes > limit {max_memory_bytes} bytes"
        ));
    }

    Ok(())
}

fn particle_state_bytes(n: usize) -> usize {
    n.saturating_mul(7).saturating_mul(F64_BYTES)
}

fn node_pool_bytes(nodes: usize) -> usize {
    nodes.saturating_mul(NODE_BYTES)
}

fn traversal_stack_bytes(node_capacity: usize, thread_count: usize) -> usize {
    // The base serial stack is allocated unconditionally before the force
    // call; threaded runs add the rayon workers' fixed-capacity stacks.
    let stack_slots = if thread_count <= 1 {
        node_capacity
    } else {
        node_capacity.saturating_add(thread_count.saturating_mul(TRAVERSAL_STACK_CAPACITY))
    };
    stack_slots.saturating_mul(USIZE_BYTES)
}

fn parallel_tree_build_transient_bytes(n: usize, thread_count: usize) -> Result<usize, String> {
    if thread_count <= 1 || n < 2 || n < MIN_PARALLEL_TREE_BUILD_PARTICLES {
        return Ok(0);
    }

    let prefix_depth = parallel_tree_prefix_depth(thread_count);
    let bucket_count = 4usize.checked_pow(prefix_depth as u32).ok_or_else(|| {
        "parallel tree prefix bucket count overflow for memory estimate".to_string()
    })?;
    let node_capacity = preflight_node_capacity(n)?;
    let per_bucket_slack = bucket_count.saturating_mul(1025);
    // Threaded tree-build transients are intentionally charged separately from
    // the persistent output tree: the prefix owns/remaps index vectors, each
    // worker allocates one scratch index buffer, and completed per-bucket
    // subtrees remain live until deterministic merge.  The Vec growth factor is
    // bounded conservatively by charging one extra full index layer per prefix
    // level in addition to the final bucket indices and subtree scratch.
    let index_layers = prefix_depth.saturating_add(2);
    let index_bytes = n.saturating_mul(index_layers).saturating_mul(USIZE_BYTES);
    let subtree_nodes = node_capacity.saturating_add(per_bucket_slack);
    Ok(index_bytes.saturating_add(node_pool_bytes(subtree_nodes)))
}

#[cfg(test)]
mod tests {
    use super::{
        MIN_PARALLEL_TREE_BUILD_PARTICLES, build_tree, build_tree_with_threads,
        build_tree_with_threads_with_threshold, compute_accel_barnes_hut,
        compute_barnes_hut_accel_snapshot, compute_barnes_hut_accel_snapshot_with_threshold,
        node_pool_bytes, parallel_tree_build_transient_bytes, parallel_tree_prefix_depth,
        preflight_node_capacity, run_barnes_hut, run_barnes_hut_with_threshold,
    };
    use clap::Parser;
    use std::f64::consts::PI;

    use crate::{
        config::{Args, InitProfile, MassProfile},
        direct::{compute_direct_accel_with_g, run_direct},
        particle::{
            ParticleSoa, compute_energy_snapshot, compute_exact_energy_snapshot,
            compute_sampled_energy_snapshot,
        },
        tree::QuadTree,
    };

    const KEPLER_G: f64 = 1.0;
    const KEPLER_R: f64 = 1.0;
    const KEPLER_EPSILON: f64 = 0.0;
    const POSITION_TOLERANCE_FACTOR: f64 = 1.0e-3;
    const ENERGY_DRIFT_TOLERANCE: f64 = 1.0e-6;

    const ENERGY_REGRESSION_N: usize = 512;
    const ENERGY_REGRESSION_STEPS: usize = 500;
    const ENERGY_REGRESSION_DT: f64 = 0.001;
    const ENERGY_REGRESSION_THETA: f64 = 0.5;
    const ENERGY_REGRESSION_EPSILON: f64 = 0.01;
    const THETA_ACCURACY_N: usize = 1024;
    const THETA_ACCURACY_EPSILON: f64 = 0.01;
    const THETA_ACCURACY_VALUES: [f64; 4] = [0.3, 0.5, 0.7, 1.0];

    #[derive(Clone, Copy)]
    struct KeplerCase {
        name: &'static str,
        m1: f64,
        m2: f64,
    }

    fn period(separation: f64, m1: f64, m2: f64, g: f64) -> f64 {
        2.0 * PI * (separation.powi(3) / (g * (m1 + m2))).sqrt()
    }

    fn angular_velocity(separation: f64, m1: f64, m2: f64, g: f64) -> f64 {
        (g * (m1 + m2) / separation.powi(3)).sqrt()
    }

    fn barycentric_radius(separation: f64, body_mass: f64, other_mass: f64) -> f64 {
        separation * other_mass / (body_mass + other_mass)
    }

    fn analytic_positions(case: KeplerCase, separation: f64, g: f64, t: f64) -> [(f64, f64); 2] {
        let r1 = barycentric_radius(separation, case.m1, case.m2);
        let r2 = barycentric_radius(separation, case.m2, case.m1);
        let angle = angular_velocity(separation, case.m1, case.m2, g) * t;
        let (sin_a, cos_a) = angle.sin_cos();

        [(-r1 * cos_a, -r1 * sin_a), (r2 * cos_a, r2 * sin_a)]
    }

    fn circular_two_body(case: KeplerCase, separation: f64, g: f64) -> ParticleSoa {
        let mut particles = ParticleSoa::with_len(2);
        let positions = analytic_positions(case, separation, g, 0.0);
        let r1 = barycentric_radius(separation, case.m1, case.m2);
        let r2 = barycentric_radius(separation, case.m2, case.m1);

        // Derivation for the circular two-body oracle:
        // In barycentric coordinates, body i orbits at radius
        // r_i = r*m_other/(m1+m2) while the inter-body separation is r.
        // Gravity gives body i acceleration a_i = G*m_other/r^2.  Uniform
        // circular motion requires centripetal acceleration a_i = v_i^2/r_i.
        // Equating them gives v_i^2/r_i = G*m_other/r^2, hence
        // v_i = sqrt(G*m_other*r_i/r^2)
        //     = sqrt(G*m_other^2/((m1+m2)*r)).  With angular velocity
        // omega = v_i/r_i = sqrt(G*(m1+m2)/r^3), the period is
        // T = 2*pi/omega = 2*pi*sqrt(r^3/(G*(m1+m2))).
        particles.x[0] = positions[0].0;
        particles.y[0] = positions[0].1;
        particles.vx[0] = 0.0;
        particles.vy[0] = -((g * case.m2 * case.m2) / ((case.m1 + case.m2) * separation)).sqrt();
        particles.m[0] = case.m1;

        particles.x[1] = positions[1].0;
        particles.y[1] = positions[1].1;
        particles.vx[1] = 0.0;
        particles.vy[1] = ((g * case.m1 * case.m1) / ((case.m1 + case.m2) * separation)).sqrt();
        particles.m[1] = case.m2;

        debug_assert!(
            (particles.vy[0].abs() - r1 * angular_velocity(separation, case.m1, case.m2, g)).abs()
                < 1e-12
        );
        debug_assert!(
            (particles.vy[1].abs() - r2 * angular_velocity(separation, case.m1, case.m2, g)).abs()
                < 1e-12
        );

        particles
    }

    fn kepler_args(mode: &str, theta: f64, dt: f64, steps: usize) -> Args {
        kepler_args_with_integrator(mode, theta, dt, steps, "leapfrog")
    }

    fn kepler_args_with_integrator(
        mode: &str,
        theta: f64,
        dt: f64,
        steps: usize,
        integrator: &str,
    ) -> Args {
        let theta_s = theta.to_string();
        let dt_s = dt.to_string();
        let steps_s = steps.to_string();
        let g_s = KEPLER_G.to_string();
        let epsilon_s = KEPLER_EPSILON.to_string();
        Args::parse_from([
            "nq",
            "--n",
            "2",
            "--steps",
            steps_s.as_str(),
            "--dt",
            dt_s.as_str(),
            "--theta",
            theta_s.as_str(),
            "--epsilon",
            epsilon_s.as_str(),
            "--g",
            g_s.as_str(),
            "--integrator",
            integrator,
            "--mode",
            mode,
            "--threads",
            "1",
            "--energy-drift",
            "on",
        ])
    }

    fn total_energy(particles: &ParticleSoa) -> f64 {
        compute_energy_snapshot(particles, KEPLER_EPSILON, KEPLER_G, 0.0, true, 0)
            .expect("two-body energy snapshot should be available")
            .total
    }

    fn regression_energy(particles: &ParticleSoa, args: &Args) -> f64 {
        compute_exact_energy_snapshot(particles, args.epsilon, args.g).total
    }

    fn energy_regression_args(init: &str, seed: u64) -> Args {
        energy_regression_args_with_dt_steps(
            init,
            seed,
            ENERGY_REGRESSION_DT,
            ENERGY_REGRESSION_STEPS,
        )
    }

    fn energy_regression_args_with_dt_steps(init: &str, seed: u64, dt: f64, steps: usize) -> Args {
        let n_s = ENERGY_REGRESSION_N.to_string();
        let steps_s = steps.to_string();
        let dt_s = dt.to_string();
        let theta_s = ENERGY_REGRESSION_THETA.to_string();
        let epsilon_s = ENERGY_REGRESSION_EPSILON.to_string();
        let seed_s = seed.to_string();

        Args::parse_from([
            "nq",
            "--n",
            n_s.as_str(),
            "--init",
            init,
            "--steps",
            steps_s.as_str(),
            "--dt",
            dt_s.as_str(),
            "--theta",
            theta_s.as_str(),
            "--epsilon",
            epsilon_s.as_str(),
            "--integrator",
            "leapfrog",
            "--seed",
            seed_s.as_str(),
            "--threads",
            "1",
            "--energy-drift",
            "on",
        ])
    }

    fn thread_parity_args(threads: usize) -> Args {
        let threads_s = threads.to_string();
        Args::parse_from([
            "nq",
            "--n",
            "4097",
            "--init",
            "plummer",
            "--steps",
            "20",
            "--dt",
            "0.001",
            "--theta",
            "0.5",
            "--epsilon",
            "0.01",
            "--g",
            "0.9",
            "--integrator",
            "leapfrog",
            "--seed",
            "42",
            "--threads",
            threads_s.as_str(),
        ])
    }

    fn assert_bitwise_eq(label: &str, threads: usize, baseline: &[f64], candidate: &[f64]) {
        assert_eq!(
            baseline.len(),
            candidate.len(),
            "thread parity {label} length mismatch for threads={threads}: threads=1 len={}, threads={threads} len={}",
            baseline.len(),
            candidate.len()
        );

        for (i, (&expected, &actual)) in baseline.iter().zip(candidate).enumerate() {
            assert_eq!(
                expected.to_bits(),
                actual.to_bits(),
                "thread parity {label} mismatch for threads={threads} at particle {i}: threads=1 value={expected:.17e} bits=0x{:016x}, threads={threads} value={actual:.17e} bits=0x{:016x}",
                expected.to_bits(),
                actual.to_bits()
            );
        }
    }

    #[test]
    fn parallel_prefix_depth_scales_past_root_quadrants() {
        assert_eq!(parallel_tree_prefix_depth(1), 1);
        assert_eq!(parallel_tree_prefix_depth(12), 3);
    }

    #[test]
    fn parallel_tree_build_transients_are_charged_for_threaded_runs() -> Result<(), String> {
        let n = 100_000usize;
        assert_eq!(parallel_tree_build_transient_bytes(n, 1)?, 0);
        assert!(
            parallel_tree_build_transient_bytes(n, 12)?
                > node_pool_bytes(preflight_node_capacity(n)?)
        );
        Ok(())
    }

    type NodeFingerprint = (u64, u64, u64, u64, u64, u64, u64, i32, [i32; 4]);

    fn tree_fingerprint(tree: &QuadTree) -> Vec<NodeFingerprint> {
        tree.nodes
            .iter()
            .map(|node| {
                (
                    node.x_min.to_bits(),
                    node.x_max.to_bits(),
                    node.y_min.to_bits(),
                    node.y_max.to_bits(),
                    node.mass.to_bits(),
                    node.com_x.to_bits(),
                    node.com_y.to_bits(),
                    node.body_idx,
                    node.children,
                )
            })
            .collect()
    }

    #[test]
    fn tree_build_threshold_routes_low_n_serial_but_injected_zero_exercises_parallel_path()
    -> Result<(), String> {
        let n = 4_097usize;
        assert!(n < MIN_PARALLEL_TREE_BUILD_PARTICLES);
        let particles = ParticleSoa::random_with_profiles(
            n,
            42,
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
        );
        let capacity = preflight_node_capacity(n)?;
        let mut serial_tree = QuadTree::with_capacity(capacity);
        let mut default_threaded_tree = QuadTree::with_capacity(capacity);
        let mut forced_threaded_tree = QuadTree::with_capacity(capacity);

        build_tree(&mut serial_tree, &particles)?;
        build_tree_with_threads(&mut default_threaded_tree, &particles, 12)?;
        build_tree_with_threads_with_threshold(&mut forced_threaded_tree, &particles, 12, 0)?;

        assert_eq!(
            tree_fingerprint(&serial_tree),
            tree_fingerprint(&default_threaded_tree)
        );
        assert_ne!(
            tree_fingerprint(&serial_tree),
            tree_fingerprint(&forced_threaded_tree)
        );

        let mut serial_ax = vec![0.0; n];
        let mut serial_ay = vec![0.0; n];
        let mut forced_ax = vec![0.0; n];
        let mut forced_ay = vec![0.0; n];
        let mut stack = Vec::with_capacity(capacity);
        compute_accel_barnes_hut(
            &particles,
            &serial_tree,
            0.7,
            0.01,
            1.0,
            1,
            &mut stack,
            &mut serial_ax,
            &mut serial_ay,
        )?;
        stack.clear();
        compute_accel_barnes_hut(
            &particles,
            &forced_threaded_tree,
            0.7,
            0.01,
            1.0,
            1,
            &mut stack,
            &mut forced_ax,
            &mut forced_ay,
        )?;
        assert_bitwise_eq("forced parallel tree ax", 12, &serial_ax, &forced_ax);
        assert_bitwise_eq("forced parallel tree ay", 12, &serial_ay, &forced_ay);
        Ok(())
    }

    #[test]
    fn pathological_identical_cluster_uses_serial_backstop_when_parallel_bucket_overflows()
    -> Result<(), String> {
        let n = 10_000usize;
        let grid = 9_900usize;
        let mut particles = ParticleSoa::with_len(n);
        // 9,900 grid-spread particles keep the TOTAL node count well under
        // the main pool (4n+1), so the serial build always succeeds.
        for i in 0..grid {
            particles.x[i] = -1.0 + (i % 100) as f64 * 0.019;
            particles.y[i] = -1.0 + (i / 100) as f64 * 0.019;
        }
        // 50 near-degenerate pairs in one corner: separating a pair whose
        // members sit 1e-12 apart costs ~4 nodes per subdivision level, so
        // this single prefix bucket needs ~4k nodes against its local
        // budget of 4*100+1+1024 — the per-bucket overflow the serial
        // backstop exists for.
        for pair in 0..50 {
            let base = 0.95 + pair as f64 * 1.0e-6;
            let a = grid + 2 * pair;
            particles.x[a] = base;
            particles.y[a] = 0.95;
            particles.x[a + 1] = base + 1.0e-12;
            particles.y[a + 1] = 0.95;
        }
        let capacity = preflight_node_capacity(n)?;
        let mut fallback_tree = QuadTree::with_capacity(capacity);
        build_tree_with_threads_with_threshold(&mut fallback_tree, &particles, 12, 0)?;
        assert!(!fallback_tree.nodes.is_empty());
        // The backstop is a serial rebuild, so node numbering must match a
        // serially built tree exactly; the parallel layout would differ.
        let mut serial_tree = QuadTree::with_capacity(capacity);
        build_tree(&mut serial_tree, &particles)?;
        assert_eq!(
            tree_fingerprint(&serial_tree),
            tree_fingerprint(&fallback_tree),
            "pathological cluster did not trigger the serial backstop"
        );
        Ok(())
    }

    #[test]
    fn parallel_built_tree_forces_match_serial_tree_bitwise() -> Result<(), String> {
        fn assert_parallel_tree_force_parity(particles: &ParticleSoa) -> Result<(), String> {
            let n = particles.len();
            let capacity = preflight_node_capacity(n)?;
            let mut serial_tree = QuadTree::with_capacity(capacity);
            let mut parallel_tree = QuadTree::with_capacity(capacity);
            build_tree(&mut serial_tree, particles)?;
            build_tree_with_threads_with_threshold(&mut parallel_tree, particles, 12, 0)?;

            let mut serial_ax = vec![0.0; n];
            let mut serial_ay = vec![0.0; n];
            let mut parallel_ax = vec![0.0; n];
            let mut parallel_ay = vec![0.0; n];
            let mut stack = Vec::with_capacity(capacity);
            compute_accel_barnes_hut(
                particles,
                &serial_tree,
                0.7,
                0.01,
                1.0,
                1,
                &mut stack,
                &mut serial_ax,
                &mut serial_ay,
            )?;
            stack.clear();
            compute_accel_barnes_hut(
                particles,
                &parallel_tree,
                0.7,
                0.01,
                1.0,
                1,
                &mut stack,
                &mut parallel_ax,
                &mut parallel_ay,
            )?;

            assert_bitwise_eq("parallel tree ax", 12, &serial_ax, &parallel_ax);
            assert_bitwise_eq("parallel tree ay", 12, &serial_ay, &parallel_ay);
            Ok(())
        }

        let n = 10_000usize;
        for seed in [42_u64, 1902] {
            for init in [InitProfile::Plummer, InitProfile::Gaussian] {
                let particles = ParticleSoa::random_with_profiles(
                    n,
                    seed,
                    init,
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
                );
                assert_parallel_tree_force_parity(&particles)?;
            }
        }

        let mut skewed = ParticleSoa::with_len(n);
        skewed.x[0] = -1.0;
        skewed.y[0] = -1.0;
        skewed.x[1] = -1.0;
        skewed.y[1] = 1.1;
        skewed.x[2] = 1.1;
        skewed.y[2] = -1.0;
        for i in 3..n {
            let offset = (i - 3) as f64 * 1.0e-9;
            skewed.x[i] = 0.95 + offset;
            skewed.y[i] = 0.95 + offset * 0.5;
        }
        assert_parallel_tree_force_parity(&skewed)?;
        Ok(())
    }

    fn regression_particles(args: &Args) -> ParticleSoa {
        ParticleSoa::random_with_profiles(
            args.n,
            args.seed,
            args.init,
            args.init_radius,
            args.init_spread,
            args.init_v_amp,
            args.init_lambda,
            args.init_center_x,
            args.init_center_y,
            args.mass_profile,
            args.mass_mean,
            args.mass_stddev,
            args.mass_min,
            args.mass_max,
            args.mass_alpha,
        )
    }

    fn relative_energy_drift_for(args: &Args) -> Result<f64, String> {
        let mut particles = regression_particles(args);
        let initial_energy = regression_energy(&particles, args);

        run_barnes_hut(&mut particles, args, None)?;

        let final_energy = regression_energy(&particles, args);
        Ok((final_energy - initial_energy).abs() / initial_energy.abs())
    }

    fn assert_energy_drift_regression(
        case_name: &str,
        init: &str,
        seed: u64,
        threshold: f64,
    ) -> Result<(), String> {
        let args = energy_regression_args(init, seed);
        let relative_energy_drift = relative_energy_drift_for(&args)?;
        eprintln!(
            "energy_drift_regression case={} init={} seed={} relative_energy_drift={:.15} threshold={:.15}",
            case_name, init, seed, relative_energy_drift, threshold
        );
        assert!(
            relative_energy_drift < threshold,
            "energy regression case={} init={} seed={} relative energy drift {} exceeds {}",
            case_name,
            init,
            seed,
            relative_energy_drift,
            threshold
        );

        Ok(())
    }

    fn theta_accuracy_args(init: &str, theta: f64) -> Args {
        let n_s = THETA_ACCURACY_N.to_string();
        let theta_s = theta.to_string();
        let epsilon_s = THETA_ACCURACY_EPSILON.to_string();

        Args::parse_from([
            "nq",
            "--n",
            n_s.as_str(),
            "--init",
            init,
            "--steps",
            "0",
            "--theta",
            theta_s.as_str(),
            "--epsilon",
            epsilon_s.as_str(),
            "--seed",
            "42",
            "--threads",
            "1",
        ])
    }

    fn relative_force_rms_error(init: &str, theta: f64) -> Result<f64, String> {
        let args = theta_accuracy_args(init, theta);
        let particles = regression_particles(&args);
        let (bh_ax, bh_ay) = super::compute_barnes_hut_accel_snapshot(&particles, &args)?;
        let mut direct_ax = vec![0.0; particles.len()];
        let mut direct_ay = vec![0.0; particles.len()];
        compute_direct_accel_with_g(
            &particles,
            THETA_ACCURACY_EPSILON,
            args.g,
            &mut direct_ax,
            &mut direct_ay,
        );

        let mut error_sum = 0.0;
        let mut reference_sum = 0.0;
        for i in 0..particles.len() {
            let dx = bh_ax[i] - direct_ax[i];
            let dy = bh_ay[i] - direct_ay[i];
            error_sum += dx * dx + dy * dy;
            reference_sum += direct_ax[i] * direct_ax[i] + direct_ay[i] * direct_ay[i];
        }

        if reference_sum <= f64::EPSILON {
            return Err(format!(
                "direct-force reference norm vanished for init={init} theta={theta}; relative RMS undefined"
            ));
        }
        Ok((error_sum / reference_sum).sqrt())
    }

    fn theta_accuracy_errors(init: &str) -> Result<[f64; 4], String> {
        let mut errors = [0.0; 4];
        for (idx, theta) in THETA_ACCURACY_VALUES.iter().copied().enumerate() {
            errors[idx] = relative_force_rms_error(init, theta)?;
            eprintln!(
                "theta_accuracy init={} theta={:.1} relative_force_rms_error={:.15}",
                init, theta, errors[idx]
            );
        }
        Ok(errors)
    }

    fn assert_monotone_non_decreasing(init: &str, errors: &[f64; 4]) {
        for idx in 1..errors.len() {
            assert!(
                errors[idx] > errors[idx - 1],
                "theta accuracy init={} is not strictly increasing: theta {} error {} <= theta {} error {}",
                init,
                THETA_ACCURACY_VALUES[idx],
                errors[idx],
                THETA_ACCURACY_VALUES[idx - 1],
                errors[idx - 1]
            );
        }
    }

    fn assert_theta_ceilings(init: &str, errors: &[f64; 4], ceilings: [f64; 4]) {
        for (idx, (error, ceiling)) in errors.iter().zip(ceilings).enumerate() {
            assert!(
                *error <= ceiling,
                "theta accuracy init={} theta={} relative force RMS error {} exceeds {}",
                init,
                THETA_ACCURACY_VALUES[idx],
                error,
                ceiling
            );
        }
    }

    fn max_position_error(particles: &ParticleSoa, expected: [(f64, f64); 2]) -> f64 {
        (0..2)
            .map(|i| {
                let dx = particles.x[i] - expected[i].0;
                let dy = particles.y[i] - expected[i].1;
                (dx * dx + dy * dy).sqrt()
            })
            .fold(0.0, f64::max)
    }

    fn run_kepler_direct(
        case: KeplerCase,
        integrator: &str,
        dt: f64,
        steps: usize,
    ) -> Result<ParticleSoa, String> {
        let mut particles = circular_two_body(case, KEPLER_R, KEPLER_G);
        let args = kepler_args_with_integrator("direct", 0.0, dt, steps, integrator);
        run_direct(&mut particles, &args, None)?;
        Ok(particles)
    }

    fn position_error_after_period(
        case: KeplerCase,
        integrator: &str,
        steps: usize,
    ) -> Result<f64, String> {
        let t_period = period(KEPLER_R, case.m1, case.m2, KEPLER_G);
        let dt = t_period / steps as f64;
        let particles = run_kepler_direct(case, integrator, dt, steps)?;
        Ok(max_position_error(
            &particles,
            analytic_positions(case, KEPLER_R, KEPLER_G, t_period),
        ))
    }

    fn average_convergence_order(errors: &[f64; 4]) -> f64 {
        errors
            .windows(2)
            .map(|pair| (pair[0] / pair[1]).log2())
            .sum::<f64>()
            / 3.0
    }

    fn assert_kepler_convergence_order(
        case: KeplerCase,
        integrator: &str,
        minimum_order: f64,
    ) -> Result<f64, String> {
        let step_counts = [200usize, 400, 800, 1600];
        let errors = [
            position_error_after_period(case, integrator, step_counts[0])?,
            position_error_after_period(case, integrator, step_counts[1])?,
            position_error_after_period(case, integrator, step_counts[2])?,
            position_error_after_period(case, integrator, step_counts[3])?,
        ];
        let order = average_convergence_order(&errors);
        eprintln!(
            "kepler_convergence case={} integrator={} steps={:?} position_errors={:?} average_order={:.6}",
            case.name, integrator, step_counts, errors, order
        );
        assert!(
            order >= minimum_order,
            "kepler convergence case={} integrator={} order {} below {} (errors {:?})",
            case.name,
            integrator,
            order,
            minimum_order,
            errors
        );
        Ok(order)
    }

    fn assert_kepler_orbit(case: KeplerCase, mode: &str, theta: f64) -> Result<(), String> {
        let t_period = period(KEPLER_R, case.m1, case.m2, KEPLER_G);
        let dt = t_period / 1000.0;
        let start = circular_two_body(case, KEPLER_R, KEPLER_G);

        // epsilon=0 is supported by the direct and Barnes-Hut kernels for this
        // non-colliding setup; because no softening is applied, its effect on
        // the closed-form orbit and energy bound is exactly zero.
        let mut one_period = start.clone();
        let one_period_args = kepler_args(mode, theta, dt, 1000);
        match mode {
            "direct" => {
                run_direct(&mut one_period, &one_period_args, None)?;
            }
            "barnes_hut" => {
                run_barnes_hut(&mut one_period, &one_period_args, None)?;
            }
            _ => unreachable!("unsupported test mode"),
        }
        let position_error = max_position_error(
            &one_period,
            analytic_positions(case, KEPLER_R, KEPLER_G, t_period),
        );
        eprintln!(
            "kepler case={} mode={} one_period_position_error={} tolerance={}",
            case.name,
            mode,
            position_error,
            POSITION_TOLERANCE_FACTOR * KEPLER_R
        );
        assert!(
            position_error < POSITION_TOLERANCE_FACTOR * KEPLER_R,
            "kepler case={} mode={} position error {} exceeds {}",
            case.name,
            mode,
            position_error,
            POSITION_TOLERANCE_FACTOR * KEPLER_R
        );

        let mut ten_periods = start;
        let initial_energy = total_energy(&ten_periods);
        let ten_period_args = kepler_args(mode, theta, dt, 10_000);
        match mode {
            "direct" => {
                run_direct(&mut ten_periods, &ten_period_args, None)?;
            }
            "barnes_hut" => {
                run_barnes_hut(&mut ten_periods, &ten_period_args, None)?;
            }
            _ => unreachable!("unsupported test mode"),
        }
        let final_energy = total_energy(&ten_periods);
        let relative_energy_drift = (final_energy - initial_energy).abs() / initial_energy.abs();
        eprintln!(
            "kepler case={} mode={} relative_energy_drift={} tolerance={}",
            case.name, mode, relative_energy_drift, ENERGY_DRIFT_TOLERANCE
        );
        assert!(
            relative_energy_drift < ENERGY_DRIFT_TOLERANCE,
            "kepler case={} mode={} relative energy drift {} exceeds {}",
            case.name,
            mode,
            relative_energy_drift,
            ENERGY_DRIFT_TOLERANCE
        );

        Ok(())
    }

    #[test]
    fn kepler_circular_two_body_direct_oracle() -> Result<(), String> {
        for case in [
            KeplerCase {
                name: "equal_masses",
                m1: 1.0,
                m2: 1.0,
            },
            KeplerCase {
                name: "three_to_one_mass_ratio",
                m1: 1.0,
                m2: 3.0,
            },
        ] {
            assert_kepler_orbit(case, "direct", 0.0)?;
        }

        Ok(())
    }

    #[test]
    fn kepler_circular_two_body_barnes_hut_oracle() -> Result<(), String> {
        assert_kepler_orbit(
            KeplerCase {
                name: "equal_masses",
                m1: 1.0,
                m2: 1.0,
            },
            "barnes_hut",
            1.0e-6,
        )
    }

    #[test]
    fn convergence_order_for_rk2_and_leapfrog_kepler_cases() -> Result<(), String> {
        for case in [
            KeplerCase {
                name: "equal_masses",
                m1: 1.0,
                m2: 1.0,
            },
            KeplerCase {
                name: "three_to_one_mass_ratio",
                m1: 1.0,
                m2: 3.0,
            },
        ] {
            assert_kepler_convergence_order(case, "rk2", 1.9)?;
            assert_kepler_convergence_order(case, "leapfrog", 1.9)?;
        }

        Ok(())
    }

    #[test]
    #[ignore = "diagnostic: compares sampled and exact potential energy for bead nebula-quanta-nny"]
    fn diagnostic_sampling_vs_exact_potential_plummer_seed_42() -> Result<(), String> {
        const SAMPLE_RATIO: f64 = 0.01;
        let args = energy_regression_args("plummer", 42);
        let mut particles = regression_particles(&args);
        let sample_seed = args.seed.wrapping_add(0x9E3779B97F4A7C15);

        let initial_sampled = compute_sampled_energy_snapshot(
            &particles,
            args.epsilon,
            args.g,
            SAMPLE_RATIO,
            sample_seed,
        );
        let initial_exact = compute_exact_energy_snapshot(&particles, args.epsilon, args.g);

        run_barnes_hut(&mut particles, &args, None)?;

        let final_sampled = compute_sampled_energy_snapshot(
            &particles,
            args.epsilon,
            args.g,
            SAMPLE_RATIO,
            sample_seed,
        );
        let final_exact = compute_exact_energy_snapshot(&particles, args.epsilon, args.g);

        eprintln!(
            "diagnostic_sampling_vs_exact case=plummer_seed_42 sample_ratio={} initial_sampled_pe={:.15} initial_exact_pe={:.15} final_sampled_pe={:.15} final_exact_pe={:.15} sampled_pairs={} exact_pairs={}",
            SAMPLE_RATIO,
            initial_sampled.potential,
            initial_exact.potential,
            final_sampled.potential,
            final_exact.potential,
            initial_sampled.sampled_pairs,
            initial_exact.sampled_pairs,
        );

        Ok(())
    }

    #[test]
    #[ignore = "diagnostic: dt scaling over fixed physical time for bead nebula-quanta-nny"]
    fn diagnostic_exact_energy_dt_scaling_plummer_seed_42() -> Result<(), String> {
        let cases = [(0.001, 500usize), (0.0005, 1000usize), (0.00025, 2000usize)];
        let mut drifts = Vec::with_capacity(cases.len());
        for (dt, steps) in cases {
            let args = energy_regression_args_with_dt_steps("plummer", 42, dt, steps);
            let drift = relative_energy_drift_for(&args)?;
            eprintln!(
                "diagnostic_dt_scaling case=plummer_seed_42 dt={:.8} steps={} relative_energy_drift={:.15}",
                dt, steps, drift
            );
            drifts.push(drift);
        }
        let exponent_01 = (drifts[0] / drifts[1]).log2();
        let exponent_12 = (drifts[1] / drifts[2]).log2();
        let average_exponent = 0.5 * (exponent_01 + exponent_12);
        eprintln!(
            "diagnostic_dt_scaling case=plummer_seed_42 exponents=[{:.6}, {:.6}] average_exponent={:.6}",
            exponent_01, exponent_12, average_exponent
        );

        Ok(())
    }

    #[test]
    fn thread_parity_plummer_seed_42_matches_single_thread_bitwise() -> Result<(), String> {
        let args_single = thread_parity_args(1);
        let initial_particles = regression_particles(&args_single);
        let (single_ax, single_ay) =
            compute_barnes_hut_accel_snapshot(&initial_particles, &args_single)?;

        let mut single_final = initial_particles.clone();
        run_barnes_hut(&mut single_final, &args_single, None)?;

        // The threaded path partitions particle indices into disjoint contiguous
        // rayon chunks. Each worker calls the same per-particle traversal and
        // writes only its own ax/ay slots, so no reduction or accumulation order changes.
        for threads in [2usize, 3, 4] {
            let args_threaded = thread_parity_args(threads);
            assert_ne!(
                args_threaded.n % threads,
                0,
                "N=4097 should exercise non-even threaded work distribution for threads={threads}"
            );

            let (threaded_ax, threaded_ay) = compute_barnes_hut_accel_snapshot_with_threshold(
                &initial_particles,
                &args_threaded,
                0,
            )?;
            assert_bitwise_eq("initial ax", threads, &single_ax, &threaded_ax);
            assert_bitwise_eq("initial ay", threads, &single_ay, &threaded_ay);

            let mut threaded_final = initial_particles.clone();
            run_barnes_hut_with_threshold(&mut threaded_final, &args_threaded, None, 0)?;
            assert_bitwise_eq("final x", threads, &single_final.x, &threaded_final.x);
            assert_bitwise_eq("final y", threads, &single_final.y, &threaded_final.y);
            assert_bitwise_eq("final vx", threads, &single_final.vx, &threaded_final.vx);
            assert_bitwise_eq("final vy", threads, &single_final.vy, &threaded_final.vy);
            eprintln!("thread_parity plummer seed=42 threads={threads} equality=bitwise");
        }

        Ok(())
    }

    #[test]
    fn barnes_hut_theta_accuracy_plummer_seed_42_regression() -> Result<(), String> {
        let errors = theta_accuracy_errors("plummer")?;
        assert_monotone_non_decreasing("plummer", &errors);
        // Baselines measured 2026-06-11 for N=1024, seed=42, epsilon=0.01,
        // threads=1: theta=[0.3, 0.5, 0.7, 1.0] relative force RMS errors
        // [0.003109613705270, 0.009368051830462, 0.032234402265279,
        // 0.080799996016145]; ceilings are 3x baseline.
        assert_theta_ceilings(
            "plummer",
            &errors,
            [
                0.009328841115811,
                0.028104155491386,
                0.096703206795837,
                0.242399988048435,
            ],
        );
        Ok(())
    }

    #[test]
    fn barnes_hut_theta_accuracy_rotating_disk_seed_42_regression() -> Result<(), String> {
        let errors = theta_accuracy_errors("rotating-disk")?;
        assert_monotone_non_decreasing("rotating-disk", &errors);
        // Baselines measured 2026-06-11 for N=1024, seed=42, epsilon=0.01,
        // threads=1: theta=[0.3, 0.5, 0.7, 1.0] relative force RMS errors
        // [0.002653372755433, 0.011713759891357, 0.027360719144725,
        // 0.064170371401803]; ceilings are 3x baseline.
        assert_theta_ceilings(
            "rotating-disk",
            &errors,
            [
                0.007960118266299,
                0.035141279674071,
                0.082082157434175,
                0.192511114205409,
            ],
        );
        Ok(())
    }

    #[test]
    fn energy_drift_plummer_seed_42_regression() -> Result<(), String> {
        // Exact-PE baseline measured 2026-06-11: |energy_drift_rel| = 0.997984882458993
        // (3/3 repeated runs were bit-identical); threshold is 3x baseline.
        assert_energy_drift_regression("plummer_seed_42", "plummer", 42, 2.993954647376979)
    }

    #[test]
    fn energy_drift_plummer_seed_1337_regression() -> Result<(), String> {
        // Exact-PE baseline measured 2026-06-11: |energy_drift_rel| = 0.939209763986541
        // (3/3 repeated runs were bit-identical); threshold is 3x baseline.
        assert_energy_drift_regression("plummer_seed_1337", "plummer", 1337, 2.817629291959623)
    }

    #[test]
    fn energy_drift_rotating_disk_seed_42_regression() -> Result<(), String> {
        // Disk case uses the rotating-disk initializer. Exact-PE baseline measured
        // 2026-06-11: |energy_drift_rel| = 0.979700299946477 (3/3 repeated
        // runs were bit-identical); threshold is 3x baseline.
        assert_energy_drift_regression(
            "rotating_disk_seed_42",
            "rotating-disk",
            42,
            2.939100899839431,
        )
    }

    #[test]
    fn rk2_integration_matches_direct_when_treated_as_direct() -> Result<(), String> {
        let parse = |theta: &str| {
            Args::parse_from([
                "nq",
                "--n",
                "128",
                "--steps",
                "8",
                "--dt",
                "0.001",
                "--theta",
                theta,
                "--epsilon",
                "0.01",
                "--g",
                "0.9",
                "--integrator",
                "rk2",
                "--seed",
                "2026",
                "--mass-profile",
                "pow-law",
                "--mass-alpha",
                "2.4",
                "--mass-min",
                "0.5",
                "--mass-max",
                "1.5",
            ])
        };

        let particles = ParticleSoa::random_with_profiles(
            128,
            2026,
            parse("0").init,
            parse("0").init_radius,
            parse("0").init_spread,
            parse("0").init_v_amp,
            parse("0").init_lambda,
            parse("0").init_center_x,
            parse("0").init_center_y,
            parse("0").mass_profile,
            parse("0").mass_mean,
            parse("0").mass_stddev,
            parse("0").mass_min,
            parse("0").mass_max,
            parse("0").mass_alpha,
        );
        let mut barnes = particles.clone();
        let mut direct = particles.clone();

        let args = parse("0.0");
        let node_capacity = preflight_node_capacity(args.n)?;
        assert!(node_capacity > 0);

        run_barnes_hut(&mut barnes, &args, None)?;
        run_direct(&mut direct, &args, None)?;

        for i in 0..barnes.len() {
            assert_eq!(barnes.x[i], direct.x[i]);
            assert_eq!(barnes.y[i], direct.y[i]);
            assert_eq!(barnes.vx[i], direct.vx[i]);
            assert_eq!(barnes.vy[i], direct.vy[i]);
        }

        Ok(())
    }

    #[test]
    fn parallel_bh_force_rejects_short_buffers_and_ignores_oversized_tail() -> Result<(), String> {
        let n = 129usize;
        let args = Args::parse_from([
            "nq",
            "--n",
            "129",
            "--steps",
            "0",
            "--theta",
            "0.5",
            "--epsilon",
            "0.01",
            "--g",
            "0.9",
            "--threads",
            "2",
            "--seed",
            "2026",
        ]);
        let particles = ParticleSoa::random_with_profiles(
            n,
            args.seed,
            args.init,
            args.init_radius,
            args.init_spread,
            args.init_v_amp,
            args.init_lambda,
            args.init_center_x,
            args.init_center_y,
            args.mass_profile,
            args.mass_mean,
            args.mass_stddev,
            args.mass_min,
            args.mass_max,
            args.mass_alpha,
        );
        let mut tree = QuadTree::with_capacity(preflight_node_capacity(n)?);
        build_tree(&mut tree, &particles)?;
        let mut stack = Vec::with_capacity(preflight_node_capacity(n)?);

        let mut short_ax = vec![0.0; n - 1];
        let mut short_ay = vec![0.0; n];
        let err = compute_accel_barnes_hut(
            &particles,
            &tree,
            args.theta,
            args.epsilon,
            args.g,
            2,
            &mut stack,
            &mut short_ax,
            &mut short_ay,
        )
        .expect_err("short acceleration buffer should return Err");
        assert!(
            err.contains("acceleration buffer too short"),
            "unexpected error for short buffer: {err}"
        );

        let mut oversized_ax = vec![f64::NAN; n + 7];
        let mut oversized_ay = vec![f64::NAN; n + 7];
        compute_accel_barnes_hut(
            &particles,
            &tree,
            args.theta,
            args.epsilon,
            args.g,
            2,
            &mut stack,
            &mut oversized_ax,
            &mut oversized_ay,
        )?;

        assert!(oversized_ax[..n].iter().all(|v| v.is_finite()));
        assert!(oversized_ay[..n].iter().all(|v| v.is_finite()));
        assert!(oversized_ax[n..].iter().all(|v| v.is_nan()));
        assert!(oversized_ay[n..].iter().all(|v| v.is_nan()));

        Ok(())
    }

    #[test]
    fn traversal_stack_accounting_matches_allocated_stack_strategy() {
        let node_capacity = 100_001usize;
        assert_eq!(
            super::traversal_stack_bytes(node_capacity, 1),
            node_capacity * std::mem::size_of::<usize>()
        );
        assert_eq!(
            super::traversal_stack_bytes(node_capacity, 12),
            (node_capacity + 12 * super::TRAVERSAL_STACK_CAPACITY) * std::mem::size_of::<usize>()
        );
    }

    #[test]
    fn bh_force_matches_direct_for_small_theta_single_thread() -> Result<(), String> {
        let n = 128usize;
        let args = Args::parse_from([
            "nq",
            "--n",
            "128",
            "--steps",
            "1",
            "--dt",
            "0.001",
            "--theta",
            "0.0001",
            "--epsilon",
            "0.01",
            "--g",
            "0.9",
            "--integrator",
            "leapfrog",
            "--seed",
            "777",
        ]);
        let particles = ParticleSoa::random_with_profiles(
            n,
            args.seed,
            args.init,
            args.init_radius,
            args.init_spread,
            args.init_v_amp,
            args.init_lambda,
            args.init_center_x,
            args.init_center_y,
            args.mass_profile,
            args.mass_mean,
            args.mass_stddev,
            args.mass_min,
            args.mass_max,
            args.mass_alpha,
        );
        let mut tree = QuadTree::with_capacity(preflight_node_capacity(n)?);
        build_tree(&mut tree, &particles)?;

        let mut bh_ax = vec![0.0; n];
        let mut bh_ay = vec![0.0; n];
        let mut stack = Vec::with_capacity(preflight_node_capacity(n)?);
        compute_accel_barnes_hut(
            &particles,
            &tree,
            args.theta,
            args.epsilon,
            args.g,
            1,
            &mut stack,
            &mut bh_ax,
            &mut bh_ay,
        )?;

        let mut direct_ax = vec![0.0; n];
        let mut direct_ay = vec![0.0; n];
        compute_direct_accel_with_g(
            &particles,
            args.epsilon,
            args.g,
            &mut direct_ax,
            &mut direct_ay,
        );

        for i in 0..n {
            let dx = (bh_ax[i] - direct_ax[i]).abs();
            let dy = (bh_ay[i] - direct_ay[i]).abs();
            assert!(dx < 1e-8, "x force mismatch at idx {i}: {}", dx);
            assert!(dy < 1e-8, "y force mismatch at idx {i}: {}", dy);
        }

        Ok(())
    }
}
