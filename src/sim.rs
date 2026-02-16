use std::mem::size_of;
use std::time::Instant;

use crate::{
    config::Args,
    direct::run_direct as run_direct_reference,
    frame::FrameRecorder,
    particle::ParticleSoa,
    stats::RunStats,
    tree::{Node, QuadTree},
};

const G: f64 = 1.0;

pub fn run_barnes_hut(
    particles: &mut ParticleSoa,
    args: &Args,
    recorder: Option<&mut FrameRecorder>,
) -> Result<RunStats, String> {
    if particles.len() == 0 {
        return Ok(RunStats::zero());
    }

    if args.theta <= 0.0 {
        return run_direct_reference(particles, args, None);
    }

    let n = particles.len();
    let node_capacity = n
        .checked_mul(4)
        .and_then(|v| v.checked_add(1))
        .ok_or_else(|| "node capacity overflow for requested particle count".to_string())?;
    let particle_state_bytes = particle_state_bytes(n);
    let node_pool_bytes = node_pool_bytes(node_capacity);
    let traversal_stack_bytes = traversal_stack_bytes(node_capacity);

    let mut tree = QuadTree::with_capacity(node_capacity);
    let mut ax = vec![0.0; n];
    let mut ay = vec![0.0; n];
    let mut traversal = Vec::with_capacity(node_capacity);
    let mut recorder = recorder;

    let mut build_elapsed = 0.0;
    let mut force_elapsed = 0.0;
    let mut integrate_elapsed = 0.0;
    let mut peak_node_count = 0usize;

    let mut step_start = Instant::now();
    build_tree(&mut tree, particles)?;
    peak_node_count = peak_node_count.max(tree.nodes.len());
    build_elapsed += step_start.elapsed().as_secs_f64() * 1000.0;
    if let Some(recorder) = recorder.as_deref_mut() {
        if let Some(bounds) = tree.root_bounds() {
            recorder.record_step(0, particles, bounds)?;
        }
    }

    step_start = Instant::now();
    compute_accel_barnes_hut(
        particles,
        &tree,
        args.theta,
        args.epsilon,
        &mut ax,
        &mut ay,
        &mut traversal,
    )?;
    force_elapsed += step_start.elapsed().as_secs_f64() * 1000.0;

    let mut step = 0;
    while step < args.steps {
        let mut t = Instant::now();
        for i in 0..n {
            particles.vx[i] += 0.5 * ax[i] * args.dt;
            particles.vy[i] += 0.5 * ay[i] * args.dt;
            particles.x[i] += particles.vx[i] * args.dt;
            particles.y[i] += particles.vy[i] * args.dt;
        }
        integrate_elapsed += t.elapsed().as_secs_f64() * 1000.0;

        t = Instant::now();
        build_tree(&mut tree, particles)?;
        peak_node_count = peak_node_count.max(tree.nodes.len());
        build_elapsed += t.elapsed().as_secs_f64() * 1000.0;
        if let Some(recorder) = recorder.as_deref_mut() {
            if let Some(bounds) = tree.root_bounds() {
                recorder.record_step(step + 1, particles, bounds)?;
            }
        }

        t = Instant::now();
        compute_accel_barnes_hut(
            particles,
            &tree,
            args.theta,
            args.epsilon,
            &mut ax,
            &mut ay,
            &mut traversal,
        )?;
        force_elapsed += t.elapsed().as_secs_f64() * 1000.0;

        t = Instant::now();
        for i in 0..n {
            particles.vx[i] += 0.5 * ax[i] * args.dt;
            particles.vy[i] += 0.5 * ay[i] * args.dt;
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
    })
}

fn build_tree(tree: &mut QuadTree, particles: &ParticleSoa) -> Result<(), String> {
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
    let pad = if spread == 0.0 { 1.0e-6 } else { spread * 1e-12 };
    tree.reset(
        x_min - pad,
        x_max + pad,
        y_min - pad,
        y_max + pad,
    );

    for i in 0..n {
        insert_into_node(tree, particles, 0, i)?;
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

        {
            let node = &mut tree.nodes[node_idx];
            node.mass = 0.0;
            node.com_x = 0.0;
            node.com_y = 0.0;
            node.body_idx = -1;
        }

        insert_into_node(tree, particles, child_indices[choose_child(
            tree.nodes[node_idx].x_min,
            tree.nodes[node_idx].x_max,
            tree.nodes[node_idx].y_min,
            tree.nodes[node_idx].y_max,
            particles.x[existing_body],
            particles.y[existing_body],
        )?] as usize, existing_body)?;
        insert_into_node(
            tree,
            particles,
            child_indices[choose_child(
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

    let child = choose_child(
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

fn split_leaf(tree: &mut QuadTree, node_idx: usize) -> Result<(), String> {
    if !tree.nodes[node_idx].is_leaf() {
        return Ok(());
    }

    if tree.nodes.len() + 4 > tree.nodes.capacity() {
        return Err(format!(
            "tree node capacity exceeded while splitting node {} (n={})",
            node_idx,
            tree.nodes.len()
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

fn choose_child(x_min: f64, x_max: f64, y_min: f64, y_max: f64, x: f64, y: f64) -> Result<usize, String> {
    let x_mid = 0.5 * (x_min + x_max);
    let y_mid = 0.5 * (y_min + y_max);

    let east = x >= x_mid;
    let north = y >= y_mid;

    Ok(match (east, north) {
        (false, true) => 0,
        (true, true) => 1,
        (false, false) => 2,
        (true, false) => 3,
    })
}

fn compute_accel_barnes_hut(
    particles: &ParticleSoa,
    tree: &QuadTree,
    theta: f64,
    epsilon: f64,
    ax: &mut [f64],
    ay: &mut [f64],
    stack: &mut Vec<usize>,
) -> Result<(), String> {
    let n = particles.len();
    if n == 0 {
        return Ok(());
    }

    let eps2 = epsilon * epsilon;
    let theta2 = theta * theta;

    for i in 0..n {
        ax[i] = 0.0;
        ay[i] = 0.0;
        stack.clear();
        stack.push(0);

        while let Some(node_idx) = stack.pop() {
            let node = &tree.nodes[node_idx];
            if node.mass <= 0.0 {
                continue;
            }

            let dx = node.com_x - particles.x[i];
            let dy = node.com_y - particles.y[i];
            let dist2 = dx * dx + dy * dy;
            let dist2_soft = dist2 + eps2;
            if dist2_soft <= 0.0 {
                continue;
            }

            if node.body_idx >= 0 {
                let body = node.body_idx as usize;
                if body != i {
                    let inv_r3 = 1.0 / (dist2_soft * dist2_soft.sqrt());
                    let coeff = G * particles.m[body] * inv_r3;
                    ax[i] += coeff * dx;
                    ay[i] += coeff * dy;
                }
                continue;
            }

            let size = node.size();
            if size * size <= theta2 * dist2 {
                let inv_r3 = 1.0 / (dist2_soft * dist2_soft.sqrt());
                let coeff = G * node.mass * inv_r3;
                ax[i] += coeff * dx;
                ay[i] += coeff * dy;
                continue;
            }

            for child in &node.children {
                if *child >= 0 {
                    stack.push(*child as usize);
                }
            }
        }
    }

    Ok(())
}

const F64_BYTES: usize = size_of::<f64>();
const NODE_BYTES: usize = size_of::<Node>();
const USIZE_BYTES: usize = size_of::<usize>();

fn particle_state_bytes(n: usize) -> usize {
    n.saturating_mul(5).saturating_mul(F64_BYTES)
}

fn node_pool_bytes(nodes: usize) -> usize {
    nodes.saturating_mul(NODE_BYTES)
}

fn traversal_stack_bytes(slots: usize) -> usize {
    slots.saturating_mul(USIZE_BYTES)
}
