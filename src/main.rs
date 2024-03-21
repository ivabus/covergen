mod decode;

use rand::Rng;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tiny_skia::*;

use samplerate::{convert, ConverterType};

use decode::decode_file;

const SMOOTH_FACTOR: usize = 640;
const OSIZE: usize = 2048;

fn min(v: &Vec<f32>) -> f32 {
    let mut m = f32::MAX;
    for i in v {
        if *i < m {
            m = *i
        }
    }
    m
}
fn max(v: &Vec<f32>) -> f32 {
    let mut m = f32::MIN;
    for i in v {
        if *i > m {
            m = *i
        }
    }
    m
}

fn mid(k: &[f32]) -> f32 {
    let mut sum = 0f32;
    for i in k {
        sum += *i;
    }
    sum / k.len() as f32
}

fn main() {
    let mut args = std::env::args();
    args.next();
    let mut no_background = false;
    let fa = args.next().unwrap();
    let input_path;
    if fa == "-n" {
        no_background = true;
        input_path = args.next().unwrap();
    } else {
        input_path = fa;
    }

    let start_decode = Instant::now();
    let (sample_rate, channels, samples) = decode_file(Path::new(&input_path));
    eprintln!(
        "Decoded {} samples with {} samplerate in {}s",
        samples.len(),
        &sample_rate,
        (Instant::now() - start_decode).as_secs_f32()
    );

    let resampled = if sample_rate != 48000 {
        let start_resample = Instant::now();
        let res = convert(
            sample_rate,
            48000,
            channels,
            ConverterType::Linear,
            &samples,
        )
        .unwrap();
        eprintln!(
            "Resampled in {}s",
            (Instant::now() - start_resample).as_secs_f32()
        );
        res
    } else {
        samples
    };

    // Stereo thingy
    let start_merge = Instant::now();
    let mut samples = resampled
        .chunks_exact(channels)
        .map(|x| {
            let mut sum = 0.0;
            for channel in x.iter().take(channels) {
                sum += channel;
            }
            sum
        })
        .collect::<Vec<f32>>();
    eprintln!(
        "Merged channels in {}s",
        (Instant::now() - start_merge).as_secs_f32()
    );

    let start_fft = Instant::now();
    let mut planner = realfft::RealFftPlanner::<f32>::new();
    let mut output = vec![realfft::num_complex::Complex32::new(0f32, 0f32); samples.len() / 2 + 1];
    let fft = planner.plan_fft_forward(samples.len());
    let res = fft.process(&mut samples, &mut output);
    if let Err(e) = res {
        eprintln!("FFT error: {}", e)
    }

    let output = output[..=sample_rate as usize / 2 + SMOOTH_FACTOR]
        .iter()
        .map(|realfft::num_complex::Complex32 { re, im }| (re * re + im * im).powf(0.5))
        .collect::<Vec<f32>>();
    let mino = min(&output);
    let maxo = max(&output) - mino;
    let good_data = (0..=20000)
        .map(|x| ((mid(&output[x..x + SMOOTH_FACTOR]).log10() - mino.log10()) / maxo.log10()))
        .collect::<Vec<f32>>();
    let mut lines = (0..10)
        .map(|x| mid(&good_data[2000 * x..500 * (4 * x + 1)]))
        .collect::<Vec<f32>>();
    let min = min(&lines);
    let max = max(&lines) - min;
    lines.iter_mut().for_each(|x| *x = (*x - min) / max);
    eprintln!(
        "Analysis done in {}s",
        (Instant::now() - start_fft).as_secs_f32()
    );

    let start_drawing = Instant::now();
    let hmargin = OSIZE / 31;
    let hradius = OSIZE / 31 * 2;

    let mut pixmap = Pixmap::new(OSIZE as u32, OSIZE as u32).unwrap();
    if !no_background {
        let mut rng = rand::thread_rng();
        let yes: bool = rng.gen();
        let paint = Paint::<'_> {
            anti_alias: false,
            shader: LinearGradient::new(
                if yes {
                    Point::from_xy(0.0, 0.0)
                } else {
                    Point::from_xy(OSIZE as f32, OSIZE as f32)
                },
                if yes {
                    Point::from_xy(OSIZE as f32, OSIZE as f32)
                } else {
                    Point::from_xy(0.0, 0.0)
                },
                vec![
                    GradientStop::new(
                        0.0,
                        Color::from_rgba8(
                            rng.gen_range(0..235),
                            rng.gen_range(0..235),
                            rng.gen_range(0..235),
                            255,
                        ),
                    ),
                    GradientStop::new(
                        1.0,
                        Color::from_rgba8(
                            rng.gen_range(0..235),
                            rng.gen_range(0..235),
                            rng.gen_range(0..235),
                            255,
                        ),
                    ),
                ],
                SpreadMode::Pad,
                Transform::identity(),
            )
            .unwrap(),
            ..Default::default()
        };

        let mut pb = PathBuilder::new();
        pb.move_to(0.0, 0.0);
        pb.line_to(0.0, OSIZE as f32);
        pb.line_to(OSIZE as f32, OSIZE as f32);
        pb.line_to(OSIZE as f32, 0.0);
        pb.close();
        let path = pb.finish().unwrap();
        pixmap.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
    let stroke = tiny_skia::Stroke {
        width: hradius as f32 + OSIZE as f32 / 64.0,
        line_cap: LineCap::Round,
        ..Default::default()
    };
    let mut index = hmargin;
    let mut paint = Paint::default();
    paint.set_color_rgba8(0, 0, 0, 16);
    for line in &lines {
        paint.anti_alias = true;
        let path: tiny_skia::Path = {
            let mut pb = PathBuilder::new();
            if *line == 0.0 {
                pb.move_to(index as f32 + hradius as f32 / 2.0, OSIZE as f32 / 2.0);
                pb.line_to(index as f32 + hradius as f32 / 2.0, OSIZE as f32 / 2.0);
            } else {
                pb.move_to(
                    index as f32 + hradius as f32 / 2.0,
                    OSIZE as f32 / 2.0 - (OSIZE / 2 - hmargin) as f32 * *line + hradius as f32,
                );
                pb.line_to(
                    index as f32 + hradius as f32 / 2.0,
                    OSIZE as f32 / 2.0 + (OSIZE / 2 - hmargin) as f32 * *line - hradius as f32,
                );
            }
            pb.finish().unwrap()
        };

        pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        index += hmargin + hradius;
    }
    let stroke = tiny_skia::Stroke {
        width: hradius as f32,
        line_cap: LineCap::Round,
        ..Default::default()
    };

    paint.set_color_rgba8(255, 255, 255, 128);
    let mut index = hmargin;
    for line in lines {
        paint.anti_alias = true;
        let path: tiny_skia::Path = {
            let mut pb = PathBuilder::new();
            if line == 0.0 {
                pb.move_to(index as f32 + hradius as f32 / 2.0, OSIZE as f32 / 2.0);
                pb.line_to(index as f32 + hradius as f32 / 2.0, OSIZE as f32 / 2.0);
            } else {
                pb.move_to(
                    index as f32 + hradius as f32 / 2.0,
                    OSIZE as f32 / 2.0 - (OSIZE / 2 - hmargin) as f32 * line + hradius as f32,
                );
                pb.line_to(
                    index as f32 + hradius as f32 / 2.0,
                    OSIZE as f32 / 2.0 + (OSIZE / 2 - hmargin) as f32 * line - hradius as f32,
                );
            }
            pb.finish().unwrap()
        };

        pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        index += hmargin + hradius;
    }

    let mut input = PathBuf::from(input_path)
        .file_stem()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    input.push_str(".png");
    pixmap.save_png(&input).unwrap();
    eprintln!(
        "Finished drawing in {}s and wrote to '{}'",
        (Instant::now() - start_drawing).as_secs_f32(),
        input
    );
}
