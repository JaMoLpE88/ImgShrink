use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::discovery::SourceItem;
use crate::format::ImageFormat;
use crate::options::OptimizeOptions;
use crate::output::Destination;
use crate::pathutil::{normalize_key, unique_path};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictPolicy {
    Ask,
    Skip,
    Overwrite,
    Rename,
}

impl ConflictPolicy {
    pub const ALL: [ConflictPolicy; 4] = [
        ConflictPolicy::Ask,
        ConflictPolicy::Skip,
        ConflictPolicy::Overwrite,
        ConflictPolicy::Rename,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ConflictPolicy::Ask => "Preguntar",
            ConflictPolicy::Skip => "Omitir existentes",
            ConflictPolicy::Overwrite => "Sobrescribir existentes",
            ConflictPolicy::Rename => "Renombrar con -1, -2...",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SkipReason {
    Unreadable,
    Animated,
    OutputExists,
    DuplicateOutput,
    InPlaceFormatChange,
    NoImprovement,
}

impl SkipReason {
    pub fn label(self) -> &'static str {
        match self {
            SkipReason::Unreadable => "No se pudo leer la cabecera",
            SkipReason::Animated => "Imagen animada (no soportada en v1)",
            SkipReason::OutputExists => "Ya existe un archivo en el destino",
            SkipReason::DuplicateOutput => "Dos entradas generan la misma salida",
            SkipReason::InPlaceFormatChange => "Cambio de formato no permitido al sobrescribir",
            SkipReason::NoImprovement => "El resultado no era menor; se conserva el original",
        }
    }
}

#[derive(Clone, Debug)]
pub struct PlanItem {
    pub source: SourceItem,
    pub output: PathBuf,
    pub skip: Option<SkipReason>,
}

#[derive(Clone, Debug)]
pub struct Conflict {
    pub output: PathBuf,
    pub sources: Vec<PathBuf>,
    pub reason: String,
}

#[derive(Clone, Debug)]
pub struct OperationPlan {
    pub items: Vec<PlanItem>,
    pub destination: Destination,
    pub conflicts: Vec<Conflict>,
    pub warnings: Vec<String>,
    pub total_input_bytes: u64,
    pub processable: usize,
}

impl OperationPlan {
    pub fn skipped_at_plan(&self) -> usize {
        self.items.iter().filter(|item| item.skip.is_some()).count()
    }
}

pub fn output_path(item: &SourceItem, destination: &Destination, format: ImageFormat) -> PathBuf {
    match destination {
        Destination::Separate { root } => {
            root.join(&item.relative).with_extension(format.extension())
        }
        Destination::Suffix { suffix } => {
            let parent = item.path.parent().unwrap_or_else(|| Path::new(""));
            let stem = item
                .path
                .file_stem()
                .map(|stem| stem.to_string_lossy().to_string())
                .unwrap_or_else(|| "imagen".into());
            parent.join(format!("{}{}.{}", stem, suffix, format.extension()))
        }
        Destination::InPlace => {
            if item.format == format {
                item.path.clone()
            } else {
                item.path.with_extension(format.extension())
            }
        }
    }
}

pub fn build_plan(
    items: Vec<SourceItem>,
    options: &OptimizeOptions,
    destination: Destination,
    policy: ConflictPolicy,
) -> OperationPlan {
    let mut warnings = Vec::new();
    let mut conflicts = Vec::new();
    let mut seen_outputs: HashMap<String, usize> = HashMap::new();
    let mut plan_items: Vec<PlanItem> = Vec::with_capacity(items.len());
    let mut total_input_bytes = 0u64;

    for source in items {
        total_input_bytes = total_input_bytes.saturating_add(source.size);
        let format = options.effective_format(source.format);
        let in_place = matches!(destination, Destination::InPlace);
        let mut skip = None;

        if source.probe.animated {
            skip = Some(SkipReason::Animated);
        } else if source.probe.error.is_some() {
            skip = Some(SkipReason::Unreadable);
        } else if in_place && format != source.format {
            skip = Some(SkipReason::InPlaceFormatChange);
        }

        let mut output = output_path(&source, &destination, format);
        if output.exists() && !in_place {
            match policy {
                ConflictPolicy::Overwrite => {}
                ConflictPolicy::Rename => output = unique_path(&output),
                ConflictPolicy::Skip | ConflictPolicy::Ask => {
                    if skip.is_none() {
                        skip = Some(SkipReason::OutputExists);
                    }
                    conflicts.push(Conflict {
                        output: output.clone(),
                        sources: vec![source.path.clone()],
                        reason: "El archivo de destino ya existe".into(),
                    });
                }
            }
        }

        let key = normalize_key(&output);
        if let Some(previous) = seen_outputs.get(&key).copied() {
            if skip.is_none() {
                skip = Some(SkipReason::DuplicateOutput);
            }
            if let Some(previous_item) = plan_items.get_mut(previous) {
                if previous_item.skip.is_none() {
                    previous_item.skip = Some(SkipReason::DuplicateOutput);
                }
                conflicts.push(Conflict {
                    output: output.clone(),
                    sources: vec![previous_item.source.path.clone(), source.path.clone()],
                    reason: "Dos entradas generan la misma ruta de salida".into(),
                });
            }
        } else {
            seen_outputs.insert(key, plan_items.len());
        }

        if format == ImageFormat::Jpeg && source.probe.has_alpha && !source.probe.animated {
            warnings.push(format!(
                "«{}» tiene transparencia y se aplanará sobre fondo RGB al guardar JPEG.",
                source.path.display()
            ));
        }
        if source.probe.color.contains("16") {
            warnings.push(format!(
                "«{}» usa 16 bits por canal; se convertirá a 8 bits.",
                source.path.display()
            ));
        }
        if format == ImageFormat::WebP && source.probe.has_icc {
            warnings.push(format!(
                "«{}» tiene perfil ICC; la v1 no lo incrusta en WebP.",
                source.path.display()
            ));
        }
        if source.size > 0 && source.size < 3 * 1024 {
            warnings.push(format!(
                "«{}» es muy pequeño; probablemente no mejore y se omitirá si crece.",
                source.path.display()
            ));
        }

        plan_items.push(PlanItem {
            source,
            output,
            skip,
        });
    }

    let processable = plan_items.iter().filter(|item| item.skip.is_none()).count();
    OperationPlan {
        items: plan_items,
        destination,
        conflicts,
        warnings,
        total_input_bytes,
        processable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe::Probe;

    fn item(name: &str, format: ImageFormat, size: u64) -> SourceItem {
        SourceItem {
            id: 0,
            path: PathBuf::from(name),
            relative: PathBuf::from(name),
            size,
            modified: None,
            format,
            probe: Probe {
                format: Some(format),
                width: 100,
                height: 100,
                ..Default::default()
            },
        }
    }

    #[test]
    fn opts_skip_unreadable_and_animated() {
        let mut broken = item("broken.jpg", ImageFormat::Jpeg, 10);
        broken.probe.error = Some("bad".into());
        let mut animated = item("anim.webp", ImageFormat::WebP, 10);
        animated.probe.animated = true;
        let plan = build_plan(
            vec![broken, animated],
            &OptimizeOptions::default(),
            Destination::Separate {
                root: PathBuf::from("out"),
            },
            ConflictPolicy::Skip,
        );
        assert_eq!(plan.processable, 0);
        assert_eq!(plan.skipped_at_plan(), 2);
    }

    #[test]
    fn in_place_format_change_is_skipped() {
        let plan = build_plan(
            vec![item("a.png", ImageFormat::Png, 100)],
            &OptimizeOptions {
                output_format: crate::options::OutputFormat::Jpeg,
                ..OptimizeOptions::default()
            },
            Destination::InPlace,
            ConflictPolicy::Ask,
        );
        assert_eq!(plan.items[0].skip, Some(SkipReason::InPlaceFormatChange));
    }
}
