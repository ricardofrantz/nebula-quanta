use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use std::f64::consts::PI;

use crate::config::{InitProfile, MassProfile, MergerSpin};

#[derive(Clone, Debug)]
pub struct ParticleSoa {
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    pub vx: Vec<f64>,
    pub vy: Vec<f64>,
    pub m: Vec<f64>,
}

#[derive(Debug, Clone, Copy)]
pub struct InitParams {
    pub n: usize,
    pub seed: u64,
    pub init_profile: InitProfile,
    pub init_radius: f64,
    pub init_spread: f64,
    pub init_v_amp: f64,
    pub init_lambda: f64,
    pub init_center_x: f64,
    pub init_center_y: f64,
    pub mass_profile: MassProfile,
    pub mass_mean: f64,
    pub mass_stddev: f64,
    pub mass_min: f64,
    pub mass_max: f64,
    pub mass_alpha: f64,
    pub disk_scale_length: f64,
    pub disk_central_mass_frac: f64,
    pub disk_dispersion: f64,
    pub g: f64,
    pub merger_mass_ratio: f64,
    pub merger_separation: f64,
    pub merger_impact_parameter: f64,
    pub merger_v_rel: f64,
    pub merger_spin: MergerSpin,
}

#[derive(Debug, Clone, Copy)]
pub struct EnergySnapshot {
    pub kinetic: f64,
    pub potential: f64,
    pub total: f64,
    pub sampled_pairs: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct MomentumSnapshot {
    pub px: f64,
    pub py: f64,
    pub momentum_mag: f64,
    pub angular_momentum_z: f64,
}

impl ParticleSoa {
    #[allow(dead_code)]
    pub fn with_capacity(n: usize) -> Self {
        Self {
            x: Vec::with_capacity(n),
            y: Vec::with_capacity(n),
            vx: Vec::with_capacity(n),
            vy: Vec::with_capacity(n),
            m: Vec::with_capacity(n),
        }
    }

    pub fn with_len(n: usize) -> Self {
        Self {
            x: vec![0.0; n],
            y: vec![0.0; n],
            vx: vec![0.0; n],
            vy: vec![0.0; n],
            m: vec![1.0; n],
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn random_with_profiles(
        n: usize,
        seed: u64,
        init_profile: InitProfile,
        init_radius: f64,
        init_spread: f64,
        init_v_amp: f64,
        init_lambda: f64,
        init_center_x: f64,
        init_center_y: f64,
        mass_profile: MassProfile,
        mass_mean: f64,
        mass_stddev: f64,
        mass_min: f64,
        mass_max: f64,
        mass_alpha: f64,
    ) -> Self {
        Self::from_init_params(InitParams {
            n,
            seed,
            init_profile,
            init_radius,
            init_spread,
            init_v_amp,
            init_lambda,
            init_center_x,
            init_center_y,
            mass_profile,
            mass_mean,
            mass_stddev,
            mass_min,
            mass_max,
            mass_alpha,
            disk_scale_length: 0.0,
            disk_central_mass_frac: 0.1,
            disk_dispersion: 0.05,
            g: 1.0,
            merger_mass_ratio: 1.0,
            merger_separation: 0.0,
            merger_impact_parameter: -1.0,
            merger_v_rel: 0.0,
            merger_spin: MergerSpin::Prograde,
        })
    }

    pub fn from_init_params(params: InitParams) -> Self {
        if params.init_profile == InitProfile::GalaxyDisk {
            return sample_galaxy_disk_from_params(params);
        }
        if params.init_profile == InitProfile::Merger {
            return sample_merger(params);
        }

        let mut particles = Self::with_len(params.n);
        let mut rng = ChaCha8Rng::seed_from_u64(params.seed);

        for i in 0..params.n {
            let (x, y) = sample_initial_position(
                &mut rng,
                params.init_profile,
                params.init_radius,
                params.init_spread,
                params.init_lambda,
                params.init_center_x,
                params.init_center_y,
            );
            let (vx, vy) = sample_initial_velocity(
                &mut rng,
                params.init_profile,
                params.init_v_amp,
                params.init_lambda,
                x,
                y,
                params.init_center_x,
                params.init_center_y,
                params.init_radius,
                params.init_spread,
            );
            let mass = sample_mass(
                &mut rng,
                params.mass_profile,
                params.mass_mean,
                params.mass_stddev,
                params.mass_min,
                params.mass_max,
                params.mass_alpha,
            );

            particles.x[i] = x;
            particles.y[i] = y;
            particles.vx[i] = vx;
            particles.vy[i] = vy;
            particles.m[i] = mass;
        }

        particles
    }

    pub fn len(&self) -> usize {
        self.x.len()
    }

    pub fn is_empty(&self) -> bool {
        self.x.is_empty()
    }
}

pub fn particle_bounds(particles: &ParticleSoa) -> Result<(f64, f64, f64, f64), String> {
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

pub fn total_momentum(particles: &ParticleSoa) -> MomentumSnapshot {
    let mut px = 0.0;
    let mut py = 0.0;
    let mut angular_z = 0.0;

    for i in 0..particles.len() {
        let mass = particles.m[i];
        let p_x = mass * particles.vx[i];
        let p_y = mass * particles.vy[i];

        px += p_x;
        py += p_y;
        angular_z += mass * (particles.x[i] * particles.vy[i] - particles.y[i] * particles.vx[i]);
    }

    MomentumSnapshot {
        px,
        py,
        momentum_mag: (px * px + py * py).sqrt(),
        angular_momentum_z: angular_z,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn compute_energy_snapshot(
    particles: &ParticleSoa,
    epsilon: f64,
    g: f64,
    sample_ratio: f64,
    require_exact: bool,
    sample_seed: u64,
) -> Option<EnergySnapshot> {
    if particles.is_empty() {
        return Some(EnergySnapshot {
            kinetic: 0.0,
            potential: 0.0,
            total: 0.0,
            sampled_pairs: 0,
        });
    }

    let n = particles.len();
    let force_exact = require_exact || sample_ratio <= 0.0 || n <= 8192;
    if force_exact || (sample_ratio >= 1.0) {
        return Some(compute_exact_energy_snapshot(particles, epsilon, g));
    }

    let kinetic = total_kinetic_energy(particles);
    let potential =
        total_potential_energy_sampled(particles, epsilon, g, sample_ratio, sample_seed);

    Some(EnergySnapshot {
        kinetic,
        potential,
        total: kinetic + potential,
        sampled_pairs: sampled_pair_count(n, sample_ratio),
    })
}

pub fn compute_exact_energy_snapshot(
    particles: &ParticleSoa,
    epsilon: f64,
    g: f64,
) -> EnergySnapshot {
    let kinetic = total_kinetic_energy(particles);
    let potential = total_potential_energy_exact(particles, epsilon, g);
    let n = particles.len();

    EnergySnapshot {
        kinetic,
        potential,
        total: kinetic + potential,
        sampled_pairs: n.saturating_mul(n.saturating_sub(1)) / 2,
    }
}

#[cfg(test)]
pub(crate) fn compute_sampled_energy_snapshot(
    particles: &ParticleSoa,
    epsilon: f64,
    g: f64,
    sample_ratio: f64,
    sample_seed: u64,
) -> EnergySnapshot {
    let kinetic = total_kinetic_energy(particles);
    let potential =
        total_potential_energy_sampled(particles, epsilon, g, sample_ratio, sample_seed);
    let n = particles.len();

    EnergySnapshot {
        kinetic,
        potential,
        total: kinetic + potential,
        sampled_pairs: sampled_pair_count(n, sample_ratio),
    }
}

pub fn total_kinetic_energy(particles: &ParticleSoa) -> f64 {
    let mut total = 0.0;
    for i in 0..particles.len() {
        let speed2 = particles.vx[i] * particles.vx[i] + particles.vy[i] * particles.vy[i];
        total += 0.5 * particles.m[i] * speed2;
    }
    total
}

fn total_potential_energy_exact(particles: &ParticleSoa, epsilon: f64, g: f64) -> f64 {
    let mut total = 0.0;
    let n = particles.len();
    let eps2 = epsilon * epsilon;

    for i in 0..n {
        for j in (i + 1)..n {
            let dx = particles.x[i] - particles.x[j];
            let dy = particles.y[i] - particles.y[j];
            let dist = (dx * dx + dy * dy + eps2).sqrt();
            if dist <= 0.0 {
                continue;
            }
            total -= g * particles.m[i] * particles.m[j] / dist;
        }
    }

    total
}

fn total_potential_energy_sampled(
    particles: &ParticleSoa,
    epsilon: f64,
    g: f64,
    ratio: f64,
    seed: u64,
) -> f64 {
    let n = particles.len();
    let n_pairs = n.saturating_mul(n.saturating_sub(1)) / 2;
    if n <= 1 || n_pairs == 0 {
        return 0.0;
    }

    let sample_pairs = sampled_pair_count(n, ratio);
    if sample_pairs == 0 {
        return 0.0;
    }

    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let eps2 = epsilon * epsilon;
    let mut sample_sum = 0.0;

    for _ in 0..sample_pairs {
        let i = rng.random_range(0..n);
        let mut j = rng.random_range(0..n - 1);
        if j >= i {
            j += 1;
        }

        let dx = particles.x[i] - particles.x[j];
        let dy = particles.y[i] - particles.y[j];
        let dist = (dx * dx + dy * dy + eps2).sqrt();
        if dist <= 0.0 {
            continue;
        }
        sample_sum -= g * particles.m[i] * particles.m[j] / dist;
    }

    let scale = (n_pairs as f64) / (sample_pairs as f64);
    sample_sum * scale
}

fn sampled_pair_count(n: usize, ratio: f64) -> usize {
    let n_pairs = n.saturating_mul(n.saturating_sub(1)) / 2;
    if n_pairs == 0 {
        return 0;
    }
    let ratio = ratio.clamp(0.0, 1.0);
    let approx = (n_pairs as f64 * ratio).round() as usize;
    approx.clamp(1, n_pairs)
}

fn sample_mass(
    rng: &mut ChaCha8Rng,
    profile: MassProfile,
    mass_mean: f64,
    mass_stddev: f64,
    mass_min: f64,
    mass_max: f64,
    mass_alpha: f64,
) -> f64 {
    let mut low = mass_min.min(mass_max);
    let mut high = mass_min.max(mass_max);
    if !low.is_finite() || low <= 0.0 {
        low = 1e-6;
    }
    if !high.is_finite() || high < low {
        high = low * 2.0;
    }

    let mass = match profile {
        MassProfile::Uniform => rng.random_range(low..high),
        MassProfile::Gaussian => {
            let sigma = mass_stddev.abs().max(1e-12);
            let val = mass_mean + random_standard_normal(rng) * sigma;
            val.max(low)
        }
        MassProfile::Lognormal => {
            let sigma = mass_stddev.abs().max(1e-12);
            let mu = (mass_mean.max(1e-12)).ln();
            mu.exp() * (random_standard_normal(rng) * sigma).exp()
        }
        MassProfile::PowLaw => {
            let alpha = mass_alpha;
            if (alpha - 1.0).abs() < f64::EPSILON {
                let u = rng.random_range(0.0..1.0);
                low * (high / low).powf(u)
            } else {
                let a = 1.0 - alpha;
                let low_a = low.powf(a);
                let high_a = high.powf(a);
                let u = rng.random_range(0.0..1.0);
                (low_a + (high_a - low_a) * u).powf(1.0 / a)
            }
        }
    };

    mass.clamp(low, high)
}

fn sample_initial_position(
    rng: &mut ChaCha8Rng,
    profile: InitProfile,
    radius: f64,
    spread: f64,
    lambda: f64,
    center_x: f64,
    center_y: f64,
) -> (f64, f64) {
    let radius = radius.abs().max(1e-12);
    let spread = spread.abs().max(1e-12);

    match profile {
        InitProfile::Uniform => (
            rng.random_range(-radius..radius) + center_x,
            rng.random_range(-radius..radius) + center_y,
        ),
        InitProfile::Gaussian => (
            random_standard_normal(rng) * spread + center_x,
            random_standard_normal(rng) * spread + center_y,
        ),
        InitProfile::Plummer => {
            let t = rng.random_range(f64::MIN_POSITIVE..(1.0 - f64::EPSILON));
            let r = (radius * (t / (1.0 - t)).sqrt()).min(radius * 6.0);
            let angle = rng.random_range(0.0..(2.0 * PI));
            (center_x + r * angle.cos(), center_y + r * angle.sin())
        }
        InitProfile::Disk | InitProfile::RotatingDisk | InitProfile::KeplerianDisk => {
            let t = rng.random_range(f64::MIN_POSITIVE..(1.0 - f64::EPSILON));
            let r = (radius * (-lambda * t.ln()).abs()).min(radius * 6.0);
            let angle = rng.random_range(0.0..(2.0 * PI));
            (center_x + r * angle.cos(), center_y + r * angle.sin())
        }
        InitProfile::GalaxyDisk | InitProfile::Merger => {
            unreachable!("galaxy-disk/merger use dedicated samplers")
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn sample_initial_velocity(
    rng: &mut ChaCha8Rng,
    profile: InitProfile,
    init_v_amp: f64,
    init_lambda: f64,
    x: f64,
    y: f64,
    center_x: f64,
    center_y: f64,
    radius: f64,
    spread: f64,
) -> (f64, f64) {
    let amp = init_v_amp.abs();
    if amp == 0.0 {
        return (0.0, 0.0);
    }

    let jitter = amp * 0.05;
    let jitter_x = rng.random_range(-jitter..jitter);
    let jitter_y = rng.random_range(-jitter..jitter);

    match profile {
        InitProfile::Uniform => (
            rng.random_range(-amp..amp) + jitter_x,
            rng.random_range(-amp..amp) + jitter_y,
        ),
        InitProfile::Gaussian => (
            random_standard_normal(rng) * amp * spread.min(1.0) + jitter_x,
            random_standard_normal(rng) * amp * spread.min(1.0) + jitter_y,
        ),
        InitProfile::Plummer => {
            let dx = x - center_x;
            let dy = y - center_y;
            let r2 = dx * dx + dy * dy;
            let r = r2.sqrt().max(1e-12);
            let speed = amp * (1.0 / (1.0 + init_lambda * r / (radius.abs().max(1e-12)).max(1.0)));
            let angle = dy.atan2(dx);
            (
                -speed * angle.sin() + jitter_x,
                speed * angle.cos() + jitter_y,
            )
        }
        InitProfile::Disk => {
            let dx = x - center_x;
            let dy = y - center_y;
            let r = (dx * dx + dy * dy).sqrt().max(1e-12);
            let speed = amp / (1.0 + init_lambda * r / (radius.abs().max(1e-12)).max(1.0));
            let angle = dy.atan2(dx);
            (
                -speed * angle.sin() + jitter_x,
                speed * angle.cos() + jitter_y,
            )
        }
        InitProfile::RotatingDisk => {
            let dx = x - center_x;
            let dy = y - center_y;
            let r = (dx * dx + dy * dy).sqrt().max(1e-12);
            let denominator = r / radius.abs().max(1e-12).max(1e-12);
            let softening = (1.0 + denominator).sqrt();
            let speed = amp * (1.0 + init_lambda * (1.0 / softening));
            let angle = dy.atan2(dx);
            (
                -speed * angle.sin() + jitter_x,
                speed * angle.cos() + jitter_y,
            )
        }
        InitProfile::KeplerianDisk => {
            let dx = x - center_x;
            let dy = y - center_y;
            let r = (dx * dx + dy * dy).sqrt().max(1e-12);
            let scale = (radius.abs().max(1e-12)).max(1e-12);
            let speed = amp / (1.0 + (r / scale).sqrt());
            let angle = dy.atan2(dx);
            (
                -speed * angle.sin() + jitter_x,
                speed * angle.cos() + jitter_y,
            )
        }
        InitProfile::GalaxyDisk | InitProfile::Merger => {
            unreachable!("galaxy-disk/merger use dedicated samplers")
        }
    }
}

fn sample_galaxy_disk_from_params(params: InitParams) -> ParticleSoa {
    sample_galaxy_disk(
        params.n,
        params.seed,
        params.init_radius,
        params.init_center_x,
        params.init_center_y,
        params.mass_profile,
        params.mass_mean,
        params.mass_stddev,
        params.mass_min,
        params.mass_max,
        params.mass_alpha,
        params.disk_scale_length,
        params.disk_central_mass_frac,
        params.disk_dispersion,
        params.g,
    )
}

fn sample_merger(params: InitParams) -> ParticleSoa {
    let n = params.n;
    if n == 0 {
        return ParticleSoa::with_len(0);
    }
    if n == 1 {
        let mut single = params;
        single.init_profile = InitProfile::GalaxyDisk;
        return sample_galaxy_disk_from_params(single);
    }

    let q = params.merger_mass_ratio.clamp(f64::MIN_POSITIVE, 1.0);
    let mut n2 = ((n as f64) * q / (1.0 + q)).round() as usize;
    n2 = n2.clamp(1, n - 1);
    let n1 = n - n2;

    let radius1 = params.init_radius.abs().max(1e-12);
    let radius2 = radius1 * q.sqrt();
    let scale1 = params.disk_scale_length;
    let scale2 = if params.disk_scale_length.is_finite() && params.disk_scale_length > 0.0 {
        params.disk_scale_length * q.sqrt()
    } else {
        0.0
    };

    let seed2 = params.seed ^ 0xA5A5_A5A5_5A5A_5A5A;
    let mut primary_params = params;
    primary_params.n = n1;
    primary_params.init_profile = InitProfile::GalaxyDisk;
    primary_params.init_radius = radius1;
    primary_params.init_center_x = 0.0;
    primary_params.init_center_y = 0.0;
    primary_params.disk_scale_length = scale1;

    let mut secondary_params = primary_params;
    secondary_params.n = n2;
    secondary_params.seed = seed2;
    secondary_params.init_radius = radius2;
    secondary_params.disk_scale_length = scale2;

    let mut primary = sample_galaxy_disk_from_params(primary_params);
    let mut secondary = sample_galaxy_disk_from_params(secondary_params);
    if params.merger_spin == MergerSpin::Retrograde {
        flip_tangential_velocities(&mut secondary, 0.0, 0.0);
    }

    let mass1 = total_mass(&primary);
    let raw_mass2 = total_mass(&secondary);
    let target_mass2 = mass1 * q;
    let secondary_scale = if raw_mass2 > 0.0 {
        target_mass2 / raw_mass2
    } else {
        1.0
    };
    let secondary_velocity_scale = secondary_scale.sqrt();
    for i in 0..secondary.len() {
        secondary.m[i] *= secondary_scale;
        secondary.vx[i] *= secondary_velocity_scale;
        secondary.vy[i] *= secondary_velocity_scale;
    }
    let mass2 = total_mass(&secondary);
    let total = (mass1 + mass2).max(1e-12);

    let dx = if params.merger_separation.is_finite() && params.merger_separation > 0.0 {
        params.merger_separation
    } else {
        3.0 * radius1
    };
    let dy = if params.merger_impact_parameter.is_finite() && params.merger_impact_parameter >= 0.0
    {
        params.merger_impact_parameter
    } else {
        0.5 * radius1
    };
    let v_rel = if params.merger_v_rel.is_finite() && params.merger_v_rel > 0.0 {
        params.merger_v_rel
    } else {
        (2.0 * params.g.abs() * total / dx.max(1e-12)).sqrt()
    };

    let center_x = params.init_center_x;
    let center_y = params.init_center_y;
    let x1 = center_x - mass2 / total * dx;
    let y1 = center_y - mass2 / total * dy;
    let x2 = center_x + mass1 / total * dx;
    let y2 = center_y + mass1 / total * dy;
    let vx1 = mass2 / total * v_rel;
    let vx2 = -mass1 / total * v_rel;

    translate_and_boost(&mut primary, x1, y1, vx1, 0.0);
    translate_and_boost(&mut secondary, x2, y2, vx2, 0.0);
    concat_particles(primary, secondary)
}

fn total_mass(particles: &ParticleSoa) -> f64 {
    particles.m.iter().sum()
}

fn translate_and_boost(particles: &mut ParticleSoa, dx: f64, dy: f64, dvx: f64, dvy: f64) {
    for i in 0..particles.len() {
        particles.x[i] += dx;
        particles.y[i] += dy;
        particles.vx[i] += dvx;
        particles.vy[i] += dvy;
    }
}

fn flip_tangential_velocities(particles: &mut ParticleSoa, center_x: f64, center_y: f64) {
    for i in 0..particles.len() {
        let dx = particles.x[i] - center_x;
        let dy = particles.y[i] - center_y;
        let r = (dx * dx + dy * dy).sqrt();
        if r <= 1e-12 {
            continue;
        }
        let rx = dx / r;
        let ry = dy / r;
        let tx = -ry;
        let ty = rx;
        let radial = particles.vx[i] * rx + particles.vy[i] * ry;
        let tangential = particles.vx[i] * tx + particles.vy[i] * ty;
        particles.vx[i] = radial * rx - tangential * tx;
        particles.vy[i] = radial * ry - tangential * ty;
    }
}

fn concat_particles(a: ParticleSoa, b: ParticleSoa) -> ParticleSoa {
    let mut particles = ParticleSoa::with_capacity(a.len() + b.len());
    particles.x.extend(a.x);
    particles.y.extend(a.y);
    particles.vx.extend(a.vx);
    particles.vy.extend(a.vy);
    particles.m.extend(a.m);
    particles.x.extend(b.x);
    particles.y.extend(b.y);
    particles.vx.extend(b.vx);
    particles.vy.extend(b.vy);
    particles.m.extend(b.m);
    particles
}

#[allow(clippy::too_many_arguments)]
fn sample_galaxy_disk(
    n: usize,
    seed: u64,
    init_radius: f64,
    center_x: f64,
    center_y: f64,
    mass_profile: MassProfile,
    mass_mean: f64,
    mass_stddev: f64,
    mass_min: f64,
    mass_max: f64,
    mass_alpha: f64,
    disk_scale_length: f64,
    disk_central_mass_frac: f64,
    disk_dispersion: f64,
    g: f64,
) -> ParticleSoa {
    let mut particles = ParticleSoa::with_len(n);
    if n == 0 {
        return particles;
    }

    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let radius = init_radius.abs().max(1e-12);
    let scale_length = if disk_scale_length.is_finite() && disk_scale_length > 0.0 {
        disk_scale_length
    } else {
        radius / 4.0
    };
    let central_frac = if disk_central_mass_frac.is_finite() {
        disk_central_mass_frac.clamp(0.0, 0.95)
    } else {
        0.1
    };
    let dispersion_frac = if disk_dispersion.is_finite() {
        disk_dispersion.abs()
    } else {
        0.05
    };
    let gravity = if g.is_finite() { g.abs() } else { 1.0 };

    particles.x[0] = center_x;
    particles.y[0] = center_y;
    particles.vx[0] = 0.0;
    particles.vy[0] = 0.0;

    let mut disk_mass_sum = 0.0;
    for i in 1..n {
        let r = sample_truncated_exponential_radius(&mut rng, scale_length, radius);
        let angle = rng.random_range(0.0..(2.0 * PI));
        particles.x[i] = center_x + r * angle.cos();
        particles.y[i] = center_y + r * angle.sin();
        let mass = sample_mass(
            &mut rng,
            mass_profile,
            mass_mean,
            mass_stddev,
            mass_min,
            mass_max,
            mass_alpha,
        );
        particles.m[i] = mass;
        disk_mass_sum += mass;
    }

    particles.m[0] = if n == 1 || central_frac <= 0.0 {
        if n == 1 { mass_mean.max(1e-6) } else { 0.0 }
    } else {
        disk_mass_sum * central_frac / (1.0 - central_frac)
    };

    let mut by_radius: Vec<(usize, f64)> = (1..n)
        .map(|i| {
            let dx = particles.x[i] - center_x;
            let dy = particles.y[i] - center_y;
            (i, (dx * dx + dy * dy).sqrt())
        })
        .collect();
    by_radius.sort_by(|a, b| a.1.total_cmp(&b.1).then_with(|| a.0.cmp(&b.0)));

    // GalaxyDisk intentionally uses a simple two-pass circular-speed estimate:
    // after positions and masses are fixed, v_c(r)=sqrt(G*M(<r)/r) from the
    // actual discrete enclosed mass. The dispersion below is only a Gaussian
    // multiplier of local v_c, not a Toomre-Q stability analysis.
    let mut enclosed_mass = particles.m[0];
    for (idx, r) in by_radius {
        enclosed_mass += particles.m[idx];
        let r = r.max(1e-12);
        let vc = (gravity * enclosed_mass / r).sqrt();
        let dx = particles.x[idx] - center_x;
        let dy = particles.y[idx] - center_y;
        let inv_r = 1.0 / r;
        let radial_x = dx * inv_r;
        let radial_y = dy * inv_r;
        let tangent_x = -radial_y;
        let tangent_y = radial_x;
        let sigma = dispersion_frac * vc;
        let radial_jitter = random_standard_normal(&mut rng) * sigma;
        let tangential_jitter = random_standard_normal(&mut rng) * sigma;
        let tangential_speed = vc + tangential_jitter;
        particles.vx[idx] = tangent_x * tangential_speed + radial_x * radial_jitter;
        particles.vy[idx] = tangent_y * tangential_speed + radial_y * radial_jitter;
    }

    particles
}

fn sample_truncated_exponential_radius(
    rng: &mut ChaCha8Rng,
    scale_length: f64,
    truncation_radius: f64,
) -> f64 {
    let peak_radius = scale_length.min(truncation_radius);
    let peak_density = (peak_radius * (-peak_radius / scale_length).exp()).max(1e-12);
    loop {
        let r = rng.random_range(0.0..truncation_radius);
        let accept = (r * (-r / scale_length).exp() / peak_density).clamp(0.0, 1.0);
        if rng.random::<f64>() <= accept {
            return r;
        }
    }
}

fn random_standard_normal(rng: &mut ChaCha8Rng) -> f64 {
    let u1 = (rng.random::<f64>()).clamp(f64::MIN_POSITIVE, 1.0 - f64::EPSILON);
    let u2 = (rng.random::<f64>()).clamp(f64::MIN_POSITIVE, 1.0 - f64::EPSILON);
    let r = (-2.0 * u1.ln()).sqrt();
    let theta = 2.0 * PI * u2;
    r * theta.cos()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn galaxy(seed: u64) -> ParticleSoa {
        ParticleSoa::from_init_params(base_params(6000, seed, InitProfile::GalaxyDisk))
    }

    fn base_params(n: usize, seed: u64, init_profile: InitProfile) -> InitParams {
        InitParams {
            n,
            seed,
            init_profile,
            init_radius: 1.0,
            init_spread: 1.0,
            init_v_amp: 0.05,
            init_lambda: 1.0,
            init_center_x: 0.0,
            init_center_y: 0.0,
            mass_profile: MassProfile::Uniform,
            mass_mean: 1.0,
            mass_stddev: 0.25,
            mass_min: 0.9,
            mass_max: 1.1,
            mass_alpha: 2.0,
            disk_scale_length: 0.25,
            disk_central_mass_frac: 0.1,
            disk_dispersion: 0.05,
            g: 1.0,
            merger_mass_ratio: 1.0,
            merger_separation: 3.0,
            merger_impact_parameter: 0.5,
            merger_v_rel: 0.0,
            merger_spin: MergerSpin::Prograde,
        }
    }

    fn expected_vc_by_index(particles: &ParticleSoa, g: f64) -> Vec<f64> {
        let mut expected = vec![0.0; particles.len()];
        let mut radii: Vec<(usize, f64)> = (1..particles.len())
            .map(|i| {
                let r = (particles.x[i] * particles.x[i] + particles.y[i] * particles.y[i]).sqrt();
                (i, r)
            })
            .collect();
        radii.sort_by(|a, b| a.1.total_cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
        let mut enclosed = particles.m[0];
        for (idx, r) in radii {
            enclosed += particles.m[idx];
            expected[idx] = (g * enclosed / r.max(1e-12)).sqrt();
        }
        expected
    }

    #[test]
    fn galaxy_disk_rotation_curve_matches_enclosed_mass_two_seeds() {
        for seed in [1701_u64, 1902_u64] {
            let particles = galaxy(seed);
            let expected = expected_vc_by_index(&particles, 1.0);
            for (bin_idx, (lo, hi)) in [(0.05, 0.25), (0.25, 0.55), (0.55, 1.0)]
                .into_iter()
                .enumerate()
            {
                let mut count = 0_usize;
                let mut observed_sum = 0.0;
                let mut expected_sum = 0.0;
                for (i, expected_vc) in expected.iter().enumerate().skip(1) {
                    let x = particles.x[i];
                    let y = particles.y[i];
                    let r = (x * x + y * y).sqrt();
                    if r < lo || r >= hi {
                        continue;
                    }
                    let tx = -y / r;
                    let ty = x / r;
                    observed_sum += particles.vx[i] * tx + particles.vy[i] * ty;
                    expected_sum += expected_vc;
                    count += 1;
                }
                assert!(
                    count > 50,
                    "seed {seed} bin {bin_idx} has too few samples: {count}"
                );
                let observed_mean = observed_sum / count as f64;
                let expected_mean = expected_sum / count as f64;
                let rel = ((observed_mean - expected_mean) / expected_mean).abs();
                println!(
                    "galaxy_disk_rotation_curve seed={} bin={} r=[{:.2},{:.2}) count={} observed_mean_vt={:.9} expected_mean_vc={:.9} rel_err={:.9}",
                    seed, bin_idx, lo, hi, count, observed_mean, expected_mean, rel
                );
                assert!(
                    rel <= 0.10,
                    "seed {seed} bin {bin_idx} relative error {rel} > 10%"
                );
            }
        }
    }

    #[test]
    fn galaxy_disk_same_seed_is_bitwise_deterministic() {
        let a = galaxy(4242);
        let b = galaxy(4242);
        assert_eq!(a.x, b.x);
        assert_eq!(a.y, b.y);
        assert_eq!(a.vx, b.vx);
        assert_eq!(a.vy, b.vy);
        assert_eq!(a.m, b.m);
    }

    fn merger_parts(params: InitParams) -> (usize, ParticleSoa, ParticleSoa) {
        let q = params.merger_mass_ratio.clamp(f64::MIN_POSITIVE, 1.0);
        let mut n2 = ((params.n as f64) * q / (1.0 + q)).round() as usize;
        n2 = n2.clamp(1, params.n - 1);
        let n1 = params.n - n2;
        let mut p1 = params;
        p1.n = n1;
        p1.init_profile = InitProfile::GalaxyDisk;
        p1.init_center_x = 0.0;
        p1.init_center_y = 0.0;
        let mut p2 = p1;
        p2.n = n2;
        p2.seed = params.seed ^ 0xA5A5_A5A5_5A5A_5A5A;
        p2.init_radius = params.init_radius * q.sqrt();
        p2.disk_scale_length = params.disk_scale_length * q.sqrt();
        let primary = ParticleSoa::from_init_params(p1);
        let mut secondary = ParticleSoa::from_init_params(p2);
        if params.merger_spin == MergerSpin::Retrograde {
            flip_tangential_velocities(&mut secondary, 0.0, 0.0);
        }
        let scale = total_mass(&primary) * q / total_mass(&secondary);
        let velocity_scale = scale.sqrt();
        for i in 0..secondary.len() {
            secondary.m[i] *= scale;
            secondary.vx[i] *= velocity_scale;
            secondary.vy[i] *= velocity_scale;
        }
        (n1, primary, secondary)
    }

    #[test]
    fn merger_same_seed_is_bitwise_deterministic() {
        let params = base_params(4000, 9917, InitProfile::Merger);
        let a = ParticleSoa::from_init_params(params);
        let b = ParticleSoa::from_init_params(params);
        assert_eq!(a.x, b.x);
        assert_eq!(a.y, b.y);
        assert_eq!(a.vx, b.vx);
        assert_eq!(a.vy, b.vy);
        assert_eq!(a.m, b.m);
    }

    #[test]
    fn merger_bulk_momentum_is_com_zero_after_intrinsic_subtraction_two_seeds() {
        for seed in [1701_u64, 1902_u64] {
            let params = InitParams {
                n: 4096,
                seed,
                merger_mass_ratio: 0.65,
                merger_separation: 3.2,
                merger_impact_parameter: 0.4,
                ..base_params(4096, seed, InitProfile::Merger)
            };
            let merger = ParticleSoa::from_init_params(params);
            let (_n1, primary, secondary) = merger_parts(params);
            let total = total_momentum(&merger);
            let intrinsic1 = total_momentum(&primary);
            let intrinsic2 = total_momentum(&secondary);
            let residual_px = total.px - intrinsic1.px - intrinsic2.px;
            let residual_py = total.py - intrinsic1.py - intrinsic2.py;
            println!(
                "merger_bulk_momentum seed={} total=({:.17e},{:.17e}) intrinsic=({:.17e},{:.17e}) residual=({:.17e},{:.17e}) analytic_bulk=(0,0)",
                seed,
                total.px,
                total.py,
                intrinsic1.px + intrinsic2.px,
                intrinsic1.py + intrinsic2.py,
                residual_px,
                residual_py
            );
            let scale = total.px.abs().max(total.py.abs()).max(1.0);
            assert!(residual_px.abs() <= 1.0e-10 * scale);
            assert!(residual_py.abs() <= 1.0e-10 * scale);
        }
    }

    #[test]
    fn merger_sub_galaxies_reuse_galaxy_disk_rotation_bins() {
        let params = InitParams {
            n: 8000,
            seed: 2718,
            merger_mass_ratio: 0.5,
            ..base_params(8000, 2718, InitProfile::Merger)
        };
        let (n1, primary, secondary) = merger_parts(params);
        assert_eq!(n1 + secondary.len(), params.n);
        assert_rotation_bins("merger_primary", &primary, 1.0);
        assert_rotation_bins("merger_secondary", &secondary, 1.0);
    }

    fn assert_rotation_bins(label: &str, particles: &ParticleSoa, g: f64) {
        let expected = expected_vc_by_index(particles, g);
        for (bin_idx, (lo, hi)) in [(0.05, 0.25), (0.25, 0.55), (0.55, 1.0)]
            .into_iter()
            .enumerate()
        {
            let mut count = 0_usize;
            let mut observed_sum = 0.0;
            let mut expected_sum = 0.0;
            for (i, expected_vc) in expected.iter().enumerate().skip(1) {
                let x = particles.x[i];
                let y = particles.y[i];
                let r = (x * x + y * y).sqrt();
                if r < lo || r >= hi {
                    continue;
                }
                let tx = -y / r;
                let ty = x / r;
                observed_sum += particles.vx[i] * tx + particles.vy[i] * ty;
                expected_sum += expected_vc;
                count += 1;
            }
            assert!(count > 30, "{label} bin {bin_idx} samples: {count}");
            let observed_mean = observed_sum / count as f64;
            let expected_mean = expected_sum / count as f64;
            let rel = ((observed_mean - expected_mean) / expected_mean).abs();
            println!(
                "{label} rotation bin={} count={} observed_mean_vt={:.9} expected_mean_vc={:.9} rel_err={:.9}",
                bin_idx, count, observed_mean, expected_mean, rel
            );
            assert!(rel <= 0.10, "{label} bin {bin_idx} rel {rel} > 10%");
        }
    }
}
