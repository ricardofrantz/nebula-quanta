use std::mem::size_of;
use std::time::Instant;

use crate::{
    config::Args,
    frame::FrameRecorder,
    particle::{ParticleSoa, particle_bounds},
    stats::RunStats,
};

const F64_BYTES: usize = size_of::<f64>();

pub fn run_direct(
    particles: &mut ParticleSoa,
    args: &Args,
    recorder: Option<&mut FrameRecorder>,
) -> Result<RunStats, String> {
    if particles.is_empty() {
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
    let g_is_unity = (args.g - 1.0).abs() <= f64::EPSILON;
    let mut epsilon = args.epsilon_for_step(0, n, particle_bounds(particles).ok());

    let mut t = Instant::now();
    if g_is_unity {
        compute_direct_accel(particles, epsilon, &mut ax, &mut ay);
    } else {
        compute_direct_accel_with_g(particles, epsilon, args.g, &mut ax, &mut ay);
    }
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
                    epsilon,
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
        epsilon = args.epsilon_for_step(step + 1, n, particle_bounds(particles).ok());
        if g_is_unity {
            compute_direct_accel(particles, epsilon, &mut ax, &mut ay);
        } else {
            compute_direct_accel_with_g(particles, epsilon, args.g, &mut ax, &mut ay);
        }
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
    #[cfg(feature = "simd")]
    {
        if particles.len() >= 2048 {
            compute_direct_accel_with_g_simd(particles, epsilon, g, ax, ay);
        } else {
            compute_direct_accel_with_g_scalar(particles, epsilon, g, ax, ay);
        }
    }

    #[cfg(not(feature = "simd"))]
    {
        compute_direct_accel_with_g_scalar(particles, epsilon, g, ax, ay);
    }
}

fn compute_direct_accel_with_g_scalar(
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

#[cfg(feature = "simd")]
fn compute_direct_accel_with_g_simd(
    particles: &ParticleSoa,
    epsilon: f64,
    g: f64,
    ax: &mut [f64],
    ay: &mut [f64],
) {
    const SIMD_LANE: usize = 4;

    let n = particles.len();
    for i in 0..n {
        ax[i] = 0.0;
        ay[i] = 0.0;
    }

    let eps2 = epsilon * epsilon;
    for i in 0..n {
        let xi = particles.x[i];
        let yi = particles.y[i];
        let mi = particles.m[i];
        let mut j = i + 1;

        while j + SIMD_LANE <= n {
            let j0 = j;
            let j1 = j + 1;
            let j2 = j + 2;
            let j3 = j + 3;

            let dx0 = particles.x[j0] - xi;
            let dy0 = particles.y[j0] - yi;
            let dx1 = particles.x[j1] - xi;
            let dy1 = particles.y[j1] - yi;
            let dx2 = particles.x[j2] - xi;
            let dy2 = particles.y[j2] - yi;
            let dx3 = particles.x[j3] - xi;
            let dy3 = particles.y[j3] - yi;

            let dist2_0 = dx0 * dx0 + dy0 * dy0 + eps2;
            let dist2_1 = dx1 * dx1 + dy1 * dy1 + eps2;
            let dist2_2 = dx2 * dx2 + dy2 * dy2 + eps2;
            let dist2_3 = dx3 * dx3 + dy3 * dy3 + eps2;

            let inv_r3_0 = if dist2_0 > 0.0 {
                1.0 / (dist2_0 * dist2_0.sqrt())
            } else {
                0.0
            };
            let inv_r3_1 = if dist2_1 > 0.0 {
                1.0 / (dist2_1 * dist2_1.sqrt())
            } else {
                0.0
            };
            let inv_r3_2 = if dist2_2 > 0.0 {
                1.0 / (dist2_2 * dist2_2.sqrt())
            } else {
                0.0
            };
            let inv_r3_3 = if dist2_3 > 0.0 {
                1.0 / (dist2_3 * dist2_3.sqrt())
            } else {
                0.0
            };

            let coeff_i0 = g * particles.m[j0] * inv_r3_0;
            let coeff_i1 = g * particles.m[j1] * inv_r3_1;
            let coeff_i2 = g * particles.m[j2] * inv_r3_2;
            let coeff_i3 = g * particles.m[j3] * inv_r3_3;

            let cpx = coeff_i0 * dx0 + coeff_i1 * dx1 + coeff_i2 * dx2 + coeff_i3 * dx3;
            let cpy = coeff_i0 * dy0 + coeff_i1 * dy1 + coeff_i2 * dy2 + coeff_i3 * dy3;
            ax[i] += cpx;
            ay[i] += cpy;

            let coeff_j = g * mi;
            if dist2_0 > 0.0 {
                let c = coeff_j * inv_r3_0;
                ax[j0] -= c * dx0;
                ay[j0] -= c * dy0;
            }
            if dist2_1 > 0.0 {
                let c = coeff_j * inv_r3_1;
                ax[j1] -= c * dx1;
                ay[j1] -= c * dy1;
            }
            if dist2_2 > 0.0 {
                let c = coeff_j * inv_r3_2;
                ax[j2] -= c * dx2;
                ay[j2] -= c * dy2;
            }
            if dist2_3 > 0.0 {
                let c = coeff_j * inv_r3_3;
                ax[j3] -= c * dx3;
                ay[j3] -= c * dy3;
            }

            j += SIMD_LANE;
        }

        while j < n {
            let dx = particles.x[j] - xi;
            let dy = particles.y[j] - yi;
            let dist2 = dx * dx + dy * dy + eps2;
            if dist2 <= 0.0 {
                j += 1;
                continue;
            }

            let inv_r3 = 1.0 / (dist2 * dist2.sqrt());
            let coeff_i = g * particles.m[j] * inv_r3;
            let coeff_j = g * particles.m[i] * inv_r3;

            ax[i] += coeff_i * dx;
            ay[i] += coeff_i * dy;
            ax[j] -= coeff_j * dx;
            ay[j] -= coeff_j * dy;

            j += 1;
        }
    }
}

#[allow(clippy::too_many_arguments)]
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
        // Explicit midpoint RK2: advance both position and velocity using the
        // midpoint velocity/acceleration estimated from the start-of-step state.
        particles.x[i] += mid_particles.vx[i] * dt;
        particles.y[i] += mid_particles.vy[i] * dt;
        particles.vx[i] += mid_ax[i] * dt;
        particles.vy[i] += mid_ay[i] * dt;
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
