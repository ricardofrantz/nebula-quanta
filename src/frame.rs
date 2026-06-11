use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::{config::Args, particle::ParticleSoa};

#[derive(Debug)]
pub struct FrameRecorder {
    frame_dir: PathBuf,
    width: u32,
    height: u32,
    every_steps: usize,
    next_index: usize,
    background: [u8; 3],
    buffer: Vec<u8>,
    point_radius: i32,
    trail_decay: f32,
    frame_count: usize,
    view_radius: f64,
}

impl FrameRecorder {
    pub fn new(args: &Args) -> Result<Self, String> {
        if args.width == 0 || args.height == 0 {
            return Err("width and height must be greater than zero".to_string());
        }
        if args.every_steps == 0 {
            return Err("every-steps must be greater than zero".to_string());
        }
        if !args.view_radius.is_finite() || args.view_radius < 0.0 {
            return Err("view-radius must be finite and non-negative".to_string());
        }

        let mut frame_dir = PathBuf::from(&args.frames_dir);
        if frame_dir.as_os_str().is_empty() {
            frame_dir.push("frames");
        }
        fs::create_dir_all(&frame_dir)
            .map_err(|err| format!("unable to create frame directory: {err}"))?;

        let width = args.width;
        let height = args.height;
        let buffer_len = frame_byte_size(width, height)?;
        let background = [255, 255, 255];
        let point_radius = 0;

        Ok(Self {
            frame_dir,
            width,
            height,
            every_steps: args.every_steps,
            next_index: 0,
            background,
            buffer: vec![0; buffer_len],
            point_radius,
            trail_decay: 0.0,
            frame_count: 0,
            view_radius: args.view_radius,
        })
    }

    pub fn should_record(&self, step: usize) -> bool {
        step.is_multiple_of(self.every_steps)
    }

    pub fn record_step(
        &mut self,
        step: usize,
        particles: &ParticleSoa,
        bounds: (f64, f64, f64, f64),
    ) -> Result<(), String> {
        if !self.should_record(step) {
            return Ok(());
        }

        let bounds = self.render_bounds(bounds);
        self.render_particles(particles, bounds)?;
        let path = self.frame_path();
        self.next_index = self.next_index.saturating_add(1);
        write_ppm(&path, self.width, self.height, &self.buffer)
            .map_err(|err| format!("unable to write frame {}: {err}", self.next_index - 1))?;
        self.frame_count = self.frame_count.saturating_add(1);
        Ok(())
    }

    pub fn frame_count(&self) -> usize {
        self.frame_count
    }

    pub fn render_command(&self, output: &str, fps: u32) -> String {
        format!(
            "ffmpeg -y -framerate {fps} -i {}/frame_%06d.ppm -s {}x{} -c:v libx264 -pix_fmt yuv420p {}",
            self.frame_dir.display(),
            self.width,
            self.height,
            output
        )
    }

    fn frame_path(&self) -> PathBuf {
        let mut path = self.frame_dir.clone();
        let index = self.next_index;
        path.push(format!("frame_{index:06}.ppm"));
        path
    }

    fn render_bounds(&self, autoscale_bounds: (f64, f64, f64, f64)) -> (f64, f64, f64, f64) {
        if self.view_radius > 0.0 {
            (
                -self.view_radius,
                self.view_radius,
                -self.view_radius,
                self.view_radius,
            )
        } else {
            autoscale_bounds
        }
    }

    fn decay_trails(&mut self) {
        for pixel in self.buffer.chunks_exact_mut(3) {
            pixel[0] = decayed_channel(pixel[0], self.background[0], self.trail_decay);
            pixel[1] = decayed_channel(pixel[1], self.background[1], self.trail_decay);
            pixel[2] = decayed_channel(pixel[2], self.background[2], self.trail_decay);
        }
    }

    fn render_particles(
        &mut self,
        particles: &ParticleSoa,
        bounds: (f64, f64, f64, f64),
    ) -> Result<(), String> {
        self.decay_trails();

        let (x_min, x_max, y_min, y_max) = bounds;
        let width = f64::from(self.width);
        let height = f64::from(self.height);
        let sx = (width - 1.0) / (x_max - x_min).max(1e-12);
        let sy = (height - 1.0) / (y_max - y_min).max(1e-12);
        let x_dim = usize::try_from(self.width).map_err(|_| "invalid frame width".to_string())?;
        let w = i64::from(self.width);
        let h = i64::from(self.height);
        let dot_kernel = dot_kernel(self.point_radius);

        for i in 0..particles.len() {
            let x = particles.x[i];
            let y = particles.y[i];
            if !x.is_finite() || !y.is_finite() {
                continue;
            }

            let px = ((x - x_min) * sx).round() as i64;
            let py = ((y_max - y) * sy).round() as i64;
            for &(dx, dy, falloff) in &dot_kernel {
                let xx = px + i64::from(dx);
                let yy = py + i64::from(dy);
                if xx < 0 || xx >= w || yy < 0 || yy >= h {
                    continue;
                }

                let x_usize = usize::try_from(xx).map_err(|_| "x index overflow".to_string())?;
                let y_usize = usize::try_from(yy).map_err(|_| "y index overflow".to_string())?;
                let row = y_usize
                    .checked_mul(x_dim)
                    .ok_or_else(|| "render row overflow".to_string())?;
                let idx = row
                    .checked_add(x_usize)
                    .and_then(|p| p.checked_mul(3))
                    .ok_or_else(|| "render pixel overflow".to_string())?;
                if idx + 2 >= self.buffer.len() {
                    continue;
                }
                stamp_ink(&mut self.buffer[idx..idx + 3], falloff);
            }
        }

        Ok(())
    }
}

fn dot_kernel(point_radius: i32) -> Vec<(i32, i32, f32)> {
    let radius = point_radius as f32 + 0.75;
    let mut kernel = Vec::new();
    for dy in -point_radius..=point_radius {
        for dx in -point_radius..=point_radius {
            let distance = ((dx * dx + dy * dy) as f32).sqrt();
            let falloff = (1.0 - distance / radius).clamp(0.0, 1.0);
            if falloff > 0.0 {
                kernel.push((dx, dy, falloff));
            }
        }
    }
    kernel
}

fn decayed_channel(value: u8, background: u8, decay: f32) -> u8 {
    let faded = f32::from(value) * decay;
    faded.max(f32::from(background)).round() as u8
}

fn stamp_ink(pixel: &mut [u8], falloff: f32) {
    for channel in pixel.iter_mut() {
        let ink = (255.0 * falloff).round() as u8;
        *channel = channel.saturating_sub(ink);
    }
}

fn write_ppm(path: &Path, width: u32, height: u32, buffer: &[u8]) -> io::Result<()> {
    let expected = frame_byte_size(width, height).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("frame size validation failed: {err}"),
        )
    })?;
    if buffer.len() != expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "frame buffer size mismatch",
        ));
    }

    let mut out = File::create(path)?;
    let header = format!("P6\n{width} {height}\n255\n");
    out.write_all(header.as_bytes())?;
    out.write_all(buffer)?;
    Ok(())
}

fn frame_byte_size(width: u32, height: u32) -> Result<usize, String> {
    let pixels = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| "frame dimensions overflow".to_string())?;
    pixels
        .checked_mul(3)
        .ok_or_else(|| "frame byte size overflow".to_string())
}

#[cfg(test)]
mod tests {
    use super::FrameRecorder;
    use crate::config::Args;
    use clap::Parser;

    fn recorder_args(view_radius: &str, frames_dir: &str) -> Args {
        let view_arg = format!("--view-radius={view_radius}");
        Args::parse_from([
            "nq",
            "--record",
            "--frames-dir",
            frames_dir,
            "--width",
            "9",
            "--height",
            "9",
            &view_arg,
        ])
    }

    #[test]
    fn negative_view_radius_is_rejected() {
        let args = recorder_args("-1.0", ".sc/test-frames-neg");
        let err = FrameRecorder::new(&args).unwrap_err();
        assert!(err.contains("view-radius"), "unexpected error: {err}");
    }

    #[test]
    fn fixed_view_radius_overrides_autoscale_bounds() {
        let args = recorder_args("2.0", ".sc/test-frames-fixed");
        let recorder = FrameRecorder::new(&args).unwrap();
        // Autoscale bounds are ignored when a fixed view radius is set.
        let bounds = recorder.render_bounds((-100.0, 100.0, -50.0, 50.0));
        assert_eq!(bounds, (-2.0, 2.0, -2.0, 2.0));

        let args_auto = recorder_args("0.0", ".sc/test-frames-auto");
        let recorder_auto = FrameRecorder::new(&args_auto).unwrap();
        let auto = recorder_auto.render_bounds((-100.0, 100.0, -50.0, 50.0));
        assert_eq!(auto, (-100.0, 100.0, -50.0, 50.0));
    }
}
