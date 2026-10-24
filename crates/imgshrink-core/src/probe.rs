use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use image::ImageDecoder;

use crate::format::{self, ImageFormat};

#[derive(Clone, Debug, Default)]
pub struct Probe {
    pub format: Option<ImageFormat>,
    pub width: u32,
    pub height: u32,
    pub color: String,
    pub has_alpha: bool,
    pub animated: bool,
    pub orientation: u8,
    pub has_icc: bool,
    pub error: Option<String>,
}

impl Probe {
    pub fn is_readable(&self) -> bool {
        self.error.is_none()
    }
}

fn read_head(path: &Path, limit: u64) -> std::io::Result<Vec<u8>> {
    let file = File::open(path)?;
    let mut head = Vec::new();
    file.take(limit).read_to_end(&mut head)?;
    Ok(head)
}

fn exif_orientation(path: &Path) -> u8 {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(_) => return 1,
    };
    let mut reader = BufReader::new(file);
    let exif = match exif::Reader::new().read_from_container(&mut reader) {
        Ok(exif) => exif,
        Err(_) => return 1,
    };
    exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY)
        .and_then(|field| field.value.get_uint(0))
        .map(|value| value.clamp(1, 8) as u8)
        .unwrap_or(1)
}

fn looks_animated(format: ImageFormat, head: &[u8]) -> bool {
    match format {
        ImageFormat::Png => head.windows(4).any(|w| w == b"acTL"),
        ImageFormat::WebP => head.windows(4).any(|w| w == b"ANIM"),
        ImageFormat::Jpeg => false,
    }
}

fn looks_icc(format: ImageFormat, head: &[u8]) -> bool {
    match format {
        ImageFormat::Jpeg => head.windows(12).any(|w| w == b"ICC_PROFILE\0"),
        ImageFormat::Png => head.windows(4).any(|w| w == b"iCCP"),
        ImageFormat::WebP => head.windows(4).any(|w| w == b"ICCP"),
    }
}

pub fn probe(path: &Path) -> Probe {
    let mut result = Probe::default();
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) => {
            result.error = Some(format!("No se pudo leer el archivo: {}", error));
            return result;
        }
    };
    if !metadata.is_file() {
        result.error = Some("La ruta no es un archivo".into());
        return result;
    }

    let head = match read_head(path, 1024 * 1024) {
        Ok(head) => head,
        Err(error) => {
            result.error = Some(format!("No se pudo leer la cabecera: {}", error));
            return result;
        }
    };

    let format = format::sniff(&head).or_else(|| ImageFormat::from_extension(path));
    result.format = format;
    if let Some(fmt) = format {
        result.animated = looks_animated(fmt, &head);
        result.has_icc = looks_icc(fmt, &head);
    }

    let reader = match image::ImageReader::open(path) {
        Ok(reader) => reader,
        Err(error) => {
            result.error = Some(error.to_string());
            return result;
        }
    };
    let reader = match reader.with_guessed_format() {
        Ok(reader) => reader,
        Err(error) => {
            result.error = Some(error.to_string());
            return result;
        }
    };
    match reader.into_decoder() {
        Ok(decoder) => {
            let (width, height) = decoder.dimensions();
            result.width = width;
            result.height = height;
            let color = decoder.color_type();
            result.has_alpha = color.has_alpha();
            result.color = format!("{:?}", color);
            if format == Some(ImageFormat::Jpeg) {
                result.orientation = exif_orientation(path);
            }
        }
        Err(error) => {
            result.error = Some(error.to_string());
        }
    }

    result
}
