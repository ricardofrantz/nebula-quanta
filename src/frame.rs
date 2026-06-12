use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};

use crate::{config::Args, particle::ParticleSoa};

#[derive(Debug)]
pub struct FrameRecorder {
    sink: FrameSink,
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

        let width = args.width;
        let height = args.height;
        let buffer_len = frame_byte_size(width, height)?;
        let sink = if let Some(output) = &args.output {
            FrameSink::Ffmpeg(FfmpegSink::spawn(output, width, height, args.fps)?)
        } else {
            let frame_dir = PathBuf::from(args.frames_dir.as_deref().unwrap_or("frames"));
            fs::create_dir_all(&frame_dir)
                .map_err(|err| format!("unable to create frame directory: {err}"))?;
            FrameSink::Ppm { frame_dir }
        };
        let background = [0, 0, 0];
        let point_radius = 0;

        Ok(Self {
            sink,
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
        let frame_index = self.next_index;
        self.next_index = self.next_index.saturating_add(1);
        match &mut self.sink {
            FrameSink::Ppm { frame_dir } => {
                let path = frame_path(frame_dir, frame_index);
                write_ppm(&path, self.width, self.height, &self.buffer)
                    .map_err(|err| format!("unable to write frame {frame_index}: {err}"))?;
            }
            FrameSink::Ffmpeg(sink) => sink.write_frame(&self.buffer)?,
        }
        self.frame_count = self.frame_count.saturating_add(1);
        Ok(())
    }

    pub fn frame_count(&self) -> usize {
        self.frame_count
    }

    pub fn finish(&mut self) -> Result<(), String> {
        if let FrameSink::Ffmpeg(sink) = &mut self.sink {
            sink.finish()?;
        }
        Ok(())
    }

    pub fn render_command(&self, output: &str, fps: u32) -> Option<String> {
        match &self.sink {
            FrameSink::Ppm { frame_dir } => Some(format!(
                "ffmpeg -y -framerate {fps} -i {}/frame_%06d.ppm -s {}x{} -c:v libx264 -crf 0 -pix_fmt yuv444p {}",
                frame_dir.display(),
                self.width,
                self.height,
                output
            )),
            FrameSink::Ffmpeg(_) => None,
        }
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
    // Golden star color, accumulated additively so overlapping bodies brighten.
    const GOLD: [u8; 3] = [255, 200, 110];
    for (channel, gold) in pixel.iter_mut().zip(GOLD) {
        let ink = (f32::from(gold) * falloff).round() as u8;
        *channel = channel.saturating_add(ink);
    }
}

#[derive(Debug)]
enum FrameSink {
    Ppm { frame_dir: PathBuf },
    Ffmpeg(FfmpegSink),
}

#[derive(Debug)]
struct FfmpegSink {
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    output: String,
    finished: bool,
}

impl FfmpegSink {
    fn spawn(output: &str, width: u32, height: u32, fps: u32) -> Result<Self, String> {
        Self::spawn_with_program("ffmpeg", output, width, height, fps)
    }

    fn spawn_with_program(
        program: &str,
        output: &str,
        width: u32,
        height: u32,
        fps: u32,
    ) -> Result<Self, String> {
        let mut child = Command::new(program)
            .args([
                // stderr is piped but only drained after the run completes
                // (wait_with_output); keep ffmpeg quiet on the happy path so
                // its periodic progress output cannot fill the pipe buffer
                // and deadlock long renders against our stdin writes.
                "-hide_banner",
                "-nostats",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "rawvideo",
                "-pix_fmt",
                "rgb24",
                "-s",
                &format!("{width}x{height}"),
                "-framerate",
                &fps.to_string(),
                "-i",
                "-",
                "-c:v",
                "libx264",
                "-crf",
                "0",
                "-pix_fmt",
                "yuv444p",
                output,
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|err| {
                format!(
                    "unable to start ffmpeg for streamed recording: {err}; install ffmpeg and ensure it is on PATH"
                )
            })?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "unable to open ffmpeg stdin for streamed recording".to_string())?;

        Ok(Self {
            child: Some(child),
            stdin: Some(stdin),
            output: output.to_string(),
            finished: false,
        })
    }

    fn write_frame(&mut self, buffer: &[u8]) -> Result<(), String> {
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| "ffmpeg stdin is already closed".to_string())?;
        stdin
            .write_all(buffer)
            .map_err(|err| format!("unable to stream frame to ffmpeg: {err}"))
    }

    fn finish(&mut self) -> Result<(), String> {
        if self.finished {
            return Ok(());
        }
        if let Some(mut stdin) = self.stdin.take() {
            stdin
                .flush()
                .map_err(|err| format!("unable to flush ffmpeg stdin: {err}"))?;
        }
        let child = self
            .child
            .take()
            .ok_or_else(|| "ffmpeg child is already closed".to_string())?;
        let output = child
            .wait_with_output()
            .map_err(|err| format!("unable to wait for ffmpeg: {err}"))?;
        self.finished = true;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "ffmpeg failed while writing {} with status {}: {}",
                self.output,
                output.status,
                stderr.trim()
            ));
        }
        Ok(())
    }
}

fn frame_path(frame_dir: &Path, index: usize) -> PathBuf {
    let mut path = frame_dir.to_path_buf();
    path.push(format!("frame_{index:06}.ppm"));
    path
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
    use super::{FfmpegSink, FrameRecorder};
    use crate::config::Args;
    use clap::Parser;
    use std::fs;
    use std::io::Write;
    use std::path::Path;

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

    fn write_executable(path: &Path, body: &str) {
        let mut file = fs::File::create(path).expect("create fake ffmpeg");
        file.write_all(body.as_bytes()).expect("write fake ffmpeg");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = file.metadata().expect("fake metadata").permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(path, permissions).expect("chmod fake ffmpeg");
        }
    }

    #[test]
    fn negative_view_radius_is_rejected() {
        let args = recorder_args("-1.0", ".sc/test-frames-neg");
        let err = FrameRecorder::new(&args).unwrap_err();
        assert!(err.contains("view-radius"), "unexpected error: {err}");
    }

    #[test]
    fn ffmpeg_missing_returns_actionable_error() {
        let err = FfmpegSink::spawn_with_program(
            ".sc/no-such-ffmpeg-for-test",
            ".sc/missing-output.mp4",
            2,
            2,
            30,
        )
        .unwrap_err();
        assert!(
            err.contains("unable to start ffmpeg"),
            "unexpected error: {err}"
        );
        assert!(err.contains("PATH"), "unexpected error: {err}");
    }

    #[test]
    fn ffmpeg_nonzero_exit_is_propagated() {
        fs::create_dir_all(".sc/frame-tests").expect("create test dir");
        let ffmpeg = Path::new(".sc/frame-tests/fake-ffmpeg-fail");
        write_executable(
            ffmpeg,
            "#!/bin/sh\ncat >/dev/null\necho fake failure >&2\nexit 17\n",
        );
        let mut sink = FfmpegSink::spawn_with_program(
            ffmpeg.to_str().unwrap(),
            ".sc/frame-tests/fail.mp4",
            2,
            2,
            30,
        )
        .expect("spawn fake ffmpeg");
        sink.write_frame(&[0; 12]).expect("write frame");
        let err = sink.finish().unwrap_err();
        assert!(err.contains("ffmpeg failed"), "unexpected error: {err}");
        assert!(err.contains("fake failure"), "unexpected error: {err}");
    }

    #[test]
    fn ffmpeg_finish_flushes_and_waits() {
        fs::create_dir_all(".sc/frame-tests").expect("create test dir");
        let ffmpeg = Path::new(".sc/frame-tests/fake-ffmpeg-ok");
        let raw = ".sc/frame-tests/raw.rgb";
        write_executable(ffmpeg, &format!("#!/bin/sh\ncat > {raw}\nexit 0\n"));
        let mut sink = FfmpegSink::spawn_with_program(
            ffmpeg.to_str().unwrap(),
            ".sc/frame-tests/ok.mp4",
            2,
            2,
            30,
        )
        .expect("spawn fake ffmpeg");
        sink.write_frame(&[1; 12]).expect("write frame");
        sink.finish().expect("finish fake ffmpeg");
        assert_eq!(fs::read(raw).expect("read raw"), vec![1; 12]);
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
