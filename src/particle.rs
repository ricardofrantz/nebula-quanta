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
