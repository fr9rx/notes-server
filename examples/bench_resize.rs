//! Times the image pipeline. Build for the Pi alongside the server and run it
//! there to see real numbers:
//!
//!     bench_resize                  # synthetic 12 MP (4000x3000) photo
//!     bench_resize photo.jpg 20     # your own file, 20 iterations

use std::time::{Duration, Instant};

use fast_image_resize::Resizer;
use notes_server::imaging;

fn main() {
    let mut args = std::env::args().skip(1);
    let (label, input) = match args.next() {
        Some(path) => {
            let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("reading {path}: {e}"));
            (path, bytes)
        }
        None => ("synthetic 4000x3000 JPEG".to_owned(), synthetic_photo(4000, 3000)),
    };
    let iterations: usize = args.next().map_or(10, |s| s.parse().expect("iterations must be a number"));

    println!("input:       {label} ({:.1} MB)", input.len() as f64 / 1e6);
    println!("resize SIMD: {:?}", Resizer::new().cpu_extensions());
    println!("iterations:  {iterations}\n");

    // Warm-up (page faults, allocator).
    let out = imaging::process(&input).expect("input must be a supported image");

    let decode = time(iterations, || {
        image::load_from_memory(&input).unwrap();
    });
    let full = time(iterations, || {
        imaging::process(&input).unwrap();
    });

    println!("decode only        median {:>7.1} ms   min {:>7.1} ms", ms(decode.0), ms(decode.1));
    println!("full pipeline      median {:>7.1} ms   min {:>7.1} ms", ms(full.0), ms(full.1));
    println!("  = resize+encode  ~{:.1} ms", ms(full.0.saturating_sub(decode.0)));
    println!(
        "\noutput: {}x{} ({:.0} KB), thumb {}x{} ({:.0} KB)",
        out.main.width,
        out.main.height,
        out.main.bytes.len() as f64 / 1e3,
        out.thumb.width,
        out.thumb.height,
        out.thumb.bytes.len() as f64 / 1e3
    );
}

/// (median, min)
fn time(iterations: usize, mut f: impl FnMut()) -> (Duration, Duration) {
    let mut samples: Vec<Duration> = (0..iterations.max(1))
        .map(|_| {
            let start = Instant::now();
            f();
            start.elapsed()
        })
        .collect();
    samples.sort();
    (samples[samples.len() / 2], samples[0])
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1e3
}

/// A photo-like JPEG: smooth gradients plus per-pixel noise, so the encoder
/// and decoder do realistic amounts of work (a flat image would be trivial).
fn synthetic_photo(width: u16, height: u16) -> Vec<u8> {
    let mut state = 0x2545_f491_u32;
    let mut noise = move || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        (state & 0x1f) as u8
    };
    let mut rgb = Vec::with_capacity(width as usize * height as usize * 3);
    for y in 0..height as u32 {
        for x in 0..width as u32 {
            rgb.push(((x * 255 / width as u32) as u8).wrapping_add(noise()));
            rgb.push(((y * 255 / height as u32) as u8).wrapping_add(noise()));
            rgb.push((((x + y) / 16) as u8).wrapping_add(noise()));
        }
    }
    let mut out = Vec::new();
    jpeg_encoder::Encoder::new(&mut out, 90)
        .encode(&rgb, width, height, jpeg_encoder::ColorType::Rgb)
        .unwrap();
    out
}
