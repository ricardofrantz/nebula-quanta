use clap::{Parser, ValueEnum};

#[derive(Parser, Debug)]
#[command(name = "nebula-quanta")]
pub struct Args {
    #[arg(long, default_value_t = 1000)]
    pub n: usize,
    #[arg(long, default_value_t = 100)]
    pub steps: usize,
    #[arg(long, default_value_t = 0.001)]
    pub dt: f64,
    /// Barnes–Hut opening angle. Use theta <= 0 to run the direct-force baseline.
    #[arg(long, default_value_t = 0.7)]
    pub theta: f64,
    /// Softening term used by both Barnes–Hut and direct-force solvers.
    #[arg(long, default_value_t = 0.01)]
    pub epsilon: f64,
    /// Gravitational constant multiplier.
    #[arg(long, default_value_t = 1.0)]
    pub g: f64,
    /// Initial-condition profile.
    #[arg(long, default_value = "uniform", value_enum)]
    pub init: InitProfile,
    /// Initial radial extent for uniform, plummer, and disk profiles.
    #[arg(long, default_value_t = 1.0)]
    pub init_radius: f64,
    /// Initial spread (stddev) for gaussian-style profiles.
    #[arg(long, default_value_t = 1.0)]
    pub init_spread: f64,
    /// Initial velocity amplitude used by all profiles.
    #[arg(long, default_value_t = 0.05)]
    pub init_v_amp: f64,
    /// Profile shape parameter for disk and plummer-like profiles.
    #[arg(long, default_value_t = 1.0)]
    pub init_lambda: f64,
    /// Initial x center.
    #[arg(long, default_value_t = 0.0)]
    pub init_center_x: f64,
    /// Initial y center.
    #[arg(long, default_value_t = 0.0)]
    pub init_center_y: f64,
    /// Mass profile.
    #[arg(long, default_value = "uniform", value_enum)]
    pub mass_profile: MassProfile,
    /// Mean mass for mass profile generators.
    #[arg(long, default_value_t = 1.0)]
    pub mass_mean: f64,
    /// Standard deviation for gaussian mass profiles.
    #[arg(long, default_value_t = 0.25)]
    pub mass_stddev: f64,
    /// Lower clamp for mass values.
    #[arg(long, default_value_t = 0.5)]
    pub mass_min: f64,
    /// Upper clamp for mass values.
    #[arg(long, default_value_t = 2.0)]
    pub mass_max: f64,
    /// Power-law exponent for power-law masses.
    #[arg(long, default_value_t = 2.0)]
    pub mass_alpha: f64,
    /// Seed for deterministic initialization.
    #[arg(long, default_value_t = 42)]
    pub seed: u64,
    /// Integrator.
    #[arg(long, default_value = "leapfrog", value_enum)]
    pub integrator: Integrator,
    /// Energy drift calculation mode: `auto` (default, only for N <= 8192), `on`, `off`.
    #[arg(long, value_enum, default_value = "auto")]
    pub energy_drift: EnergyDriftMode,
    /// Solver mode (`barnes_hut` or `direct`; aliases `barneshut`/`bh` are accepted).
    /// Default is Barnes–Hut for large-scale runs; `direct` retains the O(n²) baseline.
    #[arg(long, default_value = "barnes_hut")]
    pub mode: String,
    #[arg(long)]
    pub validate: bool,
    #[arg(long)]
    pub record: bool,
    #[arg(long, default_value = "frames")]
    pub frames_dir: String,
    #[arg(long, default_value_t = 1920)]
    pub width: u32,
    #[arg(long, default_value_t = 1080)]
    pub height: u32,
    #[arg(long, default_value_t = 60)]
    pub fps: u32,
    #[arg(long, default_value_t = 1)]
    pub every_steps: usize,
    #[arg(long, default_value_t = 1)]
    pub threads: usize,
    #[arg(long)]
    pub max_memory_mib: Option<usize>,
    /// Energy potential sample ratio for diagnostics (`0` uses exact for small runs when enabled).
    #[arg(long, default_value_t = 0.0)]
    pub energy_sample_ratio: f64,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnergyDriftMode {
    /// Auto-enable on small runs and skip on large runs.
    Auto,
    /// Always compute energy drift.
    On,
    /// Never compute energy drift.
    Off,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Integrator {
    /// Velocity Verlet style half-step method.
    Leapfrog,
    /// Alias for velocity Verlet.
    Verlet,
    /// Midpoint RK2 method.
    Rk2,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum InitProfile {
    Uniform,
    Gaussian,
    Plummer,
    Disk,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum MassProfile {
    Uniform,
    Lognormal,
    Gaussian,
    #[value(name = "pow-law")]
    PowLaw,
}

impl Args {
    pub fn should_measure_energy_drift(&self) -> bool {
        const AUTO_PARTICLE_LIMIT: usize = 8192;
        match self.energy_drift {
            EnergyDriftMode::Auto => self.n <= AUTO_PARTICLE_LIMIT,
            EnergyDriftMode::On => true,
            EnergyDriftMode::Off => false,
        }
    }

    pub fn should_measure_energy_snapshot(&self) -> bool {
        self.should_measure_energy_drift() || self.energy_sample_ratio > 0.0
    }

    pub fn should_sample_energy(&self) -> bool {
        self.energy_sample_ratio > 0.0 && self.n > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_profile_and_mass_options() {
        let args = Args::parse_from([
            "nq",
            "--n",
            "1000",
            "--steps",
            "10",
            "--dt",
            "0.001",
            "--epsilon",
            "0.01",
            "--init",
            "plummer",
            "--init-radius",
            "1.5",
            "--init-spread",
            "0.7",
            "--init-v-amp",
            "0.1",
            "--init-lambda",
            "2.0",
            "--init-center-x",
            "-0.3",
            "--init-center-y",
            "0.4",
            "--mass-profile",
            "pow-law",
            "--mass-mean",
            "1.2",
            "--mass-stddev",
            "0.3",
            "--mass-min",
            "0.2",
            "--mass-max",
            "4.0",
            "--mass-alpha",
            "1.5",
            "--integrator",
            "rk2",
            "--g",
            "0.98",
        ]);

        assert_eq!(args.init, InitProfile::Plummer);
        assert_eq!(args.mass_profile, MassProfile::PowLaw);
        assert_eq!(args.integrator, Integrator::Rk2);
        assert!((args.init_radius - 1.5).abs() < f64::EPSILON);
        assert!((args.mass_alpha - 1.5).abs() < f64::EPSILON);
        assert!((args.g - 0.98).abs() < f64::EPSILON);
    }

    #[test]
    fn parse_init_profile_aliases() {
        let with_alias = Args::parse_from([
            "nq",
            "--init",
            "disk",
            "--mass-profile",
            "lognormal",
        ]);
        assert_eq!(with_alias.init, InitProfile::Disk);
        assert_eq!(with_alias.mass_profile, MassProfile::Lognormal);
    }
}
