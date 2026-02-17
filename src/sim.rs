use std::mem::size_of;
use std::time::Instant;

use crate::{
    config::Args,
    direct::{compute_direct_accel_with_g, run_direct as run_direct_reference},
    frame::FrameRecorder,
    particle::{particle_bounds, ParticleSoa},
    stats::RunStats,
    tree::{Node, QuadTree},
};

pub fn compute_barnes_hut_accel_snapshot(
    particles: &ParticleSoa,
    args: &Args,
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

    let thread_count = args.threads.max(1);
    let active_threads = thread_count.min(n).max(1);
    let node_capacity = preflight_node_capacity(n)?;
    let mut tree = QuadTree::with_capacity(node_capacity);
    let mut ax = vec![0.0; n];
    let mut ay = vec![0.0; n];
    let mut stack = Vec::with_capacity(node_capacity);
    let mut traversal_stacks: Vec<Vec<usize>> = if active_threads > 1 {
        (0..active_threads).map(|_| Vec::with_capacity(node_capacity)).collect()
    } else {
        Vec::new()
    };

    build_tree(&mut tree, particles)?;
    let effective_theta = args.theta_for_step(0, n, tree.root_bounds());
    let epsilon = args.epsilon_for_step(0, n, tree.root_bounds());
    compute_accel_barnes_hut(
        particles,
        &tree,
        effective_theta,
        epsilon,
        args.g,
        active_threads,
        &mut stack,
        &mut traversal_stacks,
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
    if particles.len() == 0 {
        return Ok(RunStats::zero());
    }

    if args.theta <= 0.0 {
        return run_direct_reference(particles, args, None);
    }

    let n = particles.len();
    let thread_count = args.threads.max(1);
    let active_threads = thread_count.min(n).max(1);
    let node_capacity = preflight_node_capacity(n)?;
    let particle_state_bytes = particle_state_bytes(n);
    let node_pool_bytes = node_pool_bytes(node_capacity);
    let traversal_stack_bytes = traversal_stack_bytes(node_capacity, active_threads);
    let workspace_bytes = particle_state_bytes + node_pool_bytes + traversal_stack_bytes;
    check_memory_budget(args, workspace_bytes)?;

    let mut tree = QuadTree::with_capacity(node_capacity);
    let mut ax = vec![0.0; n];
    let mut ay = vec![0.0; n];
    let mut traversal = Vec::with_capacity(node_capacity);
    let mut traversal_stacks: Vec<Vec<usize>> = if active_threads > 1 {
        (0..active_threads).map(|_| Vec::with_capacity(node_capacity)).collect()
    } else {
        Vec::new()
    };
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
        theta,
        epsilon,
        args.g,
        active_threads,
        &mut traversal,
        &mut traversal_stacks,
        &mut ax,
        &mut ay,
    )?;
    force_elapsed += step_start.elapsed().as_secs_f64() * 1000.0;

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
                    &mut traversal_stacks,
                )?;
            }
        }
        integrate_elapsed += t.elapsed().as_secs_f64() * 1000.0;

        t = Instant::now();
        build_tree(&mut tree, particles)?;
        peak_node_count = peak_node_count.max(tree.nodes.len());
        build_elapsed += t.elapsed().as_secs_f64() * 1000.0;
        theta = args.theta_for_step(step + 1, n, tree.root_bounds());
        epsilon = args.epsilon_for_step(step + 1, n, tree.root_bounds());
        if let Some(recorder) = recorder.as_deref_mut() {
            if let Some(bounds) = tree.root_bounds() {
                recorder.record_step(step + 1, particles, bounds)?;
            }
        }

        t = Instant::now();
        compute_accel_barnes_hut(
            particles,
            &tree,
            theta,
            epsilon,
            args.g,
            active_threads,
            &mut traversal,
            &mut traversal_stacks,
            &mut ax,
            &mut ay,
        )?;
        force_elapsed += t.elapsed().as_secs_f64() * 1000.0;

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
    })
}

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
    traversal_stacks: &mut Vec<Vec<usize>>,
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

    build_tree(tree, mid_particles)?;

    compute_accel_barnes_hut(
        mid_particles,
        tree,
        theta,
        epsilon,
        g,
        thread_count,
        traversal,
        traversal_stacks,
        mid_ax,
        mid_ay,
    )?;

    for i in 0..particles.len() {
        particles.x[i] += mid_particles.vx[i] * dt;
        particles.y[i] += mid_particles.vy[i] * dt;
        particles.vx[i] += 0.5 * (ax[i] + mid_ax[i]) * dt;
        particles.vy[i] += 0.5 * (ay[i] + mid_ay[i]) * dt;
    }
    Ok(())
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
    g: f64,
    thread_count: usize,
    stack: &mut Vec<usize>,
    thread_stacks: &mut Vec<Vec<usize>>,
    ax: &mut [f64],
    ay: &mut [f64],
) -> Result<(), String> {
    let n = particles.len();
    if n == 0 {
        return Ok(());
    }

    let theta2 = theta * theta;
    let eps2 = epsilon * epsilon;

    if thread_count <= 1 {
        for i in 0..n {
            let (force_x, force_y) = compute_particle_force(
                i,
                particles,
                &tree.nodes,
                theta2,
                eps2,
                g,
                stack,
            );
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
        thread_stacks,
        ax,
        ay,
    )?;

    Ok(())
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

fn compute_accel_barnes_hut_parallel(
    particles: &ParticleSoa,
    nodes: &[Node],
    theta2: f64,
    eps2: f64,
    g: f64,
    thread_count: usize,
    stack: &mut Vec<usize>,
    thread_stacks: &mut Vec<Vec<usize>>,
    ax: &mut [f64],
    ay: &mut [f64],
) -> Result<(), String> {
    if thread_count <= 1 {
        return Ok(());
    }

    let n = particles.len();
    let active_threads = thread_count.min(n);
    if active_threads <= 1 {
        for i in 0..n {
            let (force_x, force_y) = compute_particle_force(i, particles, nodes, theta2, eps2, g, stack);
            ax[i] = force_x;
            ay[i] = force_y;
        }
        return Ok(());
    }

    let stack_capacity = stack.capacity().max(1);
    if thread_stacks.len() < active_threads {
        thread_stacks.extend((thread_stacks.len()..active_threads).map(|_| {
            Vec::with_capacity(stack_capacity)
        }));
    }
    for thread_stack in thread_stacks.iter_mut().take(active_threads) {
        if thread_stack.capacity() < stack_capacity {
            thread_stack.reserve(stack_capacity - thread_stack.capacity());
        }
    }

    let chunk_base = n / active_threads;
    let chunk_extra = n % active_threads;
    let stack_ptr = thread_stacks.as_mut_ptr();
    let ax_ptr = ax.as_mut_ptr();
    let ay_ptr = ay.as_mut_ptr();

    let thread_result: Result<(), String> = std::thread::scope(|scope| {
        let mut stack_handles = Vec::with_capacity(active_threads);
        for thread_id in 0..active_threads {
            let chunk_len = chunk_base + usize::from(thread_id < chunk_extra);
            let chunk_start = thread_id * chunk_base + thread_id.min(chunk_extra);
            let mut local_stack = unsafe { &mut *stack_ptr.add(thread_id) };
            local_stack.clear();
            let chunk_ax_ptr = unsafe { ax_ptr.add(chunk_start) } as usize;
            let chunk_ay_ptr = unsafe { ay_ptr.add(chunk_start) } as usize;

            let handle = scope.spawn(move || {
                let chunk_ax_ptr = chunk_ax_ptr as *mut f64;
                let chunk_ay_ptr = chunk_ay_ptr as *mut f64;
                for offset in 0..chunk_len {
                    let particle_idx = chunk_start + offset;
                    let (force_x, force_y) = compute_particle_force(
                        particle_idx,
                        particles,
                        nodes,
                        theta2,
                        eps2,
                        g,
                        &mut local_stack,
                    );
                    unsafe {
                        *chunk_ax_ptr.add(offset) = force_x;
                        *chunk_ay_ptr.add(offset) = force_y;
                    }
                }
                Ok::<(), String>(())
            });
            stack_handles.push(handle);
        }
        for handle in stack_handles {
            handle
                .join()
                .map_err(|_| "threaded Barnes-Hut force worker panicked".to_string())??;
        }
        Ok(())
    });

    thread_result?;

    Ok(())
}

const F64_BYTES: usize = size_of::<f64>();
const NODE_BYTES: usize = size_of::<Node>();
const USIZE_BYTES: usize = size_of::<usize>();

fn preflight_node_capacity(n: usize) -> Result<usize, String> {
    n.checked_mul(4)
        .and_then(|v| v.checked_add(1))
        .ok_or_else(|| "node capacity overflow for requested particle count".to_string())
}

fn check_memory_budget(args: &Args, workspace_bytes: usize) -> Result<(), String> {
    let Some(max_memory_mib) = args.max_memory_mib else {
        return Ok(());
    };

    let max_memory_bytes = max_memory_mib.checked_mul(1024 * 1024).ok_or_else(|| {
        format!("invalid --max-memory-mib value (overflow while converting to bytes): {max_memory_mib}")
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

fn traversal_stack_bytes(slots: usize, thread_count: usize) -> usize {
    let active_threads = thread_count.max(1);
    slots
        .saturating_mul(active_threads)
        .saturating_mul(USIZE_BYTES)
}

#[cfg(test)]
mod tests {
    use super::{
        build_tree,
        compute_accel_barnes_hut,
        preflight_node_capacity,
        run_barnes_hut,
    };
    use crate::{config::Args, direct::compute_direct_accel_with_g, direct::run_direct, particle::ParticleSoa, tree::QuadTree};

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

        let mut particles = ParticleSoa::random_with_profiles(
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

        let mut args = parse("0.0");
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
        let mut particles = ParticleSoa::random_with_profiles(
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
        let mut stacks = vec![Vec::with_capacity(preflight_node_capacity(n)?); 1];
        compute_accel_barnes_hut(
            &particles,
            &tree,
            args.theta,
            args.epsilon,
            args.g,
            1,
            &mut stack,
            &mut stacks,
            &mut bh_ax,
            &mut bh_ay,
        )?;

        let mut direct_ax = vec![0.0; n];
        let mut direct_ay = vec![0.0; n];
        compute_direct_accel_with_g(&particles, args.epsilon, args.g, &mut direct_ax, &mut direct_ay);

        for i in 0..n {
            let dx = (bh_ax[i] - direct_ax[i]).abs();
            let dy = (bh_ay[i] - direct_ay[i]).abs();
            assert!(dx < 1e-8, "x force mismatch at idx {i}: {}", dx);
            assert!(dy < 1e-8, "y force mismatch at idx {i}: {}", dy);
        }

        Ok(())
    }
}
