use std::time::Instant;

use crate::config::Args;
use crate::frame::FrameRecorder;

pub struct ProgressReporter {
    mode: &'static str,
    total_steps: usize,
    every_steps: usize,
    start: Instant,
    last_step: Option<usize>,
    last_instant: Option<Instant>,
}

impl ProgressReporter {
    pub fn from_args(args: &Args, mode: &'static str) -> Option<Self> {
        if args.progress_every == 0 {
            return None;
        }

        Some(Self {
            mode,
            total_steps: args.steps,
            every_steps: args.progress_every,
            start: Instant::now(),
            last_step: None,
            last_instant: None,
        })
    }

    pub fn maybe_report(&mut self, step: usize, recorder: Option<&FrameRecorder>) {
        let step = step.min(self.total_steps);
        let should_report = step == 0
            || step == self.total_steps
            || self
                .last_step
                .is_none_or(|last| step.saturating_sub(last) >= self.every_steps);

        if !should_report {
            return;
        }

        let now = Instant::now();
        let elapsed_s = now.duration_since(self.start).as_secs_f64();

        // Recent throughput over the interval since the last report, so the ETA
        // tracks the current pace instead of the cumulative average. The first
        // report has no prior interval, so it falls back to the running mean.
        let steps_per_sec = match (self.last_step, self.last_instant) {
            (Some(last), Some(last_instant)) if step > last => {
                let dt = now.duration_since(last_instant).as_secs_f64();
                if dt > 0.0 {
                    (step - last) as f64 / dt
                } else {
                    0.0
                }
            }
            _ => {
                if step == 0 || elapsed_s <= 0.0 {
                    0.0
                } else {
                    step as f64 / elapsed_s
                }
            }
        };

        self.last_step = Some(step);
        self.last_instant = Some(now);

        let pct = if self.total_steps == 0 {
            100.0
        } else {
            100.0 * step as f64 / self.total_steps as f64
        };
        let eta_s = if steps_per_sec > 0.0 && step < self.total_steps {
            (self.total_steps - step) as f64 / steps_per_sec
        } else {
            0.0
        };
        let frames = recorder
            .map(|recorder| recorder.frame_count().to_string())
            .unwrap_or_else(|| "na".to_string());

        eprintln!(
            "progress mode={} step={}/{} pct={:.1} frames={} elapsed_s={:.1} steps_per_sec={:.3} eta_s={:.1}",
            self.mode, step, self.total_steps, pct, frames, elapsed_s, steps_per_sec, eta_s
        );
    }
}

#[cfg(test)]
mod tests {
    use super::ProgressReporter;
    use crate::config::Args;
    use clap::Parser;

    #[test]
    fn disabled_when_progress_every_is_zero() {
        let args = Args::parse_from(["nq"]);

        assert!(ProgressReporter::from_args(&args, "barnes_hut").is_none());
    }

    #[test]
    fn enabled_when_progress_every_is_positive() {
        let args = Args::parse_from(["nq", "--progress-every", "25", "--steps", "100"]);

        let reporter = ProgressReporter::from_args(&args, "barnes_hut").unwrap();

        assert_eq!(reporter.total_steps, 100);
        assert_eq!(reporter.every_steps, 25);
    }
}
