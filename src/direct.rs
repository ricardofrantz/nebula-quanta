use std::mem::size_of;
use std::time::Instant;

use crate::{config::Args, frame::FrameRecorder, particle::ParticleSoa, stats::RunStats};

const F64_BYTES: usize = size_of::<f64>();

pub fn run_direct(
    particles: &mut ParticleSoa,
    args: &Args,
    recorder: Option<&mut FrameRecorder>,
) -> Result<RunStats, String> {
    if particles.len() == 0 {
        return Ok(RunStats::zero());
    }

    let n = particles.len();
    let mut ax = vec![0.0; n];
    let mut ay = vec![0.0; n];
    let mut recorder = recorder;
    let workspace_bytes = particle_state_bytes(n);
    check_memory_budget(args, workspace_bytes)?;

    let mut build_elapsed = 0.0;
    let mut force_elapsed = 0.0;
    let mut integrate_elapsed = 0.0;
    let mut rk2_particles = particles.clone();
    let mut rk2_ax = vec![0.0; n];
    let mut rk2_ay = vec![0.0; n];

    let mut t = Instant::now();
    compute_direct_accel_with_g(particles, args.epsilon, args.g, &mut ax, &mut ay);
    build_elapsed += t.elapsed().as_secs_f64() * 1000.0;
    if let Some(recorder) = recorder.as_deref_mut() {
        recorder.record_step(0, particles, particle_bounds(particles)?)?;
    }

    let mut step = 0;
    while step < args.steps {
        t = Instant::now();
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
                integrate_rk2_direct_step(
                    particles,
                    &mut rk2_particles,
                    args.dt,
                    args.g,
                    args.epsilon,
                    &ax,
                    &ay,
                    &mut rk2_ax,
                    &mut rk2_ay,
                );
            }
        }
        integrate_elapsed += t.elapsed().as_secs_f64() * 1000.0;
        if let Some(recorder) = recorder.as_deref_mut() {
            recorder.record_step(step + 1, particles, particle_bounds(particles)?)?;
        }

        t = Instant::now();
        compute_direct_accel_with_g(particles, args.epsilon, args.g, &mut ax, &mut ay);
        force_elapsed += t.elapsed().as_secs_f64() * 1000.0;

        t = Instant::now();
        match args.integrator {
            crate::config::Integrator::Leapfrog | crate::config::Integrator::Verlet => {
                for i in 0..n {
                    particles.vx[i] += 0.5 * ax[i] * args.dt;
                    particles.vy[i] += 0.5 * ay[i] * args.dt;
                }
            }
            crate::config::Integrator::Rk2 => {}
        }
        integrate_elapsed += t.elapsed().as_secs_f64() * 1000.0;

        step += 1;
    }

    Ok(RunStats {
        build_ms: build_elapsed,
        force_ms: force_elapsed,
        integrate_ms: integrate_elapsed,
        peak_node_count: 0,
        node_capacity: 0,
        particle_count: n,
        particle_bytes: particle_state_bytes(n),
        node_pool_bytes: 0,
        traversal_stack_bytes: 0,
    })
}

pub fn compute_direct_accel(particles: &ParticleSoa, epsilon: f64, ax: &mut [f64], ay: &mut [f64]) {
    compute_direct_accel_with_g(particles, epsilon, 1.0, ax, ay)
}

pub fn compute_direct_accel_with_g(
    particles: &ParticleSoa,
    epsilon: f64,
    g: f64,
    ax: &mut [f64],
    ay: &mut [f64],
) {
    let n = particles.len();
    for i in 0..n {
        ax[i] = 0.0;
        ay[i] = 0.0;
    }

    let eps2 = epsilon * epsilon;

    for i in 0..n {
        for j in (i + 1)..n {
            let dx = particles.x[j] - particles.x[i];
            let dy = particles.y[j] - particles.y[i];
            let dist2 = dx * dx + dy * dy + eps2;
            if dist2 <= 0.0 {
                continue;
            }

            let inv_r3 = 1.0 / (dist2 * dist2.sqrt());
            let coeff_i = g * particles.m[j] * inv_r3;
            let coeff_j = g * particles.m[i] * inv_r3;

            ax[i] += coeff_i * dx;
            ay[i] += coeff_i * dy;
            ax[j] -= coeff_j * dx;
            ay[j] -= coeff_j * dy;
        }
    }
}

fn integrate_rk2_direct_step(
    particles: &mut ParticleSoa,
    mid_particles: &mut ParticleSoa,
    dt: f64,
    g: f64,
    epsilon: f64,
    ax: &[f64],
    ay: &[f64],
    mid_ax: &mut [f64],
    mid_ay: &mut [f64],
) {
    let n = particles.len();
    for i in 0..n {
        let vx_half = particles.vx[i] + 0.5 * ax[i] * dt;
        let vy_half = particles.vy[i] + 0.5 * ay[i] * dt;
        mid_particles.x[i] = particles.x[i] + 0.5 * particles.vx[i] * dt;
        mid_particles.y[i] = particles.y[i] + 0.5 * particles.vy[i] * dt;
        mid_particles.vx[i] = vx_half;
        mid_particles.vy[i] = vy_half;
        mid_particles.m[i] = particles.m[i];
    }

    compute_direct_accel_with_g(mid_particles, epsilon, g, mid_ax, mid_ay);

    for i in 0..n {
        particles.x[i] += mid_particles.vx[i] * dt;
        particles.y[i] += mid_particles.vy[i] * dt;
        particles.vx[i] += 0.5 * (ax[i] + mid_ax[i]) * dt;
        particles.vy[i] += 0.5 * (ay[i] + mid_ay[i]) * dt;
    }
}

fn particle_state_bytes(n: usize) -> usize {
    n.saturating_mul(7).saturating_mul(F64_BYTES)
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

fn particle_bounds(particles: &ParticleSoa) -> Result<(f64, f64, f64, f64), String> {
    let n = particles.len();
    if n == 0 {
        return Err("no particles available for frame bounds".to_string());
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

    let pad_x = ((x_max - x_min).abs() + (y_max - y_min).abs()) * 1e-12 + 1.0e-6;
    Ok((x_min - pad_x, x_max + pad_x, y_min - pad_x, y_max + pad_x))
}
