use std::collections::HashSet;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::format::{self, ImageFormat};
use crate::pathutil::display_path;
use crate::probe::{self, Probe};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputKind {
    File,
    Directory,
}

#[derive(Clone, Debug)]
pub struct InputSelection {
    pub path: PathBuf,
    pub kind: InputKind,
    pub recursive: bool,
}

impl InputSelection {
    pub fn file(path: PathBuf) -> Self {
        Self {
            path,
            kind: InputKind::File,
            recursive: false,
        }
    }

    pub fn directory(path: PathBuf, recursive: bool) -> Self {
        Self {
            path,
            kind: InputKind::Directory,
            recursive,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SourceItem {
    pub id: usize,
    pub path: PathBuf,
    pub relative: PathBuf,
    pub size: u64,
    pub modified: Option<std::time::SystemTime>,
    pub format: ImageFormat,
    pub probe: Probe,
}

#[derive(Debug, Default)]
pub struct DiscoverResult {
    pub items: Vec<SourceItem>,
    pub warnings: Vec<String>,
    pub skipped: usize,
    pub unreadable: usize,
}

pub fn discover(selections: &[InputSelection], exclude: Option<&Path>) -> DiscoverResult {
    let mut result = DiscoverResult::default();
    let mut seen: HashSet<PathBuf> = HashSet::new();
    let exclude = exclude.and_then(|path| std::fs::canonicalize(path).ok());

    for selection in selections {
        match selection.kind {
            InputKind::File => {
                add_file(
                    &selection.path,
                    Path::new(""),
                    &mut result,
                    &mut seen,
                    exclude.as_deref(),
                );
            }
            InputKind::Directory => {
                let base = &selection.path;
                let max_depth = if selection.recursive { usize::MAX } else { 1 };
                let walker = WalkDir::new(base)
                    .follow_links(false)
                    .max_depth(max_depth)
                    .into_iter();
                for entry in walker {
                    let entry = match entry {
                        Ok(entry) => entry,
                        Err(error) => {
                            result.warnings.push(format!(
                                "No se pudo recorrer «{}»: {}",
                                error
                                    .path()
                                    .map(display_path)
                                    .unwrap_or_else(|| base.display().to_string()),
                                error
                            ));
                            result.unreadable += 1;
                            continue;
                        }
                    };
                    if entry.file_type().is_symlink() {
                        result.warnings.push(format!(
                            "Se omite el enlace simbólico «{}»",
                            display_path(entry.path())
                        ));
                        continue;
                    }
                    if !entry.file_type().is_file() {
                        continue;
                    }
                    let relative = entry
                        .path()
                        .strip_prefix(base)
                        .unwrap_or_else(|_| entry.path())
                        .to_path_buf();
                    add_file(
                        entry.path(),
                        &relative,
                        &mut result,
                        &mut seen,
                        exclude.as_deref(),
                    );
                }
            }
        }
    }

    result.items.sort_by(|a, b| a.path.cmp(&b.path));
    for (index, item) in result.items.iter_mut().enumerate() {
        item.id = index;
    }
    result
}

fn add_file(
    path: &Path,
    relative: &Path,
    result: &mut DiscoverResult,
    seen: &mut HashSet<PathBuf>,
    exclude: Option<&Path>,
) {
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if let Some(exclude) = exclude {
        if canonical.starts_with(exclude) {
            return;
        }
    }
    if !seen.insert(canonical.clone()) {
        return;
    }

    if let Ok(link_metadata) = std::fs::symlink_metadata(path) {
        if link_metadata.file_type().is_symlink() {
            result.warnings.push(format!(
                "Se omite el enlace simbólico «{}» para no procesar su destino",
                display_path(path)
            ));
            return;
        }
    }

    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) => {
            result.warnings.push(format!(
                "No se pudo leer «{}»: {}",
                display_path(path),
                error
            ));
            result.unreadable += 1;
            return;
        }
    };

    let sniffed = format::sniff_file(path).ok().flatten();
    let format = sniffed.or_else(|| ImageFormat::from_extension(path));
    let Some(format) = format else {
        result.skipped += 1;
        return;
    };

    // Si la extensión dice una cosa y el contenido otra, mandan los magic bytes.
    if let Some(sniffed) = sniffed {
        if ImageFormat::from_extension(path).is_some_and(|ext| ext != sniffed) {
            result.warnings.push(format!(
                "«{}» tiene extensión distinta al contenido real ({}); se usará {}.",
                display_path(path),
                ImageFormat::from_extension(path)
                    .map(|format| format.label())
                    .unwrap_or("desconocido"),
                sniffed.label()
            ));
        }
    }

    let probe = probe::probe(path);
    if probe.error.is_some() {
        result.unreadable += 1;
    }

    result.items.push(SourceItem {
        id: 0,
        path: path.to_path_buf(),
        relative: if relative.as_os_str().is_empty() {
            path.file_name()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("imagen"))
        } else {
            relative.to_path_buf()
        },
        size: metadata.len(),
        modified: metadata.modified().ok(),
        format,
        probe,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovers_only_images_and_respects_depth() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let nested = root.join("sub");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(root.join("a.png"), b"not a real png").unwrap();
        std::fs::write(nested.join("b.jpg"), b"not a real jpeg").unwrap();
        std::fs::write(root.join("notes.txt"), b"hello").unwrap();

        let flat = discover(
            &[InputSelection::directory(root.to_path_buf(), false)],
            None,
        );
        assert_eq!(flat.items.len(), 1);
        assert_eq!(flat.items[0].path, root.join("a.png"));

        let deep = discover(&[InputSelection::directory(root.to_path_buf(), true)], None);
        assert_eq!(deep.items.len(), 2);
    }

    #[test]
    fn skips_output_directory_inside_input() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let out = root.join("out");
        std::fs::create_dir_all(&out).unwrap();
        std::fs::write(root.join("a.png"), b"x").unwrap();
        std::fs::write(out.join("b.png"), b"x").unwrap();
        let result = discover(
            &[InputSelection::directory(root.to_path_buf(), true)],
            Some(&out),
        );
        assert_eq!(result.items.len(), 1);
    }
}
