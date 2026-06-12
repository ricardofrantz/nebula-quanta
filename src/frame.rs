use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};

use crate::{
    config::{Args, ColorMode},
    particle::ParticleSoa,
};

#[derive(Debug)]
pub struct FrameRecorder {
    sinks: Vec<ModeSink>,
    width: u32,
    height: u32,
    every_steps: usize,
    next_index: usize,
    background: [u8; 3],
    buffers: Vec<Vec<u8>>,
    point_radius: i32,
    trail_decay: f32,
    frame_count: usize,
    view_radius: f64,
}

#[derive(Debug)]
struct ModeSink {
    mode: ColorMode,
    sink: FrameSink,
    scale: Option<f64>,
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

        let modes = args.color_modes()?;
        let width = args.width;
        let height = args.height;
        let buffer_len = frame_byte_size(width, height)?;
        let multi = modes.len() > 1;
        let mut sinks = Vec::with_capacity(modes.len());
        for (index, mode) in modes.iter().copied().enumerate() {
            let sink = if let Some(output) = &args.output {
                let output = output_for_mode(output, mode, index)?;
                FrameSink::Ffmpeg(FfmpegSink::spawn(&output, width, height, args.fps)?)
            } else {
                let root = PathBuf::from(args.frames_dir.as_deref().unwrap_or("frames"));
                let frame_dir = if multi {
                    root.join(mode.as_str())
                } else {
                    root
                };
                fs::create_dir_all(&frame_dir)
                    .map_err(|err| format!("unable to create frame directory: {err}"))?;
                FrameSink::Ppm { frame_dir }
            };
            sinks.push(ModeSink {
                mode,
                sink,
                scale: None,
            });
        }
        let background = [0, 0, 0];
        let point_radius = 0;
        let buffers = vec![vec![0; buffer_len]; sinks.len()];

        Ok(Self {
            sinks,
            width,
            height,
            every_steps: args.every_steps,
            next_index: 0,
            background,
            buffers,
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
        ax: &[f64],
        ay: &[f64],
    ) -> Result<(), String> {
        if !self.should_record(step) {
            return Ok(());
        }
        if ax.len() != particles.len() || ay.len() != particles.len() {
            return Err("acceleration buffers must match particle count".to_string());
        }

        let bounds = self.render_bounds(bounds);
        let quantities = frame_quantities(&self.sinks, particles, bounds, ax, ay);
        for (sink_index, quantities) in quantities.iter().enumerate() {
            if self.sinks[sink_index].mode != ColorMode::Golden
                && self.sinks[sink_index].scale.is_none()
            {
                self.sinks[sink_index].scale = Some(first_frame_scale(quantities));
            }
        }
        for i in 0..self.sinks.len() {
            let mode = self.sinks[i].mode;
            let scale = self.sinks[i].scale;
            render_particles_into(
                &mut self.buffers[i],
                self.background,
                self.trail_decay,
                self.point_radius,
                self.width,
                self.height,
                self.view_radius,
                particles,
                bounds,
                mode,
                scale,
                &quantities[i],
            )?;
        }
        let frame_index = self.next_index;
        self.next_index = self.next_index.saturating_add(1);
        for (mode_sink, buffer) in self.sinks.iter_mut().zip(&self.buffers) {
            match &mut mode_sink.sink {
                FrameSink::Ppm { frame_dir } => {
                    let path = frame_path(frame_dir, frame_index);
                    write_ppm(&path, self.width, self.height, buffer)
                        .map_err(|err| format!("unable to write frame {frame_index}: {err}"))?;
                }
                FrameSink::Ffmpeg(sink) => sink.write_frame(buffer)?,
            }
        }
        self.frame_count = self.frame_count.saturating_add(1);
        Ok(())
    }

    pub fn frame_count(&self) -> usize {
        self.frame_count
    }
    pub fn color_by_receipt(&self) -> String {
        self.sinks
            .iter()
            .map(|s| s.mode.as_str())
            .collect::<Vec<_>>()
            .join(",")
    }
    pub fn color_scale_receipts(&self) -> Vec<(ColorMode, Option<f64>)> {
        self.sinks.iter().map(|s| (s.mode, s.scale)).collect()
    }

    pub fn finish(&mut self) -> Result<(), String> {
        let mut errors = Vec::new();
        for mode_sink in &mut self.sinks {
            if let FrameSink::Ffmpeg(sink) = &mut mode_sink.sink
                && let Err(err) = sink.finish()
            {
                errors.push(format!("{}: {err}", mode_sink.mode.as_str()));
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("; "))
        }
    }

    pub fn render_command(&self, output: &str, fps: u32) -> Option<String> {
        match self.sinks.first().map(|s| &s.sink) {
            Some(FrameSink::Ppm { frame_dir }) => Some(format!(
                "ffmpeg -y -framerate {fps} -i {}/frame_%06d.ppm -s {}x{} -c:v libx264 -crf 0 -pix_fmt yuv444p {}",
                frame_dir.display(),
                self.width,
                self.height,
                output
            )),
            _ => None,
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
}

fn render_particles_into(
    buffer: &mut [u8],
    background: [u8; 3],
    trail_decay: f32,
    point_radius: i32,
    width_u32: u32,
    height_u32: u32,
    _view_radius: f64,
    particles: &ParticleSoa,
    bounds: (f64, f64, f64, f64),
    mode: ColorMode,
    scale: Option<f64>,
    quantities: &[f64],
) -> Result<(), String> {
    for pixel in buffer.chunks_exact_mut(3) {
        pixel[0] = decayed_channel(pixel[0], background[0], trail_decay);
        pixel[1] = decayed_channel(pixel[1], background[1], trail_decay);
        pixel[2] = decayed_channel(pixel[2], background[2], trail_decay);
    }
    let (x_min, x_max, y_min, y_max) = bounds;
    let width = f64::from(width_u32);
    let height = f64::from(height_u32);
    let sx = (width - 1.0) / (x_max - x_min).max(1e-12);
    let sy = (height - 1.0) / (y_max - y_min).max(1e-12);
    let x_dim = usize::try_from(width_u32).map_err(|_| "invalid frame width".to_string())?;
    let w = i64::from(width_u32);
    let h = i64::from(height_u32);
    let dot_kernel = dot_kernel(point_radius);
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
            if idx + 2 >= buffer.len() {
                continue;
            }
            stamp_ink_mode(
                &mut buffer[idx..idx + 3],
                falloff,
                mode,
                scale,
                quantities[i],
            );
        }
    }
    Ok(())
}

fn frame_quantities(
    sinks: &[ModeSink],
    particles: &ParticleSoa,
    bounds: (f64, f64, f64, f64),
    ax: &[f64],
    ay: &[f64],
) -> Vec<Vec<f64>> {
    sinks
        .iter()
        .map(|sink| match sink.mode {
            ColorMode::Golden => vec![0.0; particles.len()],
            ColorMode::Speed => (0..particles.len())
                .map(|i| particles.vx[i].hypot(particles.vy[i]))
                .collect(),
            ColorMode::Accel => (0..particles.len()).map(|i| ax[i].hypot(ay[i])).collect(),
            ColorMode::Density => leaf_bucket_density(particles, bounds),
        })
        .collect()
}

fn leaf_bucket_density(particles: &ParticleSoa, bounds: (f64, f64, f64, f64)) -> Vec<f64> {
    // Rendering density uses an independent frame-space leaf-bucket occupancy heuristic.
    // It does not alter theta/softening local-density policies used by the solver.
    let n = particles.len();
    if n == 0 {
        return Vec::new();
    }
    let bins = (n.isqrt().clamp(4, 256)).max(1);
    let mut counts = vec![0u32; bins * bins];
    let mut indices = vec![usize::MAX; n];
    let (x_min, x_max, y_min, y_max) = bounds;
    let sx = (bins as f64) / (x_max - x_min).max(1e-12);
    let sy = (bins as f64) / (y_max - y_min).max(1e-12);
    for (i, idx) in indices.iter_mut().enumerate() {
        let x = particles.x[i];
        let y = particles.y[i];
        if !x.is_finite() || !y.is_finite() {
            continue;
        }
        let bx = (((x - x_min) * sx).floor() as isize).clamp(0, bins as isize - 1) as usize;
        let by = (((y - y_min) * sy).floor() as isize).clamp(0, bins as isize - 1) as usize;
        let bucket = by * bins + bx;
        *idx = bucket;
        counts[bucket] = counts[bucket].saturating_add(1);
    }
    indices
        .into_iter()
        .map(|idx| {
            if idx == usize::MAX {
                0.0
            } else {
                f64::from(counts[idx])
            }
        })
        .collect()
}

fn first_frame_scale(values: &[f64]) -> f64 {
    let mut finite: Vec<f64> = values
        .iter()
        .copied()
        .filter(|v| v.is_finite() && *v > 0.0)
        .collect();
    if finite.is_empty() {
        return 1.0;
    }
    finite.sort_by(f64::total_cmp);
    let idx = ((finite.len() - 1) * 99) / 100;
    (finite[idx] * 1.5).max(f64::EPSILON)
}

fn output_for_mode(output: &str, mode: ColorMode, index: usize) -> Result<String, String> {
    if index == 0 {
        return Ok(output.to_string());
    }
    let path = Path::new(output);
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "output path must have a valid file name".to_string())?;
    let name = if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        format!("{stem}-{}.{}", mode.as_str(), ext)
    } else {
        format!("{stem}-{}", mode.as_str())
    };
    Ok(path.with_file_name(name).to_string_lossy().into_owned())
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

fn stamp_ink_mode(
    pixel: &mut [u8],
    falloff: f32,
    mode: ColorMode,
    scale: Option<f64>,
    quantity: f64,
) {
    if mode == ColorMode::Golden {
        stamp_ink(pixel, falloff);
        return;
    }
    let scale = scale.unwrap_or(1.0).max(f64::EPSILON);
    let t = (quantity.max(0.0) / scale).asinh().clamp(0.0, 1.0) as f32;
    let color = gradient(mode, t);
    for (channel, value) in pixel.iter_mut().zip(color) {
        let ink = (f32::from(value) * falloff).round() as u8;
        *channel = channel.saturating_add(ink);
    }
}

fn gradient(mode: ColorMode, t: f32) -> [u8; 3] {
    let stops = match mode {
        ColorMode::Speed => [[90, 0, 0], [255, 190, 0], [255, 255, 255]],
        ColorMode::Accel => [[0, 8, 90], [0, 220, 255], [255, 255, 255]],
        ColorMode::Density => [[45, 0, 80], [255, 0, 200], [255, 255, 255]],
        ColorMode::Golden => [[255, 200, 110], [255, 200, 110], [255, 200, 110]],
    };
    let (a, b, u) = if t <= 0.5 {
        (stops[0], stops[1], t * 2.0)
    } else {
        (stops[1], stops[2], (t - 0.5) * 2.0)
    };
    [
        lerp(a[0], b[0], u),
        lerp(a[1], b[1], u),
        lerp(a[2], b[2], u),
    ]
}

fn lerp(a: u8, b: u8, t: f32) -> u8 {
    (f32::from(a) + (f32::from(b) - f32::from(a)) * t).round() as u8
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
                "-hide_banner", "-nostats", "-loglevel", "error", "-y", "-f", "rawvideo", "-pix_fmt", "rgb24", "-s", &format!("{width}x{height}"), "-framerate", &fps.to_string(), "-i", "-", "-c:v", "libx264", "-crf", "0", "-pix_fmt", "yuv444p", output,
            ])
            .stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::piped())
            .spawn().map_err(|err| format!("unable to start ffmpeg for streamed recording: {err}; install ffmpeg and ensure it is on PATH"))?;
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
    frame_dir.join(format!("frame_{index:06}.ppm"))
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
    use super::{FfmpegSink, FrameRecorder, first_frame_scale};
    use crate::{config::Args, particle::ParticleSoa};
    use clap::Parser;
    use std::fs;
    use std::io::Write;
    use std::path::Path;

    fn recorder_args(extra: &[&str]) -> Args {
        let mut args = vec![
            "nq",
            "--record",
            "--frames-dir",
            ".sc/frame-test-frames",
            "--width",
            "9",
            "--height",
            "9",
            "--view-radius",
            "2.0",
        ];
        args.extend_from_slice(extra);
        Args::parse_from(args)
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
        let args = Args::parse_from([
            "nq",
            "--record",
            "--frames-dir",
            ".sc/test-frames-neg",
            "--width",
            "9",
            "--height",
            "9",
            "--view-radius=-1.0",
        ]);
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
        let recorder = FrameRecorder::new(&recorder_args(&[])).unwrap();
        assert_eq!(
            recorder.render_bounds((-100.0, 100.0, -50.0, 50.0)),
            (-2.0, 2.0, -2.0, 2.0)
        );
    }
    #[test]
    fn multi_mode_frames_dir_writes_mode_subdirs_and_fixed_scale() {
        let _ = fs::remove_dir_all(".sc/frame-test-frames");
        let args = recorder_args(&["--color-by", "speed,accel,density"]);
        let mut rec = FrameRecorder::new(&args).unwrap();
        let mut p = ParticleSoa::with_len(2);
        p.x = vec![-1.0, 1.0];
        p.y = vec![0.0, 0.0];
        p.vx = vec![1.0, 1000.0];
        p.vy = vec![0.0, 0.0];
        p.m = vec![1.0, 1.0];
        rec.record_step(0, &p, (-2.0, 2.0, -2.0, 2.0), &[1.0, 2.0], &[0.0, 0.0])
            .unwrap();
        let scales = rec.color_scale_receipts();
        p.vx[1] = 1.0e9;
        rec.record_step(1, &p, (-2.0, 2.0, -2.0, 2.0), &[1.0e9, 2.0], &[0.0, 0.0])
            .unwrap();
        assert_eq!(scales, rec.color_scale_receipts());
        for mode in ["speed", "accel", "density"] {
            assert_eq!(
                fs::read_dir(format!(".sc/frame-test-frames/{mode}"))
                    .unwrap()
                    .count(),
                2
            );
        }
    }
    #[test]
    fn p99_scale_has_headroom() {
        assert_eq!(first_frame_scale(&[1.0, 2.0, 100.0]), 3.0);
    }
}
