use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::discovery::SourceItem;
use crate::engine::validate;
use crate::error::CoreError;
use crate::format::ImageFormat;
use crate::journal::Journal;
use crate::pathutil::{display_path, unique_path};
use crate::plan::ConflictPolicy;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Destination {
    Separate { root: PathBuf },
    InPlace,
    Suffix { suffix: String },
}

impl Destination {
    pub fn label(&self) -> &'static str {
        match self {
            Destination::Separate { .. } => "Carpeta separada",
            Destination::InPlace => "Sobrescribir originales",
            Destination::Suffix { .. } => "Junto al original con sufijo",
        }
    }

    pub fn describe(&self) -> String {
        match self {
            Destination::Separate { root } => format!("Carpeta de salida: {}", display_path(root)),
            Destination::InPlace => "Se reemplazarán los archivos originales".into(),
            Destination::Suffix { suffix } => {
                format!("Se guardará junto al original con el sufijo «{}»", suffix)
            }
        }
    }
}

pub fn write_item(
    item: &SourceItem,
    output: &Path,
    bytes: &[u8],
    format: ImageFormat,
    in_place: bool,
    policy: ConflictPolicy,
    journal: Option<&Journal>,
) -> Result<PathBuf, CoreError> {
    validate(bytes, format)?;

    if in_place {
        let metadata = fs::metadata(&item.path).map_err(|source| CoreError::Read {
            path: item.path.clone(),
            source,
        })?;
        if metadata.len() != item.size || metadata.modified().ok() != item.modified {
            return Err(CoreError::SourceChanged);
        }
    }

    let mut target = output.to_path_buf();
    if target.exists() && !in_place {
        match policy {
            ConflictPolicy::Skip | ConflictPolicy::Ask => {
                return Err(CoreError::Exists(target));
            }
            ConflictPolicy::Overwrite => {}
            ConflictPolicy::Rename => target = unique_path(&target),
        }
    }

    let parent = target.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|source| CoreError::Write {
        path: parent.to_path_buf(),
        source,
    })?;

    let mut temp = tempfile::Builder::new()
        .prefix(".imgshrink-")
        .suffix(".tmp")
        .tempfile_in(parent)
        .map_err(|source| CoreError::Write {
            path: parent.to_path_buf(),
            source,
        })?;
    temp.write_all(bytes).map_err(|source| CoreError::Write {
        path: temp.path().to_path_buf(),
        source,
    })?;
    temp.flush().map_err(|source| CoreError::Write {
        path: temp.path().to_path_buf(),
        source,
    })?;
    temp.as_file()
        .sync_all()
        .map_err(|source| CoreError::Write {
            path: temp.path().to_path_buf(),
            source,
        })?;

    if in_place {
        if let Some(journal) = journal {
            journal
                .begin(&item.path, temp.path())
                .map_err(|source| CoreError::Write {
                    path: journal.path().to_path_buf(),
                    source,
                })?;
        }
    }

    let persisted = temp.persist(&target).map_err(|error| CoreError::Write {
        path: target.clone(),
        source: error.error,
    });

    if in_place {
        if let Some(journal) = journal {
            let _ = journal.finish(&item.path);
        }
    }

    persisted?;
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe::Probe;
    use image::Rgba;

    fn source(path: &Path, bytes: usize) -> SourceItem {
        SourceItem {
            id: 0,
            path: path.to_path_buf(),
            relative: path.file_name().unwrap().into(),
            size: bytes as u64,
            modified: std::fs::metadata(path).ok().and_then(|m| m.modified().ok()),
            format: ImageFormat::Png,
            probe: Probe::default(),
        }
    }

    fn png_bytes() -> Vec<u8> {
        let mut image = image::RgbaImage::new(4, 4);
        for pixel in image.pixels_mut() {
            *pixel = Rgba([255, 0, 0, 255]);
        }
        let mut out = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut out, 4, 4);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(image.as_raw())
                .unwrap();
        }
        out
    }

    #[test]
    fn writes_and_does_not_clobber_by_default() {
        let dir = tempfile::tempdir().unwrap();
        let source_path = dir.path().join("source.png");
        let bytes = png_bytes();
        std::fs::write(&source_path, &bytes).unwrap();
        let item = source(&source_path, bytes.len());
        let output = dir.path().join("out.png");

        write_item(
            &item,
            &output,
            &bytes,
            ImageFormat::Png,
            false,
            ConflictPolicy::Skip,
            None,
        )
        .unwrap();
        assert!(output.exists());
        assert!(write_item(
            &item,
            &output,
            &bytes,
            ImageFormat::Png,
            false,
            ConflictPolicy::Skip,
            None,
        )
        .is_err());
    }

    #[test]
    fn failed_validation_keeps_original_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("original.png");
        let bytes = png_bytes();
        std::fs::write(&path, &bytes).unwrap();
        let item = source(&path, bytes.len());
        let result = write_item(
            &item,
            &path,
            b"not a png",
            ImageFormat::Png,
            true,
            ConflictPolicy::Overwrite,
            None,
        );
        assert!(result.is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
}
