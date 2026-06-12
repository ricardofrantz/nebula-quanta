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
    /// Spatial opening-angle control policy for Barnes–Hut.
    #[arg(long, default_value = "fixed", value_enum)]
    pub theta_policy: ThetaPolicy,
    /// Density scaling factor for `theta` local-density adaptation. Larger values reduce adaptive effect.
    #[arg(long, default_value_t = 128.0)]
    pub theta_density_scale: f64,
    /// Softening-length policy for near-field interaction regularization.
    #[arg(long, default_value = "fixed", value_enum)]
    pub softening_policy: SofteningPolicy,
    /// Density scaling factor for local-density softening adaptation. Larger values reduce adaptive effect.
    #[arg(long, default_value_t = 128.0)]
    pub softening_density_scale: f64,
    /// Softening term used by both Barnes–Hut and direct-force solvers.
    #[arg(long, default_value_t = 0.01)]
    pub epsilon: f64,
    /// Gravitational constant multiplier.
    #[arg(long, default_value_t = 1.0)]
    pub g: f64,
    /// Initial-condition profile.
    #[arg(long, default_value = "uniform", value_enum)]
    pub init: InitProfile,
    /// Secondary/primary mass ratio for merger initial conditions (clamped to (0, 1]).
    #[arg(long, default_value_t = 1.0)]
    pub merger_mass_ratio: f64,
    /// Initial merger separation along x; defaults to 3 * init-radius when <= 0.
    #[arg(long, default_value_t = 0.0)]
    pub merger_separation: f64,
    /// Initial merger perpendicular y offset; defaults to 0.5 * init-radius when < 0.
    #[arg(long, default_value_t = -1.0)]
    pub merger_impact_parameter: f64,
    /// Initial merger closing speed along x; defaults to near-parabolic when <= 0.
    #[arg(long, default_value_t = 0.0)]
    pub merger_v_rel: f64,
    /// Secondary disk spin sense for merger initial conditions.
    #[arg(long, default_value = "prograde", value_enum)]
    pub merger_spin: MergerSpin,
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
    /// Exponential disk scale length for galaxy-disk; defaults to init-radius / 4 when <= 0.
    #[arg(long, default_value_t = 0.0)]
    pub disk_scale_length: f64,
    /// Fraction of total galaxy-disk mass assigned to the central point particle (clamped to [0, 0.95]).
    #[arg(long, default_value_t = 0.1)]
    pub disk_central_mass_frac: f64,
    /// Gaussian radial/tangential dispersion multiplier for galaxy-disk, as a fraction of local circular speed.
    #[arg(long, default_value_t = 0.05)]
    pub disk_dispersion: f64,
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
    /// Problem dimension (2D supported; 3D is a planned expansion).
    #[arg(long, default_value = "2", value_enum)]
    pub dim: Dimensionality,
    /// Solver mode (`barnes_hut` or `direct`; aliases `barneshut`/`bh` are accepted).
    /// Default is Barnes–Hut for large-scale runs; `direct` retains the O(n²) baseline.
    #[arg(long, default_value = "barnes_hut")]
    pub mode: String,
    #[arg(long)]
    pub validate: bool,
    #[arg(long)]
    pub record: bool,
    #[arg(long, conflicts_with = "output")]
    pub frames_dir: Option<String>,
    #[arg(long, conflicts_with = "frames_dir")]
    pub output: Option<String>,
    #[arg(long, default_value_t = 1920)]
    pub width: u32,
    #[arg(long, default_value_t = 1080)]
    pub height: u32,
    #[arg(long, default_value_t = 60)]
    pub fps: u32,
    #[arg(long, default_value_t = 1)]
    pub every_steps: usize,
    /// Comma-separated frame coloring modes: golden,speed,accel,density,mass.
    #[arg(long, default_value = "golden")]
    pub color_by: String,
    /// Colormap for non-golden color modes. `mode` preserves the built-in per-quantity palettes.
    #[arg(long, default_value = "mode", value_enum)]
    pub colormap: ColorMap,
    /// Scalar-to-color transfer function for non-golden color modes.
    #[arg(long, default_value = "asinh", value_enum)]
    pub color_scale: ColorScale,
    /// Lower color normalization bound, or `auto`.
    #[arg(long, default_value = "auto")]
    pub color_min: String,
    /// Upper color normalization bound, or `auto`.
    #[arg(long, default_value = "auto")]
    pub color_max: String,
    /// First-recorded-frame automatic normalization statistic.
    #[arg(long, default_value = "first-p99", value_enum)]
    pub color_auto: ColorAuto,
    /// Multiplicative headroom applied to automatic upper bounds.
    #[arg(long, default_value_t = 1.5)]
    pub color_headroom: f64,
    /// Fixed square view half-width centered on the origin; `0` auto-fits the particle extent per frame.
    #[arg(long, default_value_t = 0.0)]
    pub view_radius: f64,
    /// Barnes-Hut force worker threads; `1` is strictly serial, values >1 use a persistent rayon pool.
    #[arg(long, default_value_t = 1)]
    pub threads: usize,
    #[arg(long)]
    pub max_memory_mib: Option<usize>,
    /// Energy potential sample ratio for diagnostics (`0` uses exact for small runs when enabled).
    #[arg(long, default_value_t = 0.0)]
    pub energy_sample_ratio: f64,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorMode {
    Golden,
    Speed,
    Accel,
    Density,
    Mass,
}

impl ColorMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Golden => "golden",
            Self::Speed => "speed",
            Self::Accel => "accel",
            Self::Density => "density",
            Self::Mass => "mass",
        }
    }
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorMap {
    /// Preserve the built-in palette associated with each `--color-by` mode.
    Mode,
    Gold,
    Inferno,
    Viridis,
    Magma,
    Plasma,
    Turbo,
    #[value(name = "blue-red")]
    BlueRed,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorScale {
    Linear,
    Log,
    Asinh,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorAuto {
    #[value(name = "first-p99", alias = "p99")]
    FirstP99,
    #[value(name = "first-p95", alias = "p95")]
    FirstP95,
    #[value(name = "first-minmax", alias = "minmax")]
    FirstMinmax,
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
pub enum Dimensionality {
    /// 2D execution (x-y plane).
    #[value(name = "2")]
    Two,
    /// 3D execution is reserved (not yet implemented).
    #[value(name = "3")]
    Three,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThetaPolicy {
    /// Use a fixed global theta value for all steps.
    Fixed,
    /// Reduce theta when particle density rises (bounds-derived heuristic).
    #[value(name = "local-density")]
    LocalDensity,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SofteningPolicy {
    /// Use a fixed global softening length for all steps.
    Fixed,
    /// Increase softening with density to reduce singularity sensitivity in clustered states.
    #[value(name = "local-density")]
    LocalDensity,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum InitProfile {
    Uniform,
    Gaussian,
    Plummer,
    Disk,
    #[value(name = "rotating-disk")]
    RotatingDisk,
    #[value(name = "keplerian-disk")]
    KeplerianDisk,
    #[value(name = "galaxy-disk")]
    GalaxyDisk,
    Merger,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum MergerSpin {
    Prograde,
    Retrograde,
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
    /// Resolve a step-specific Barnes–Hut opening angle from policy and current bounds.
    /// Local-density policy adapts theta downward as particle density rises.
    pub fn theta_for_step(
        &self,
        _step: usize,
        particle_count: usize,
        bounds: Option<(f64, f64, f64, f64)>,
    ) -> f64 {
        if self.theta <= 0.0 {
            return self.theta;
        }

        match self.theta_policy {
            ThetaPolicy::Fixed => self.theta,
            ThetaPolicy::LocalDensity => {
                let Some((x_min, x_max, y_min, y_max)) = bounds else {
                    return self.theta;
                };

                let span_x = (x_max - x_min).abs();
                let span_y = (y_max - y_min).abs();
                let area = span_x * span_y;

                if !area.is_finite() || area <= 0.0 {
                    return self.theta;
                }

                let density = (particle_count as f64) / area;
                if !density.is_finite() || density <= 0.0 {
                    return self.theta;
                }

                let scale = self.theta_density_scale.max(f64::EPSILON);
                let factor = (density / scale).ln_1p();
                if !factor.is_finite() {
                    return self.theta;
                }

                let min_theta = (self.theta * 0.05).max(1.0e-4);
                (self.theta / (1.0 + factor)).clamp(min_theta, self.theta)
            }
        }
    }

    /// Resolve a step-specific softening length from policy and current bounds.
    /// Local-density policy increases softening in dense systems.
    pub fn epsilon_for_step(
        &self,
        _step: usize,
        particle_count: usize,
        bounds: Option<(f64, f64, f64, f64)>,
    ) -> f64 {
        if !self.epsilon.is_finite() || self.epsilon <= 0.0 {
            return self.epsilon;
        }

        match self.softening_policy {
            SofteningPolicy::Fixed => self.epsilon,
            SofteningPolicy::LocalDensity => {
                let Some((x_min, x_max, y_min, y_max)) = bounds else {
                    return self.epsilon;
                };

                let span_x = (x_max - x_min).abs();
                let span_y = (y_max - y_min).abs();
                let area = span_x * span_y;
                if !area.is_finite() || area <= 0.0 {
                    return self.epsilon;
                }

                let density = (particle_count as f64) / area;
                if !density.is_finite() || density <= 0.0 {
                    return self.epsilon;
                }

                let scale = self.softening_density_scale.max(f64::EPSILON);
                let factor = (density / scale).ln_1p();
                if !factor.is_finite() {
                    return self.epsilon;
                }

                let factor = factor.clamp(0.0, 8.0);
                self.epsilon * (1.0 + factor)
            }
        }
    }

    pub fn color_modes(&self) -> Result<Vec<ColorMode>, String> {
        let mut modes = Vec::new();
        for raw in self.color_by.split(',') {
            let token = raw.trim();
            if token.is_empty() {
                return Err("--color-by entries must not be empty".to_string());
            }
            let mode = match token {
                "golden" => ColorMode::Golden,
                "speed" => ColorMode::Speed,
                "accel" => ColorMode::Accel,
                "density" => ColorMode::Density,
                "mass" => ColorMode::Mass,
                other => {
                    return Err(format!(
                        "unknown --color-by mode '{other}'; expected golden,speed,accel,density,mass"
                    ));
                }
            };
            if modes.contains(&mode) {
                return Err(format!("duplicate --color-by mode '{token}'"));
            }
            modes.push(mode);
        }
        if modes.is_empty() {
            return Err("--color-by must include at least one mode".to_string());
        }
        Ok(modes)
    }

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
            "--init-center-x=-0.3",
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
    fn parse_galaxy_disk_options() {
        let args = Args::parse_from([
            "nq",
            "--init",
            "galaxy-disk",
            "--disk-scale-length",
            "0.4",
            "--disk-central-mass-frac",
            "0.2",
            "--disk-dispersion",
            "0.03",
        ]);

        assert_eq!(args.init, InitProfile::GalaxyDisk);
        assert!((args.disk_scale_length - 0.4).abs() < f64::EPSILON);
        assert!((args.disk_central_mass_frac - 0.2).abs() < f64::EPSILON);
        assert!((args.disk_dispersion - 0.03).abs() < f64::EPSILON);
    }

    #[test]
    fn parse_merger_options() {
        let args = Args::parse_from([
            "nq",
            "--init",
            "merger",
            "--merger-mass-ratio",
            "0.7",
            "--merger-separation",
            "3.5",
            "--merger-impact-parameter",
            "0.4",
            "--merger-v-rel",
            "1.2",
            "--merger-spin",
            "retrograde",
        ]);

        assert_eq!(args.init, InitProfile::Merger);
        assert_eq!(args.merger_spin, MergerSpin::Retrograde);
        assert!((args.merger_mass_ratio - 0.7).abs() < f64::EPSILON);
        assert!((args.merger_separation - 3.5).abs() < f64::EPSILON);
        assert!((args.merger_impact_parameter - 0.4).abs() < f64::EPSILON);
        assert!((args.merger_v_rel - 1.2).abs() < f64::EPSILON);
    }

    #[test]
    fn parse_init_profile_aliases() {
        let with_alias = Args::parse_from(["nq", "--init", "disk", "--mass-profile", "lognormal"]);
        assert_eq!(with_alias.init, InitProfile::Disk);
        assert_eq!(with_alias.mass_profile, MassProfile::Lognormal);
    }

    #[test]
    fn parse_dim_and_adaptive_theta_flags() {
        let args = Args::parse_from([
            "nq",
            "--dim",
            "3",
            "--theta",
            "0.7",
            "--theta-policy",
            "local-density",
            "--theta-density-scale",
            "64.0",
        ]);

        assert_eq!(args.dim, Dimensionality::Three);
        assert_eq!(args.theta_policy, ThetaPolicy::LocalDensity);
        assert!((args.theta_density_scale - 64.0).abs() < f64::EPSILON);
        let adaptive_theta = args.theta_for_step(0, 1000, Some((-1.0, 1.0, -2.0, 2.0)));
        assert!(adaptive_theta < 0.7);
        assert!(adaptive_theta >= (0.7_f64 * 0.05).max(1.0e-4));
        assert_eq!(args.theta_for_step(0, 1000, None), 0.7);

        let fixed_args = Args::parse_from(["nq", "--theta-policy", "fixed", "--theta", "0.6"]);
        assert_eq!(
            fixed_args.theta_for_step(0, 1000, Some((-1.0, 1.0, -2.0, 2.0))),
            0.6
        );
    }

    #[test]
    fn parse_softening_policy_and_density_scale() {
        let adaptive_args = Args::parse_from([
            "nq",
            "--epsilon",
            "0.01",
            "--softening-policy",
            "local-density",
            "--softening-density-scale",
            "64.0",
        ]);

        assert_eq!(
            adaptive_args.softening_policy,
            SofteningPolicy::LocalDensity
        );
        assert!((adaptive_args.softening_density_scale - 64.0).abs() < f64::EPSILON);
        let adaptive_epsilon =
            adaptive_args.epsilon_for_step(0, 1000, Some((-1.0, 1.0, -1.0, 1.0)));
        assert!(adaptive_epsilon > adaptive_args.epsilon);

        let fixed_args =
            Args::parse_from(["nq", "--epsilon", "0.02", "--softening-policy", "fixed"]);
        assert_eq!(
            fixed_args.epsilon_for_step(0, 1000, Some((-1.0, 1.0, -1.0, 1.0)),),
            0.02
        );
    }

    #[test]
    fn output_and_frames_dir_conflict() {
        let err = Args::try_parse_from([
            "nq",
            "--record",
            "--output",
            "out.mp4",
            "--frames-dir",
            "frames",
        ])
        .unwrap_err();
        assert!(err.to_string().contains("cannot be used"));
    }

    #[test]
    fn accepts_color_mapping_flags() {
        let parsed = Args::try_parse_from([
            "nq",
            "--record",
            "--color-by",
            "speed",
            "--colormap",
            "viridis",
            "--color-scale",
            "linear",
            "--color-min",
            "0.2",
            "--color-max",
            "2.0",
            "--color-auto",
            "first-p95",
            "--color-headroom",
            "2.0",
        ]);

        assert!(parsed.is_ok(), "unexpected parse error: {parsed:?}");
    }

    #[test]
    fn parse_additional_profile_variants() {
        let disk_args = Args::parse_from(["nq", "--init", "rotating-disk", "--n", "32"]);
        let keplerian_args = Args::parse_from(["nq", "--init", "keplerian-disk", "--n", "32"]);

        assert_eq!(disk_args.init, InitProfile::RotatingDisk);
        assert_eq!(keplerian_args.init, InitProfile::KeplerianDisk);
    }
}
