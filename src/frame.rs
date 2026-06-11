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
    frame_count: usize,
}

impl FrameRecorder {
    pub fn new(args: &Args) -> Result<Self, String> {
        if args.width == 0 || args.height == 0 {
            return Err("width and height must be greater than zero".to_string());
        }
        if args.every_steps == 0 {
            return Err("every-steps must be greater than zero".to_string());
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
        let background = [6, 10, 16];
        let point_radius = 1;

        Ok(Self {
            frame_dir,
            width,
            height,
            every_steps: args.every_steps,
            next_index: 0,
            background,
            buffer: vec![0; buffer_len],
            point_radius,
            frame_count: 0,
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

    fn render_particles(
        &mut self,
        particles: &ParticleSoa,
        bounds: (f64, f64, f64, f64),
    ) -> Result<(), String> {
        for pixel in self.buffer.chunks_exact_mut(3) {
            pixel[0] = self.background[0];
            pixel[1] = self.background[1];
            pixel[2] = self.background[2];
        }

        let (x_min, x_max, y_min, y_max) = bounds;
        let width = f64::from(self.width);
        let height = f64::from(self.height);
        let sx = (width - 1.0) / (x_max - x_min).max(1e-12);
        let sy = (height - 1.0) / (y_max - y_min).max(1e-12);

        for i in 0..particles.len() {
            let x = particles.x[i];
            let y = particles.y[i];
            if !x.is_finite() || !y.is_finite() {
                continue;
            }

            let px = ((x - x_min) * sx).round() as i64;
            let py = ((y_max - y) * sy).round() as i64;
            let w = i64::from(self.width);
            let h = i64::from(self.height);

            for dy in -self.point_radius..=self.point_radius {
                for dx in -self.point_radius..=self.point_radius {
                    let xx = px + i64::from(dx);
                    let yy = py + i64::from(dy);
                    if xx < 0 || xx >= w || yy < 0 || yy >= h {
                        continue;
                    }

                    let x_usize =
                        usize::try_from(xx).map_err(|_| "x index overflow".to_string())?;
                    let y_usize =
                        usize::try_from(yy).map_err(|_| "y index overflow".to_string())?;
                    let x_dim = usize::try_from(self.width)
                        .map_err(|_| "invalid frame width".to_string())?;
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
                    self.buffer[idx] = 255;
                    self.buffer[idx + 1] = 255;
                    self.buffer[idx + 2] = 255;
                }
            }
        }

        Ok(())
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
    let w = usize::try_from(width).map_err(|_| "invalid frame width".to_string())?;
    let h = usize::try_from(height).map_err(|_| "invalid frame height".to_string())?;
    w.checked_mul(h)
        .and_then(|p| p.checked_mul(3))
        .ok_or_else(|| "frame dimensions too large".to_string())
}
