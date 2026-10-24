use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::error::CoreError;
use crate::jobs::ItemResult;
use crate::options::OptimizeOptions;
use crate::output::Destination;
use crate::pathutil::display_path;
use crate::plan::OperationPlan;
use crate::stats::Summary;

#[derive(Serialize)]
pub struct Report {
    pub schema: u32,
    pub app: &'static str,
    pub version: &'static str,
    pub generated_unix: u64,
    pub destination: Destination,
    pub options: OptimizeOptions,
    pub summary: Summary,
    pub items: Vec<ReportItem>,
}

#[derive(Serialize)]
pub struct ReportItem {
    pub source: String,
    pub output: Option<String>,
    pub status: &'static str,
    pub code: Option<String>,
    pub message: Option<String>,
    pub input_bytes: u64,
    pub output_bytes: Option<u64>,
    pub duration_ms: Option<u64>,
}

pub fn build(
    plan: &OperationPlan,
    options: &OptimizeOptions,
    summary: &Summary,
    results: &[(usize, ItemResult)],
) -> Report {
    let mut items = Vec::with_capacity(plan.items.len());
    for plan_item in &plan.items {
        let result = results
            .iter()
            .find(|(id, _)| *id == plan_item.source.id)
            .map(|(_, result)| result);
        let (status, output, code, message, output_bytes, duration) = match result {
            Some(ItemResult::Done {
                output,
                output_bytes,
                duration,
                ..
            }) => (
                "ok",
                Some(display_path(output)),
                None,
                None,
                Some(*output_bytes),
                Some(duration.as_millis() as u64),
            ),
            Some(ItemResult::Skipped { reason }) => (
                "skipped",
                None,
                None,
                Some(reason.label().to_string()),
                None,
                None,
            ),
            Some(ItemResult::Failed { code, message }) => (
                "failed",
                None,
                Some(code.clone()),
                Some(message.clone()),
                None,
                None,
            ),
            Some(ItemResult::Cancelled) => (
                "cancelled",
                None,
                None,
                Some("Operación cancelada".into()),
                None,
                None,
            ),
            None => (
                "skipped",
                None,
                None,
                plan_item.skip.map(|reason| reason.label().to_string()),
                None,
                None,
            ),
        };
        items.push(ReportItem {
            source: display_path(&plan_item.source.path),
            output,
            status,
            code,
            message,
            input_bytes: plan_item.source.size,
            output_bytes,
            duration_ms: duration,
        });
    }
    Report {
        schema: 1,
        app: "ImgShrink",
        version: env!("CARGO_PKG_VERSION"),
        generated_unix: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0),
        destination: plan.destination.clone(),
        options: options.clone(),
        summary: summary.clone(),
        items,
    }
}

pub fn write(
    path: &Path,
    plan: &OperationPlan,
    options: &OptimizeOptions,
    summary: &Summary,
    results: &[(usize, ItemResult)],
) -> Result<(), CoreError> {
    let report = build(plan, options, summary, results);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| CoreError::Write {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let text = serde_json::to_string_pretty(&report)
        .map_err(|error| CoreError::Other(error.to_string()))?;
    fs::write(path, text).map_err(|source| CoreError::Write {
        path: path.to_path_buf(),
        source,
    })
}
