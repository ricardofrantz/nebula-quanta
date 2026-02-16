use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

#[derive(Clone, Debug)]
pub struct ParticleSoa {
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    pub vx: Vec<f64>,
    pub vy: Vec<f64>,
    pub m: Vec<f64>,
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

    pub fn random(n: usize, seed: u64) -> Self {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let mut particles = Self::with_len(n);
        for i in 0..n {
            particles.x[i] = rng.random_range(-1.0..1.0);
            particles.y[i] = rng.random_range(-1.0..1.0);
            particles.vx[i] = rng.random_range(-0.05..0.05);
            particles.vy[i] = rng.random_range(-0.05..0.05);
            particles.m[i] = rng.random_range(0.5..2.0);
        }
        particles
    }

    pub fn len(&self) -> usize {
        self.x.len()
    }
}

pub fn total_mechanical_energy(particles: &ParticleSoa, epsilon: f64) -> f64 {
    total_kinetic_energy(particles) + total_potential_energy(particles, epsilon)
}

fn total_kinetic_energy(particles: &ParticleSoa) -> f64 {
    let mut total = 0.0;
    for i in 0..particles.len() {
        let speed2 = particles.vx[i] * particles.vx[i] + particles.vy[i] * particles.vy[i];
        total += 0.5 * particles.m[i] * speed2;
    }
    total
}

fn total_potential_energy(particles: &ParticleSoa, epsilon: f64) -> f64 {
    let mut total = 0.0;
    let n = particles.len();
    let eps2 = epsilon * epsilon;

    for i in 0..n {
        for j in (i + 1)..n {
            let dx = particles.x[i] - particles.x[j];
            let dy = particles.y[i] - particles.y[j];
            let dist = (dx * dx + dy * dy + eps2).sqrt();
            if dist == 0.0 {
                continue;
            }
            total -= particles.m[i] * particles.m[j] / dist;
        }
    }

    total
}
