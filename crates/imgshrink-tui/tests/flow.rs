use std::time::{Duration, Instant};

use crossterm::event::KeyCode;
use imgshrink_core::config::Config;
use imgshrink_tui::app::{App, Screen};

#[test]
fn full_flow_from_selection_to_summary() {
    let dir = tempfile::tempdir().unwrap();
    let mut image = image::RgbaImage::new(96, 64);
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        *pixel = image::Rgba([(x * 2) as u8, (y * 3) as u8, 90, 255]);
    }
    image.save(dir.path().join("foto.png")).unwrap();

    let config = Config {
        workers: 2,
        ..Config::default()
    };
    let mut app = App::new(config, false, false, false);
    app.options.keep_if_larger = false;
    app.source.cwd = dir.path().to_path_buf();
    app.source.refresh();
    app.screen = Screen::Source;
    assert_eq!(app.source.entries.len(), 1, "debe ver la imagen de prueba");

    app.test_key(KeyCode::Char(' '));
    assert_eq!(app.source.selected_files.len(), 1);
    app.test_key(KeyCode::Char('p'));
    assert_eq!(app.screen, Screen::Analysis);

    let start = Instant::now();
    while app.plan.is_none() && start.elapsed() < Duration::from_secs(10) {
        app.tick();
        std::thread::sleep(Duration::from_millis(10));
    }
    let plan = app.plan.as_ref().expect("el análisis debe terminar");
    assert_eq!(plan.processable, 1);

    app.test_key(KeyCode::Enter);
    assert_eq!(app.screen, Screen::Destination);
    app.test_key(KeyCode::Enter);
    assert_eq!(app.screen, Screen::Review);
    app.test_key(KeyCode::Enter);
    assert_eq!(app.screen, Screen::Running);

    let start = Instant::now();
    while app.screen != Screen::Summary && start.elapsed() < Duration::from_secs(20) {
        app.tick();
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(app.screen, Screen::Summary, "debe llegar al resumen");
    let summary = app.summary.as_ref().expect("debe haber resumen");
    assert_eq!(summary.succeeded, 1);
    assert_eq!(summary.failed, 0);
    assert!(dir
        .path()
        .join("imgshrink-output")
        .join("foto.png")
        .exists());
    assert!(app.report_path.as_ref().is_some_and(|path| path.exists()));
}

#[test]
fn options_overlay_changes_and_discards_format() {
    use std::path::PathBuf;

    use imgshrink_core::discovery::SourceItem;
    use imgshrink_core::format::ImageFormat;
    use imgshrink_core::options::{OptimizeOptions, Purpose};
    use imgshrink_core::output::Destination;
    use imgshrink_core::plan::{build_plan, ConflictPolicy};
    use imgshrink_core::probe::Probe;
    use imgshrink_tui::app::Overlay;

    let item = SourceItem {
        id: 0,
        path: PathBuf::from("foto.png"),
        relative: PathBuf::from("foto.png"),
        size: 1000,
        modified: None,
        format: ImageFormat::Png,
        probe: Probe {
            format: Some(ImageFormat::Png),
            width: 100,
            height: 100,
            ..Default::default()
        },
    };
    let mut app = App::new(Config::default(), false, false, false);
    app.discovered = vec![item.clone()];
    app.plan = Some(build_plan(
        vec![item],
        &OptimizeOptions::for_purpose(Purpose::Share),
        Destination::Separate {
            root: PathBuf::from("salida"),
        },
        ConflictPolicy::Skip,
    ));
    app.screen = Screen::Review;

    app.test_key(KeyCode::Char('o'));
    assert!(matches!(app.overlay, Some(Overlay::Options { .. })));
    app.test_key(KeyCode::Right);
    assert!(matches!(
        app.options.output_format,
        imgshrink_core::OutputFormat::Jpeg
    ));
    app.test_key(KeyCode::Esc);
    assert!(app.overlay.is_none());
    assert!(matches!(
        app.options.output_format,
        imgshrink_core::OutputFormat::Auto
    ));
}

#[test]
fn paste_goes_into_text_inputs() {
    use imgshrink_tui::app::{InputTarget, Overlay};

    let mut app = App::new(Config::default(), false, false, false);
    app.overlay = Some(Overlay::TextInput {
        target: InputTarget::Suffix,
        value: String::new(),
        cursor: 0,
        error: None,
    });
    app.test_paste("-mini");
    match &app.overlay {
        Some(Overlay::TextInput { value, cursor, .. }) => {
            assert_eq!(value, "-mini");
            assert_eq!(*cursor, 5);
        }
        other => panic!("overlay inesperado: {other:?}"),
    }
}
