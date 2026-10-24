use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::path::Path;

use image::metadata::Orientation;
use image::{DynamicImage, GenericImageView, ImageReader, Limits, RgbaImage};

use crate::discovery::SourceItem;
use crate::error::CoreError;
use crate::format::{self, ImageFormat};
use crate::options::{MetadataPolicy, OptimizeOptions};

const MAX_DIMENSION: u32 = 50_000;
const MAX_ALLOC: u64 = 768 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EngineStage {
    Decode,
    Transform,
    Encode,
}

#[derive(Debug)]
pub struct OptimizedImage {
    pub bytes: Vec<u8>,
    pub format: ImageFormat,
    pub width: u32,
    pub height: u32,
}

pub fn optimize(item: &SourceItem, options: &OptimizeOptions) -> Result<OptimizedImage, CoreError> {
    optimize_with(item, options, |_| {})
}

pub fn optimize_with<F>(
    item: &SourceItem,
    options: &OptimizeOptions,
    mut on_stage: F,
) -> Result<OptimizedImage, CoreError>
where
    F: FnMut(EngineStage),
{
    on_stage(EngineStage::Decode);
    let mut image = decode(&item.path)?;

    on_stage(EngineStage::Transform);
    if let Some(orientation) = Orientation::from_exif(item.probe.orientation) {
        image.apply_orientation(orientation);
    }
    let mut rgba = image.to_rgba8();
    if let Some(max) = options.max_dimension {
        let (width, height) = rgba.dimensions();
        let longest = width.max(height);
        if longest > max {
            let scale = max as f64 / longest as f64;
            let new_width = ((width as f64 * scale).round() as u32).max(1);
            let new_height = ((height as f64 * scale).round() as u32).max(1);
            rgba = image::imageops::resize(
                &rgba,
                new_width,
                new_height,
                image::imageops::FilterType::Lanczos3,
            );
        }
    }

    let format = options.effective_format(item.format);
    let metadata = options.metadata;
    let icc = if metadata == MetadataPolicy::ColorOnly && item.probe.has_icc {
        extract_icc(&item.path, item.format)
    } else {
        None
    };

    on_stage(EngineStage::Encode);
    let (bytes, width, height) = match format {
        ImageFormat::Jpeg => {
            let bytes = encode_jpeg(&rgba, options.quality, options.background, icc.as_deref())?;
            (bytes, rgba.width(), rgba.height())
        }
        ImageFormat::Png => {
            let bytes = encode_png(&rgba, options.png_effort, icc.as_deref())?;
            (bytes, rgba.width(), rgba.height())
        }
        ImageFormat::WebP => {
            let bytes = encode_webp(&rgba, options.quality, options.lossless)?;
            (bytes, rgba.width(), rgba.height())
        }
    };

    Ok(OptimizedImage {
        bytes,
        format,
        width,
        height,
    })
}

fn decode(path: &Path) -> Result<DynamicImage, CoreError> {
    let reader = ImageReader::open(path).map_err(|source| CoreError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let reader = reader
        .with_guessed_format()
        .map_err(|source| CoreError::Read {
            path: path.to_path_buf(),
            source,
        })?;
    let mut reader = reader;
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_DIMENSION);
    limits.max_image_height = Some(MAX_DIMENSION);
    limits.max_alloc = Some(MAX_ALLOC);
    reader.limits(limits);
    reader.decode().map_err(|error| match error {
        image::ImageError::Limits(_) => CoreError::TooLarge,
        other => CoreError::Decode(other.to_string()),
    })
}

fn flatten(rgba: &RgbaImage, background: [u8; 3]) -> image::RgbImage {
    let (width, height) = rgba.dimensions();
    let mut rgb = image::RgbImage::new(width, height);
    for (pixel, out) in rgba.pixels().zip(rgb.pixels_mut()) {
        let alpha = pixel[3] as u16;
        let inv = 255u16 - alpha;
        out[0] = ((pixel[0] as u16 * alpha + background[0] as u16 * inv) / 255) as u8;
        out[1] = ((pixel[1] as u16 * alpha + background[1] as u16 * inv) / 255) as u8;
        out[2] = ((pixel[2] as u16 * alpha + background[2] as u16 * inv) / 255) as u8;
    }
    rgb
}

fn encode_jpeg(
    rgba: &RgbaImage,
    quality: u8,
    background: [u8; 3],
    icc: Option<&[u8]>,
) -> Result<Vec<u8>, CoreError> {
    let rgb = flatten(rgba, background);
    let mut bytes = Vec::new();
    {
        let mut encoder =
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, quality.clamp(1, 100));
        encoder
            .encode_image(&DynamicImage::ImageRgb8(rgb))
            .map_err(|error| CoreError::Encode(error.to_string()))?;
    }
    if let Some(icc) = icc {
        insert_jpeg_icc(&mut bytes, icc);
    }
    Ok(bytes)
}

fn encode_png(rgba: &RgbaImage, effort: u8, icc: Option<&[u8]>) -> Result<Vec<u8>, CoreError> {
    let (width, height) = rgba.dimensions();
    let opaque = rgba.pixels().all(|pixel| pixel[3] == 255);
    let mut bytes = Vec::new();
    if opaque {
        let rgb = image::DynamicImage::ImageRgba8(rgba.clone()).to_rgb8();
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Best);
        let mut writer = encoder
            .write_header()
            .map_err(|error| CoreError::Encode(error.to_string()))?;
        writer
            .write_image_data(rgb.as_raw())
            .map_err(|error| CoreError::Encode(error.to_string()))?;
    } else {
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Best);
        let mut writer = encoder
            .write_header()
            .map_err(|error| CoreError::Encode(error.to_string()))?;
        writer
            .write_image_data(rgba.as_raw())
            .map_err(|error| CoreError::Encode(error.to_string()))?;
    }
    if let Some(icc) = icc {
        inject_png_icc(&mut bytes, icc);
    }
    let optimized = oxipng::optimize_from_memory(&bytes, &oxipng::Options::from_preset(effort))
        .map_err(|error| CoreError::Encode(error.to_string()))?;
    Ok(optimized)
}

fn encode_webp(rgba: &RgbaImage, quality: u8, lossless: bool) -> Result<Vec<u8>, CoreError> {
    let (width, height) = rgba.dimensions();
    let encoder = webp::Encoder::from_rgba(rgba.as_raw(), width, height);
    let memory = if lossless {
        encoder.encode_lossless()
    } else {
        encoder.encode(quality.clamp(1, 100) as f32)
    };
    Ok(memory.to_vec())
}

fn insert_jpeg_icc(bytes: &mut Vec<u8>, icc: &[u8]) {
    if !bytes.starts_with(&[0xFF, 0xD8]) || icc.is_empty() {
        return;
    }
    const MAX_CHUNK: usize = 65_519;
    let chunks: Vec<&[u8]> = icc.chunks(MAX_CHUNK).collect();
    if chunks.len() > 255 {
        return;
    }
    let mut out = Vec::with_capacity(bytes.len() + icc.len() + chunks.len() * 18);
    out.extend_from_slice(&bytes[..2]);
    for (index, chunk) in chunks.iter().enumerate() {
        out.extend_from_slice(&[0xFF, 0xE2]);
        let length = (2 + 14 + chunk.len()) as u16;
        out.extend_from_slice(&length.to_be_bytes());
        out.extend_from_slice(b"ICC_PROFILE\0");
        out.push((index + 1) as u8);
        out.push(chunks.len() as u8);
        out.extend_from_slice(chunk);
    }
    out.extend_from_slice(&bytes[2..]);
    *bytes = out;
}

fn inject_png_icc(bytes: &mut Vec<u8>, icc: &[u8]) {
    if bytes.len() < 33 || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") || icc.is_empty() {
        return;
    }
    let mut compressed = Vec::new();
    {
        let mut encoder =
            flate2::write::ZlibEncoder::new(&mut compressed, flate2::Compression::default());
        if encoder.write_all(icc).is_err() {
            return;
        }
        if encoder.finish().is_err() {
            return;
        }
    }
    let mut data = Vec::with_capacity(icc.len() / 2 + 8);
    data.extend_from_slice(b"ImgShrink\0");
    data.push(0);
    data.extend_from_slice(&compressed);

    let ihdr_length = u32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
    let insert_at = 8 + 4 + 4 + ihdr_length + 4;
    if insert_at > bytes.len() {
        return;
    }
    let mut chunk = Vec::with_capacity(data.len() + 12);
    chunk.extend_from_slice(&(data.len() as u32).to_be_bytes());
    chunk.extend_from_slice(b"iCCP");
    chunk.extend_from_slice(&data);
    let crc = crc32fast::hash(&chunk[4..]);
    chunk.extend_from_slice(&crc.to_be_bytes());
    bytes.splice(insert_at..insert_at, chunk);
}

pub fn extract_icc(path: &Path, format: ImageFormat) -> Option<Vec<u8>> {
    match format {
        ImageFormat::Jpeg => {
            let file = File::open(path).ok()?;
            let mut data = Vec::new();
            file.take(128 * 1024 * 1024).read_to_end(&mut data).ok()?;
            jpeg_icc(&data)
        }
        ImageFormat::Png => {
            let decoder = png::Decoder::new(BufReader::new(File::open(path).ok()?));
            let reader = decoder.read_info().ok()?;
            reader
                .info()
                .icc_profile
                .as_ref()
                .map(|profile| profile.to_vec())
        }
        ImageFormat::WebP => {
            let file = File::open(path).ok()?;
            let mut data = Vec::new();
            file.take(128 * 1024 * 1024).read_to_end(&mut data).ok()?;
            webp_icc(&data)
        }
    }
}

fn jpeg_icc(data: &[u8]) -> Option<Vec<u8>> {
    let mut index = 2usize;
    let mut chunks: Vec<(u8, Vec<u8>)> = Vec::new();
    while index + 4 <= data.len() {
        if data[index] != 0xFF {
            index += 1;
            continue;
        }
        let marker = data[index + 1];
        if marker == 0xD9 || marker == 0xDA {
            break;
        }
        if marker == 0x01 || (0xD0..=0xD7).contains(&marker) {
            index += 2;
            continue;
        }
        let length = u16::from_be_bytes([data[index + 2], data[index + 3]]) as usize;
        if length < 2 || index + 2 + length > data.len() {
            break;
        }
        let segment = &data[index + 4..index + 2 + length];
        if marker == 0xE2 && segment.len() >= 14 && &segment[..12] == b"ICC_PROFILE\0" {
            chunks.push((segment[12], segment[14..].to_vec()));
        }
        index += 2 + length;
    }
    if chunks.is_empty() {
        return None;
    }
    chunks.sort_by_key(|(sequence, _)| *sequence);
    Some(
        chunks
            .into_iter()
            .flat_map(|(_, bytes)| bytes)
            .collect::<Vec<u8>>(),
    )
}

fn webp_icc(data: &[u8]) -> Option<Vec<u8>> {
    if data.len() < 12 || &data[0..4] != b"RIFF" || &data[8..12] != b"WEBP" {
        return None;
    }
    let mut index = 12usize;
    while index + 8 <= data.len() {
        let fourcc = &data[index..index + 4];
        let size = u32::from_le_bytes([
            data[index + 4],
            data[index + 5],
            data[index + 6],
            data[index + 7],
        ]) as usize;
        if index + 8 + size > data.len() {
            break;
        }
        if fourcc == b"ICCP" {
            return Some(data[index + 8..index + 8 + size].to_vec());
        }
        index += 8 + size + (size & 1);
    }
    None
}

/// Comprueba que el resultado se puede volver a decodificar y que conserva un formato válido.
pub fn validate(bytes: &[u8], expected: ImageFormat) -> Result<(u32, u32), CoreError> {
    if format::sniff(bytes) != Some(expected) {
        return Err(CoreError::Encode(
            "El archivo generado no tiene el formato esperado".into(),
        ));
    }
    let image = image::load_from_memory(bytes)
        .map_err(|error| CoreError::Encode(format!("Verificación fallida: {}", error)))?;
    Ok(image.dimensions())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    fn sample() -> RgbaImage {
        let mut image = RgbaImage::new(32, 24);
        for (x, y, pixel) in image.enumerate_pixels_mut() {
            *pixel = Rgba([(x * 8) as u8, (y * 10) as u8, 128, 255]);
        }
        image
    }

    #[test]
    fn jpeg_roundtrip() {
        let bytes = encode_jpeg(&sample(), 80, [255, 255, 255], None).unwrap();
        assert_eq!(format::sniff(&bytes), Some(ImageFormat::Jpeg));
        assert_eq!(validate(&bytes, ImageFormat::Jpeg).unwrap(), (32, 24));
    }

    #[test]
    fn png_roundtrip_is_valid() {
        let bytes = encode_png(&sample(), 2, None).unwrap();
        assert_eq!(format::sniff(&bytes), Some(ImageFormat::Png));
        assert_eq!(validate(&bytes, ImageFormat::Png).unwrap(), (32, 24));
    }

    #[test]
    fn webp_roundtrip() {
        let bytes = encode_webp(&sample(), 80, false).unwrap();
        assert_eq!(format::sniff(&bytes), Some(ImageFormat::WebP));
        assert_eq!(validate(&bytes, ImageFormat::WebP).unwrap(), (32, 24));
    }

    #[test]
    fn jpeg_icc_roundtrip() {
        let icc = vec![7u8; 700];
        let mut bytes = encode_jpeg(&sample(), 80, [255, 255, 255], None).unwrap();
        insert_jpeg_icc(&mut bytes, &icc);
        assert_eq!(jpeg_icc(&bytes), Some(icc));
        validate(&bytes, ImageFormat::Jpeg).unwrap();
    }

    #[test]
    fn icc_survives_png_injection() {
        let icc = vec![0x42; 300];
        let mut bytes = encode_png(&sample(), 0, None).unwrap();
        inject_png_icc(&mut bytes, &icc);
        assert!(bytes.windows(4).any(|window| window == b"iCCP"));
        // El PNG debe seguir siendo decodificable.
        validate(&bytes, ImageFormat::Png).unwrap();
    }
}
