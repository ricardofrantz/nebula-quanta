use clap::Parser;

mod config;
mod direct;
mod particle;
mod sim;
mod stats;
mod frame;
mod tree;

use config::Args;
use config::Dimensionality;
use frame::FrameRecorder;
use direct::{compute_direct_accel_with_g, run_direct};
use particle::{
    compute_energy_snapshot,
    particle_bounds,
    total_momentum,
    EnergySnapshot,
    ParticleSoa,
};
use std::path::PathBuf;

fn main() {
    let args = Args::parse();

    if args.n == 0 {
        eprintln!("N must be greater than zero");
        return;
    }
    if args.threads == 0 {
        eprintln!("--threads must be at least one");
        return;
    }
    if args.dim == Dimensionality::Three {
        eprintln!("--dim=3 is planned but not implemented in this release; use --dim 2");
        return;
    }

    let mut particles = ParticleSoa::random_with_profiles(
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
    );
    let initial_epsilon = args.epsilon_for_step(0, particles.len(), particle_bounds(&particles).ok());
    let initial_energy = if args.should_measure_energy_snapshot() {
        compute_energy_snapshot(
            &particles,
            initial_epsilon,
            args.g,
            args.energy_sample_ratio,
            args.should_measure_energy_drift(),
            args.seed.wrapping_add(0x9E3779B97F4A7C15),
        )
    } else {
        None
    };
    let initial_momentum = total_momentum(&particles);
    let validate_particles = if should_validate(&args) {
        Some(particles.clone())
    } else {
        None
    };
    let mut recorder = match make_recorder(&args) {
        Ok(Some(recorder)) => Some(recorder),
        Ok(None) => None,
        Err(err) => {
            eprintln!("recording disabled: {err}");
            None
        }
    };

    let mode_hint = args.mode.to_lowercase();

    let result = match mode_hint.as_str() {
        "barnes_hut" | "barneshut" | "bh" => {
            sim::run_barnes_hut(&mut particles, &args, recorder.as_mut())
                .map(|stats| ("barnes_hut".to_string(), stats))
        }
        "direct" => {
            run_direct(&mut particles, &args, recorder.as_mut())
                .map(|stats| ("direct".to_string(), stats))
        }
        other => {
            eprintln!("unknown mode: {}. use --mode=barnes_hut or --mode=direct", other);
            return;
        }
    };

    match result {
        Ok((mode_name, stats)) => {
            let final_epsilon = args.epsilon_for_step(
                args.steps,
                particles.len(),
                particle_bounds(&particles).ok(),
            );
            let final_energy = if args.should_measure_energy_snapshot() {
                compute_energy_snapshot(
                    &particles,
                    final_epsilon,
                    args.g,
                    args.energy_sample_ratio,
                    args.should_measure_energy_drift(),
                    args.seed.wrapping_add(0x9E3779B97F4A7C15),
                )
            } else {
                None
            };
            let final_momentum = total_momentum(&particles);
            let (initial_kinetic, initial_potential, initial_total, initial_pairs) =
                energy_format(&initial_energy);
            let (final_kinetic, final_potential, final_total, final_pairs) =
                energy_format(&final_energy);

            let (energy_drift_abs, energy_drift_rel) = initial_energy.zip(final_energy).map_or(
                ("na".to_string(), "na".to_string()),
                |(start, end)| {
                    let abs = (end.total - start.total).abs();
                    let rel = if start.total.abs() > 0.0 {
                        abs / start.total.abs()
                    } else {
                        0.0
                    };
                    (format!("{:.9}", abs), format!("{:.9}", rel))
                },
            );
            let delta_px = final_momentum.px - initial_momentum.px;
            let delta_py = final_momentum.py - initial_momentum.py;
            let delta_pmag = final_momentum.momentum_mag - initial_momentum.momentum_mag;
            let delta_lz = final_momentum.angular_momentum_z - initial_momentum.angular_momentum_z;
            let mut validate_force_rms = "na".to_string();
            let mut validate_force_max = "na".to_string();
            let mut validate_energy_abs = "na".to_string();
            let mut validate_energy_rel = "na".to_string();

            if let Some(reference_particles) = validate_particles {
                if mode_name == "barnes_hut" {
                    if let Some(metrics) = validate_against_direct(
                        &args,
                        &particles,
                        reference_particles,
                        final_epsilon,
                    ) {
                        validate_force_rms = format!("{:.9}", metrics.force_rms);
                        validate_force_max = format!("{:.9}", metrics.force_max);
                        validate_energy_abs = format!("{:.9}", metrics.energy_abs);
                        validate_energy_rel = format!("{:.9}", metrics.energy_rel);
                    }
                } else if mode_name == "direct" {
                    println!("validate=skipped mode=direct");
                }
            }

            println!(
                "mode={} n={} steps={} dim={:?} theta={} theta_policy={:?} theta_density_scale={} softening_policy={:?} softening_density_scale={} epsilon={} dt={} g={} integrator={:?} threads={} init={:?} mass_profile={:?} init_radius={} init_spread={} init_v_amp={} init_lambda={} mass_mean={} mass_stddev={} mass_min={} mass_max={} mass_alpha={} build_ms={:.3} force_ms={:.3} integrate_ms={:.3} total_ms={:.3} avg_step_ms={:.3} steps_per_sec={:.3} ns_per_particle_force={:.1} peak_nodes={} node_capacity={} node_utilization={:.2}% workspace_bytes={} bytes_per_particle={:.1} particle_bytes={} node_bytes={} stack_bytes={} initial_ke={} initial_pe={} initial_te={} sampled_pairs={} final_ke={} final_pe={} final_te={} final_sampled_pairs={} energy_drift_abs={} energy_drift_rel={} validate_force_rms={} validate_force_max={} validate_energy_abs={} validate_energy_rel={} p0_x={} p0_y={} p0_mag={} lz0={} p1_x={} p1_y={} p1_mag={} lz1={} dp_x={} dp_y={} dp_mag={} dp_lz={}",
                mode_name,
                args.n,
                args.steps,
                args.dim,
                args.theta,
                args.theta_policy,
                args.theta_density_scale,
                args.softening_policy,
                args.softening_density_scale,
                args.epsilon,
                args.dt,
                args.g,
                args.integrator,
                args.threads,
                args.init,
                args.mass_profile,
                args.init_radius,
                args.init_spread,
                args.init_v_amp,
                args.init_lambda,
                args.mass_mean,
                args.mass_stddev,
                args.mass_min,
                args.mass_max,
                args.mass_alpha,
                stats.build_ms,
                stats.force_ms,
                stats.integrate_ms,
                stats.total_ms(),
                stats.avg_step_ms(args.steps),
                stats.steps_per_sec(args.steps),
                stats.force_ns_per_particle_step(args.steps, args.n),
                stats.peak_node_count,
                stats.node_capacity,
                stats.node_utilization() * 100.0,
                stats.workspace_bytes(),
                stats.bytes_per_particle(),
                stats.particle_bytes,
                stats.node_pool_bytes,
                stats.traversal_stack_bytes,
                initial_kinetic,
                initial_potential,
                initial_total,
                initial_pairs,
                final_kinetic,
                final_potential,
                final_total,
                final_pairs,
                energy_drift_abs,
                energy_drift_rel,
                validate_force_rms,
                validate_force_max,
                validate_energy_abs,
                validate_energy_rel,
                initial_momentum.px,
                initial_momentum.py,
                initial_momentum.momentum_mag,
                initial_momentum.angular_momentum_z,
                final_momentum.px,
                final_momentum.py,
                final_momentum.momentum_mag,
                final_momentum.angular_momentum_z,
                delta_px,
                delta_py,
                delta_pmag,
                delta_lz,
            );

            if let Some(recorder) = recorder.as_ref() {
                if recorder.frame_count() > 0 {
                    let output_name = PathBuf::from(format!("nebula-quanta-{}.mp4", mode_name))
                        .to_string_lossy()
                        .into_owned();
                    println!(
                        "record_frames={} render_cmd=\"{}\"",
                        recorder.frame_count(),
                        recorder.render_command(&output_name, args.fps),
                    );
                }
            }
        }
        Err(err) => {
            eprintln!("simulation failed: {}", err);
        }
    }
}

fn should_validate(args: &Args) -> bool {
    if !args.validate {
        return false;
    }

    let mode_hint = args.mode.to_lowercase();

    if mode_hint != "barnes_hut"
        && mode_hint != "barneshut"
        && mode_hint != "bh"
    {
        return false;
    }

    if args.steps > 8_192 {
        eprintln!("validate=true but steps too high; validation disabled");
        return false;
    }

    if args.n > 8_192 {
        eprintln!("validate=true but n too high; validation disabled");
        return false;
    }

    true
}

fn energy_format(snapshot: &Option<EnergySnapshot>) -> (String, String, String, String) {
    match snapshot {
        Some(value) => (
            format!("{:.9}", value.kinetic),
            format!("{:.9}", value.potential),
            format!("{:.9}", value.total),
            format!("{}", value.sampled_pairs),
        ),
        None => (
            "na".to_string(),
            "na".to_string(),
            "na".to_string(),
            "na".to_string(),
        ),
    }
}

#[derive(Debug, Clone, Copy)]
struct ValidationMetrics {
    force_rms: f64,
    force_max: f64,
    energy_abs: f64,
    energy_rel: f64,
}

fn validate_against_direct(
    args: &Args,
    fast: &ParticleSoa,
    mut ref_particles: ParticleSoa,
    final_epsilon: f64,
) -> Option<ValidationMetrics> {
    if run_direct(&mut ref_particles, args, None).is_err() {
        eprintln!("validation failed: unable to run reference direct mode");
        return None;
    }

    let (rms_pos, rms_vel, max_pos, max_vel) = compare_states(fast, &ref_particles);
    let seed = args
        .seed
        .wrapping_add(0x9E3779B97F4A7C15);
    let fast_energy = compute_energy_snapshot(
        fast,
        final_epsilon,
        args.g,
        0.0,
        true,
        seed,
    );
    let direct_energy = compute_energy_snapshot(
        &ref_particles,
        final_epsilon,
        args.g,
        0.0,
        true,
        seed,
    );
    let (energy_abs, energy_rel) = match (fast_energy, direct_energy) {
        (Some(current), Some(reference)) => {
            let abs = (current.total - reference.total).abs();
            let rel = if reference.total.abs() > 0.0 {
                abs / reference.total.abs()
            } else {
                0.0
            };
            (abs, rel)
        }
        _ => {
            eprintln!("validation failed: unable to compute validation energy");
            return None;
        }
    };

    let (force_rms, force_max) = match compute_force_error(fast, args, final_epsilon) {
        Some(values) => values,
        None => {
            eprintln!("validation failed: unable to compute Barnes-Hut validation forces");
            return None;
        }
    };

    println!(
        "validate=ok n={} steps={} rms_pos={} rms_vel={} max_pos={} max_vel={} force_rms={} force_max={} energy_abs={} energy_rel={}",
        args.n,
        args.steps,
        rms_pos,
        rms_vel,
        max_pos,
        max_vel,
        force_rms,
        force_max,
        energy_abs,
        energy_rel,
    );

    Some(ValidationMetrics {
        force_rms,
        force_max,
        energy_abs,
        energy_rel,
    })
}

fn compute_force_error(fast: &ParticleSoa, args: &Args, epsilon: f64) -> Option<(f64, f64)> {
    let (fast_bh_ax, fast_bh_ay) = match sim::compute_barnes_hut_accel_snapshot(fast, args) {
        Ok(values) => values,
        Err(err) => {
            eprintln!("validation failed to compute bh validation force snapshot: {err}");
            return None;
        }
    };

    let n = fast.len();
    if n == 0 {
        return Some((0.0, 0.0));
    }

    let mut direct_ax = vec![0.0; n];
    let mut direct_ay = vec![0.0; n];
    compute_direct_accel_with_g(fast, epsilon, args.g, &mut direct_ax, &mut direct_ay);

    let mut force_rms_sum = 0.0;
    let mut force_max = 0.0;
    for i in 0..n {
        let dx = fast_bh_ax[i] - direct_ax[i];
        let dy = fast_bh_ay[i] - direct_ay[i];
        let err = (dx * dx + dy * dy).sqrt();
        force_rms_sum += err * err;
        if err > force_max {
            force_max = err;
        }
    }

    Some(((force_rms_sum / n as f64).sqrt(), force_max))
}

fn make_recorder(args: &Args) -> Result<Option<FrameRecorder>, String> {
    if !args.record {
        return Ok(None);
    }

    if args.fps == 0 {
        return Err("fps must be greater than zero for recording".to_string());
    }

    FrameRecorder::new(args).map(Some)
}

fn compare_states(a: &ParticleSoa, b: &ParticleSoa) -> (f64, f64, f64, f64) {
    let n = a.len().min(b.len());
    let mut sum_pos = 0.0;
    let mut sum_vel = 0.0;
    let mut max_pos = 0.0;
    let mut max_vel = 0.0;

    for i in 0..n {
        let dx = a.x[i] - b.x[i];
        let dy = a.y[i] - b.y[i];
        let dvx = a.vx[i] - b.vx[i];
        let dvy = a.vy[i] - b.vy[i];

        let pos_err = (dx * dx + dy * dy).sqrt();
        let vel_err = (dvx * dvx + dvy * dvy).sqrt();

        sum_pos += pos_err * pos_err;
        sum_vel += vel_err * vel_err;
        if pos_err > max_pos {
            max_pos = pos_err;
        }
        if vel_err > max_vel {
            max_vel = vel_err;
        }
    }

    let denom = (n as f64).max(1.0);
    ((sum_pos / denom).sqrt(), (sum_vel / denom).sqrt(), max_pos, max_vel)
}
