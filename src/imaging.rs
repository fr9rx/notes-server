//! Decode → orient → flatten → resize → encode. Pure Rust, CPU-bound: call
//! [`process`] from `spawn_blocking`.
//!
//! Resizing uses `fast_image_resize` (NEON on the Pi 5, AVX2/SSE4.1 on x86)
//! and encoding uses `jpeg-encoder`. Re-encoding also drops all metadata, so
//! EXIF GPS positions from phone photos are never published.

use std::io::Cursor;

use fast_image_resize::images::{Image as FrImage, ImageRef};
use fast_image_resize::{FilterType, PixelType, ResizeAlg, ResizeOptions, Resizer};
use image::metadata::Orientation;
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits, RgbImage};
use jpeg_encoder::{ColorType, Encoder};

/// Longest side of the stored image.
pub const MAX_DIMENSION: u32 = 1600;
/// Longest side of the thumbnail.
pub const THUMB_DIMENSION: u32 = 300;
/// Refuse to decode anything wider or taller than this (decompression bombs).
pub const MAX_SOURCE_DIMENSION: u32 = 8000;
/// Upper bound on decoder allocations.
pub const MAX_DECODE_ALLOC: u64 = 256 * 1024 * 1024;

const MAIN_QUALITY: u8 = 85;
const THUMB_QUALITY: u8 = 80;

#[derive(Debug, thiserror::Error)]
pub enum ImageError {
    #[error("unsupported or corrupt image (accepted: JPEG, PNG, WebP, GIF)")]
    Unsupported,
    #[error("image dimensions are too large (max {MAX_SOURCE_DIMENSION}x{MAX_SOURCE_DIMENSION})")]
    TooLarge,
    #[error("{0}")]
    Internal(String),
}

#[derive(Debug, Clone)]
pub struct EncodedImage {
    /// JPEG bytes.
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub struct Processed {
    pub main: EncodedImage,
    pub thumb: EncodedImage,
}

pub fn process(bytes: &[u8]) -> Result<Processed, ImageError> {
    let img = decode(bytes)?;
    let mut resizer = Resizer::new();

    let (w, h) = fit_within(img.width(), img.height(), MAX_DIMENSION);
    let main = if (w, h) == img.dimensions() {
        img
    } else {
        resize(&mut resizer, &img, w, h, FilterType::Lanczos3)?
    };

    // Thumbnail from the already-shrunk image: a fraction of the work of
    // going from the full-size original, with no visible difference at 300px.
    let (tw, th) = fit_within(main.width(), main.height(), THUMB_DIMENSION);
    let thumb = if (tw, th) == main.dimensions() {
        main.clone()
    } else {
        resize(&mut resizer, &main, tw, th, FilterType::Bilinear)?
    };

    Ok(Processed {
        main: encode_jpeg(&main, MAIN_QUALITY, true)?,
        thumb: encode_jpeg(&thumb, THUMB_QUALITY, false)?,
    })
}

/// Decode with the format sniffed from the bytes (never the client's
/// Content-Type), apply EXIF orientation, and flatten to RGB8.
fn decode(bytes: &[u8]) -> Result<RgbImage, ImageError> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| ImageError::Unsupported)?;
    match reader.format() {
        Some(ImageFormat::Jpeg | ImageFormat::Png | ImageFormat::WebP | ImageFormat::Gif) => {}
        _ => return Err(ImageError::Unsupported),
    }

    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SOURCE_DIMENSION);
    limits.max_image_height = Some(MAX_SOURCE_DIMENSION);
    limits.max_alloc = Some(MAX_DECODE_ALLOC);
    reader.limits(limits);

    let mut decoder = reader.into_decoder().map_err(map_decode_error)?;
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let mut img = DynamicImage::from_decoder(decoder).map_err(map_decode_error)?;
    img.apply_orientation(orientation);
    Ok(flatten_to_rgb8(img))
}

fn map_decode_error(e: image::ImageError) -> ImageError {
    match e {
        image::ImageError::Limits(_) => ImageError::TooLarge,
        _ => ImageError::Unsupported,
    }
}

/// Composite any alpha channel onto white; JPEG has no transparency and a
/// black background (what a plain channel drop gives) looks broken.
fn flatten_to_rgb8(img: DynamicImage) -> RgbImage {
    if !img.color().has_alpha() {
        return img.into_rgb8();
    }
    let rgba = img.into_rgba8();
    let (w, h) = rgba.dimensions();
    let mut out = Vec::with_capacity(w as usize * h as usize * 3);
    for px in rgba.as_raw().chunks_exact(4) {
        let a = u32::from(px[3]);
        for &c in &px[..3] {
            out.push(((u32::from(c) * a + 255 * (255 - a) + 127) / 255) as u8);
        }
    }
    RgbImage::from_raw(w, h, out).expect("buffer sized for w*h*3")
}

/// Largest size fitting in `max`×`max` with the same aspect ratio. Never upscales.
pub fn fit_within(width: u32, height: u32, max: u32) -> (u32, u32) {
    if width <= max && height <= max {
        return (width, height);
    }
    let scale = f64::from(max) / f64::from(width.max(height));
    let scaled = |v: u32| ((f64::from(v) * scale).round() as u32).clamp(1, max);
    (scaled(width), scaled(height))
}

fn resize(
    resizer: &mut Resizer,
    src: &RgbImage,
    width: u32,
    height: u32,
    filter: FilterType,
) -> Result<RgbImage, ImageError> {
    let src_view = ImageRef::new(src.width(), src.height(), src.as_raw(), PixelType::U8x3)
        .map_err(|e| ImageError::Internal(e.to_string()))?;
    let mut dst = FrImage::new(width, height, PixelType::U8x3);
    let options = ResizeOptions::new().resize_alg(ResizeAlg::Convolution(filter));
    resizer
        .resize(&src_view, &mut dst, &options)
        .map_err(|e| ImageError::Internal(e.to_string()))?;
    RgbImage::from_raw(width, height, dst.into_vec())
        .ok_or_else(|| ImageError::Internal("resized buffer has wrong size".into()))
}

fn encode_jpeg(img: &RgbImage, quality: u8, progressive: bool) -> Result<EncodedImage, ImageError> {
    let (width, height) = img.dimensions();
    let to_u16 =
        |v: u32| u16::try_from(v).map_err(|_| ImageError::Internal("image too large for JPEG".into()));
    let mut bytes = Vec::with_capacity(img.as_raw().len() / 8);
    let mut encoder = Encoder::new(&mut bytes, quality);
    encoder.set_progressive(progressive);
    encoder
        .encode(img.as_raw(), to_u16(width)?, to_u16(height)?, ColorType::Rgb)
        .map_err(|e| ImageError::Internal(e.to_string()))?;
    Ok(EncodedImage { bytes, width, height })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgb, Rgba, RgbaImage};

    fn jpeg(width: u32, height: u32, exif: Option<&[u8]>) -> Vec<u8> {
        let img: RgbImage = ImageBuffer::from_fn(width, height, |x, y| {
            Rgb([(x % 256) as u8, (y % 256) as u8, 128])
        });
        let mut out = Vec::new();
        let mut enc = Encoder::new(&mut out, 90);
        if let Some(exif) = exif {
            enc.add_exif_metadata(exif).unwrap();
        }
        enc.encode(img.as_raw(), width as u16, height as u16, ColorType::Rgb)
            .unwrap();
        out
    }

    fn decode_jpeg(bytes: &[u8]) -> RgbImage {
        image::load_from_memory_with_format(bytes, ImageFormat::Jpeg)
            .unwrap()
            .into_rgb8()
    }

    #[test]
    fn fit_within_keeps_aspect_and_never_upscales() {
        assert_eq!(fit_within(4000, 2000, 1600), (1600, 800));
        assert_eq!(fit_within(2000, 4000, 1600), (800, 1600));
        assert_eq!(fit_within(800, 600, 1600), (800, 600));
        assert_eq!(fit_within(1600, 1600, 1600), (1600, 1600));
        assert_eq!(fit_within(10000, 1, 300), (300, 1));
    }

    #[test]
    fn large_image_is_shrunk_with_thumbnail() {
        let out = process(&jpeg(4000, 2000, None)).unwrap();
        assert_eq!((out.main.width, out.main.height), (1600, 800));
        assert_eq!((out.thumb.width, out.thumb.height), (300, 150));
        // The output really is a JPEG of the reported size.
        assert_eq!(decode_jpeg(&out.main.bytes).dimensions(), (1600, 800));
        assert_eq!(decode_jpeg(&out.thumb.bytes).dimensions(), (300, 150));
    }

    #[test]
    fn small_image_is_not_upscaled() {
        let out = process(&jpeg(800, 600, None)).unwrap();
        assert_eq!((out.main.width, out.main.height), (800, 600));
        assert_eq!((out.thumb.width, out.thumb.height), (300, 225));
    }

    #[test]
    fn transparent_png_is_flattened_onto_white() {
        let img = RgbaImage::from_pixel(64, 64, Rgba([0, 0, 0, 0]));
        let mut png = Vec::new();
        DynamicImage::ImageRgba8(img)
            .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
            .unwrap();

        let out = process(&png).unwrap();
        let decoded = decode_jpeg(&out.main.bytes);
        let px = decoded.get_pixel(32, 32);
        assert!(px.0.iter().all(|&c| c > 245), "expected white, got {px:?}");
    }

    #[test]
    fn exif_orientation_is_applied() {
        // Minimal little-endian TIFF with one IFD entry: Orientation (0x0112) = 6,
        // i.e. "rotate 90° clockwise to display".
        let exif: &[u8] = &[
            b'I', b'I', 0x2A, 0x00, 0x08, 0x00, 0x00, 0x00, // header, IFD at offset 8
            0x01, 0x00, // 1 entry
            0x12, 0x01, 0x03, 0x00, 0x01, 0x00, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, // no next IFD
        ];
        let out = process(&jpeg(400, 200, Some(exif))).unwrap();
        assert_eq!((out.main.width, out.main.height), (200, 400));
    }

    #[test]
    fn garbage_is_rejected() {
        assert!(matches!(process(b"definitely not an image"), Err(ImageError::Unsupported)));
        assert!(matches!(process(&[]), Err(ImageError::Unsupported)));
        // Valid magic bytes, truncated body.
        let mut truncated = jpeg(100, 100, None);
        truncated.truncate(40);
        assert!(matches!(process(&truncated), Err(ImageError::Unsupported)));
    }

    #[test]
    fn oversized_dimensions_are_rejected() {
        // A tiny PNG claiming 9000x10 pixels: rejected from the header alone.
        let img = image::GrayImage::new(MAX_SOURCE_DIMENSION + 1000, 10);
        let mut png = Vec::new();
        DynamicImage::ImageLuma8(img)
            .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
            .unwrap();
        assert!(matches!(process(&png), Err(ImageError::TooLarge)));
    }
}
