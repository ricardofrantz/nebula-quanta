use std::time::Instant;
use std::mem::size_of;

use crate::{config::Args, particle::ParticleSoa, stats::RunStats};

const G: f64 = 1.0;
const F64_BYTES: usize = size_of::<f64>();

pub fn run_direct(particles: &mut ParticleSoa, args: &Args) -> Result<RunStats, String> {
    if particles.len() == 0 {
        return Ok(RunStats::zero());
    }

    let n = particles.len();
    let mut ax = vec![0.0; n];
    let mut ay = vec![0.0; n];

    let mut build_elapsed = 0.0;
    let mut force_elapsed = 0.0;
    let mut integrate_elapsed = 0.0;

    let mut t = Instant::now();
    compute_direct_accel(particles, args.epsilon, &mut ax, &mut ay);
    build_elapsed += t.elapsed().as_secs_f64() * 1000.0;

    let mut step = 0;
    while step < args.steps {
        t = Instant::now();
        for i in 0..n {
            particles.vx[i] += 0.5 * ax[i] * args.dt;
            particles.vy[i] += 0.5 * ay[i] * args.dt;
            particles.x[i] += particles.vx[i] * args.dt;
            particles.y[i] += particles.vy[i] * args.dt;
        }
        integrate_elapsed += t.elapsed().as_secs_f64() * 1000.0;

        t = Instant::now();
        compute_direct_accel(particles, args.epsilon, &mut ax, &mut ay);
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
        peak_node_count: 0,
        node_capacity: 0,
        particle_count: n,
        particle_bytes: particle_state_bytes(n),
        node_pool_bytes: 0,
        traversal_stack_bytes: 0,
    })
}

pub fn compute_direct_accel(particles: &ParticleSoa, epsilon: f64, ax: &mut [f64], ay: &mut [f64]) {
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

            let coeff_i = G * particles.m[j] * inv_r3;
            let coeff_j = G * particles.m[i] * inv_r3;

            ax[i] += coeff_i * dx;
            ay[i] += coeff_i * dy;
            ax[j] -= coeff_j * dx;
            ay[j] -= coeff_j * dy;
        }
    }
}

fn particle_state_bytes(n: usize) -> usize {
    n.saturating_mul(5).saturating_mul(F64_BYTES)
}
