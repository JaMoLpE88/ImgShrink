use std::path::Path;

use imgshrink_core::discovery::{discover, InputSelection};
use imgshrink_core::jobs::{run_blocking, JobOptions};
use imgshrink_core::options::{OptimizeOptions, OutputFormat, Purpose};
use imgshrink_core::output::Destination;
use imgshrink_core::plan::{build_plan, ConflictPolicy, SkipReason};
use imgshrink_core::report;

fn write_png(path: &Path, width: u32, height: u32) {
    let mut image = image::RgbaImage::new(width, height);
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        *pixel = image::Rgba([
            ((x * 7) % 255) as u8,
            ((y * 11) % 255) as u8,
            ((x + y) % 255) as u8,
            255,
        ]);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    image.save(path).unwrap();
}

fn options() -> OptimizeOptions {
    OptimizeOptions {
        output_format: OutputFormat::Auto,
        quality: 80,
        keep_if_larger: false,
        ..OptimizeOptions::for_purpose(Purpose::Custom)
    }
}

#[test]
fn separate_mode_mirrors_structure_and_reports() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("entrada");
    let output = dir.path().join("salida");
    write_png(&input.join("a.png"), 120, 80);
    write_png(&input.join("sub").join("b.png"), 64, 64);

    let discovered = discover(&[InputSelection::directory(input, true)], Some(&output));
    assert_eq!(discovered.items.len(), 2);
    let plan = build_plan(
        discovered.items,
        &options(),
        Destination::Separate {
            root: output.clone(),
        },
        ConflictPolicy::Skip,
    );
    let (summary, results) = run_blocking(
        &plan,
        &options(),
        &JobOptions {
            workers: 2,
            conflict_policy: ConflictPolicy::Skip,
            journal: None,
        },
    );
    assert_eq!(summary.succeeded, 2);
    assert!(output.join("a.png").exists());
    assert!(output.join("sub").join("b.png").exists());
    let report_path = output.join("informe.json");
    report::write(&report_path, &plan, &options(), &summary, &results).unwrap();
    let report: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(report_path).unwrap()).unwrap();
    assert_eq!(report["schema"], 1);
    assert_eq!(report["summary"]["succeeded"], 2);
}

#[test]
fn suffix_mode_uses_configured_suffix() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("foto.png");
    write_png(&input, 80, 60);
    let discovered = discover(&[InputSelection::file(input.clone())], None);
    let plan = build_plan(
        discovered.items,
        &options(),
        Destination::Suffix {
            suffix: "-mini".into(),
        },
        ConflictPolicy::Skip,
    );
    let (summary, _) = run_blocking(
        &plan,
        &options(),
        &JobOptions {
            workers: 1,
            conflict_policy: ConflictPolicy::Skip,
            journal: None,
        },
    );
    assert_eq!(summary.succeeded, 1);
    assert!(dir.path().join("foto-mini.png").exists());
    assert!(input.exists());
}

#[test]
fn in_place_replaces_only_after_success() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("original.png");
    write_png(&path, 100, 70);
    let original = std::fs::read(&path).unwrap();

    let discovered = discover(&[InputSelection::file(path.clone())], None);
    let plan = build_plan(
        discovered.items,
        &options(),
        Destination::InPlace,
        ConflictPolicy::Overwrite,
    );
    let (summary, results) = run_blocking(
        &plan,
        &options(),
        &JobOptions {
            workers: 1,
            conflict_policy: ConflictPolicy::Overwrite,
            journal: None,
        },
    );
    assert_eq!(summary.succeeded, 1, "{results:?}");
    let replaced = std::fs::read(&path).unwrap();
    assert_ne!(replaced, original, "el archivo debe haberse reescrito");
    image::open(&path).expect("el original sustituido debe seguir siendo una imagen válida");
}

#[test]
fn existing_destination_is_skipped_by_policy() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("a.png");
    let output = dir.path().join("salida");
    write_png(&input, 60, 60);
    std::fs::create_dir_all(&output).unwrap();
    std::fs::write(output.join("a.png"), b"ocupado").unwrap();

    let discovered = discover(&[InputSelection::file(input)], Some(&output));
    let plan = build_plan(
        discovered.items,
        &options(),
        Destination::Separate {
            root: output.clone(),
        },
        ConflictPolicy::Skip,
    );
    assert_eq!(plan.processable, 0);
    assert_eq!(plan.items[0].skip, Some(SkipReason::OutputExists));
    assert_eq!(std::fs::read(output.join("a.png")).unwrap(), b"ocupado");
}

#[test]
fn rename_policy_avoids_overwriting() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("a.png");
    let output = dir.path().join("salida");
    write_png(&input, 60, 60);
    std::fs::create_dir_all(&output).unwrap();
    std::fs::write(output.join("a.png"), b"ocupado").unwrap();

    let discovered = discover(&[InputSelection::file(input)], Some(&output));
    let plan = build_plan(
        discovered.items,
        &options(),
        Destination::Separate {
            root: output.clone(),
        },
        ConflictPolicy::Rename,
    );
    let (summary, _) = run_blocking(
        &plan,
        &options(),
        &JobOptions {
            workers: 1,
            conflict_policy: ConflictPolicy::Rename,
            journal: None,
        },
    );
    assert_eq!(summary.succeeded, 1);
    assert_eq!(std::fs::read(output.join("a.png")).unwrap(), b"ocupado");
    assert!(output.join("a-1.png").exists());
}

#[test]
fn failed_item_does_not_touch_original() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("danada.png");
    std::fs::write(&path, b"\x89PNG\r\n\x1a\n datos incompletos").unwrap();
    let original = std::fs::read(&path).unwrap();
    let discovered = discover(&[InputSelection::file(path.clone())], None);
    let plan = build_plan(
        discovered.items,
        &options(),
        Destination::InPlace,
        ConflictPolicy::Overwrite,
    );
    assert_eq!(plan.processable, 0);
    let (summary, _) = run_blocking(
        &plan,
        &options(),
        &JobOptions {
            workers: 1,
            conflict_policy: ConflictPolicy::Overwrite,
            journal: None,
        },
    );
    assert_eq!(summary.succeeded, 0);
    assert_eq!(std::fs::read(&path).unwrap(), original);
}
