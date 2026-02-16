use clap::Parser;

mod config;
mod direct;
mod particle;
mod sim;
mod stats;
mod tree;

use config::Args;
use direct::run_direct;
use particle::ParticleSoa;

fn main() {
    let args = Args::parse();

    if args.n == 0 {
        eprintln!("N must be greater than zero");
        return;
    }

    let mut particles = ParticleSoa::random(args.n, args.seed);
    let validate_particles = if should_validate(&args) {
        Some(particles.clone())
    } else {
        None
    };

    let mode_hint = args.mode.to_lowercase();

    let result = match mode_hint.as_str() {
        "barnes_hut" | "barneshut" | "bh" => {
            sim::run_barnes_hut(&mut particles, &args).map(|stats| ("barnes_hut".to_string(), stats))
        }
        "direct" => {
            run_direct(&mut particles, &args).map(|stats| ("direct".to_string(), stats))
        }
        other => {
            eprintln!("unknown mode: {}. use --mode=barnes_hut or --mode=direct", other);
            return;
        }
    };

    match result {
        Ok((mode_name, stats)) => {
            println!(
                "mode={} n={} steps={} theta={} epsilon={} dt={} build_ms={:.3} force_ms={:.3} integrate_ms={:.3} peak_nodes={} node_capacity={} ",
                mode_name,
                args.n,
                args.steps,
                args.theta,
                args.epsilon,
                args.dt,
                stats.build_ms,
                stats.force_ms,
                stats.integrate_ms,
                stats.peak_node_count,
                stats.node_capacity,
            );

            if let Some(reference_particles) = validate_particles {
                if mode_name == "barnes_hut" {
                    validate_against_direct(&args, &particles, reference_particles);
                } else if mode_name == "direct" {
                    println!("validate=skipped mode=direct");
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

fn validate_against_direct(args: &Args, fast: &ParticleSoa, mut ref_particles: ParticleSoa) {
    if run_direct(&mut ref_particles, args).is_err() {
        eprintln!("validation failed: unable to run reference direct mode");
        return;
    }

    let (rms_pos, rms_vel, max_pos, max_vel) = compare_states(fast, &ref_particles);
    println!(
        "validate=ok n={} steps={} rms_pos={} rms_vel={} max_pos={} max_vel={}",
        args.n,
        args.steps,
        rms_pos,
        rms_vel,
        max_pos,
        max_vel,
    );
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
