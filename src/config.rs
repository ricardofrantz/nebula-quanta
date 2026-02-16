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
    #[arg(long, default_value_t = 42)]
    pub seed: u64,
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
}

#[derive(ValueEnum, Clone, Debug)]
pub enum EnergyDriftMode {
    /// Auto-enable on small runs and skip on large runs.
    Auto,
    /// Always compute energy drift.
    On,
    /// Never compute energy drift.
    Off,
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
}
