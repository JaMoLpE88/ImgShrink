use std::path::PathBuf;

use imgshrink_core::config::Config;
use imgshrink_core::discovery::SourceItem;
use imgshrink_core::format::ImageFormat;
use imgshrink_core::options::{OptimizeOptions, Purpose};
use imgshrink_core::output::Destination;
use imgshrink_core::plan::{build_plan, ConflictPolicy};
use imgshrink_core::probe::Probe;
use imgshrink_core::stats::Summary;
use imgshrink_tui::app::{App, Overlay, Screen};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn render(app: &mut App, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| imgshrink_tui::ui::draw(frame, app))
        .unwrap();
    let buffer = terminal.backend().buffer();
    let mut text = String::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            text.push_str(buffer.cell((x, y)).map(|cell| cell.symbol()).unwrap_or(" "));
        }
        text.push('\n');
    }
    text
}

fn sample_app() -> App {
    App::new(Config::default(), true, false, true)
}

fn fake_item() -> SourceItem {
    SourceItem {
        id: 0,
        path: PathBuf::from("fotos/vacaciones.png"),
        relative: PathBuf::from("vacaciones.png"),
        size: 845_123,
        modified: None,
        format: ImageFormat::Png,
        probe: Probe {
            format: Some(ImageFormat::Png),
            width: 1600,
            height: 1200,
            color: "Rgba8".into(),
            has_alpha: true,
            ..Default::default()
        },
    }
}

fn with_plan(app: &mut App) {
    let item = fake_item();
    app.discovered = vec![item.clone()];
    let plan = build_plan(
        vec![item],
        &OptimizeOptions::for_purpose(Purpose::Share),
        Destination::Separate {
            root: PathBuf::from("salida"),
        },
        ConflictPolicy::Skip,
    );
    app.plan = Some(plan);
}

#[test]
fn renders_every_screen_at_standard_sizes() {
    let mut app = sample_app();
    app.screen = Screen::Welcome;
    let welcome = render(&mut app, 100, 30);
    assert!(welcome.contains("I M G S H R I N K"));
    assert!(welcome.lines().count() == 30);

    app.screen = Screen::Purpose;
    let purpose = render(&mut app, 100, 30);
    assert!(purpose.contains("Compartir"));
    assert!(purpose.contains("Personalizado"));

    app.screen = Screen::Source;
    let source = render(&mut app, 100, 30);
    assert!(source.contains("Explorar"));
    assert!(source.contains("Selección"));

    app.screen = Screen::Analysis;
    let analysis = render(&mut app, 100, 30);
    assert!(analysis.contains("Analizando"));

    with_plan(&mut app);
    app.screen = Screen::Analysis;
    let ready = render(&mut app, 100, 30);
    assert!(ready.contains("Resumen del análisis"));

    app.screen = Screen::Destination;
    let destination = render(&mut app, 100, 30);
    assert!(destination.contains("Carpeta separada"));
    assert!(destination.contains("Sobrescribir"));

    app.screen = Screen::Review;
    let review = render(&mut app, 100, 30);
    assert!(review.contains("Revisión de la operación"));

    app.summary = Some(Summary {
        found: 12,
        succeeded: 10,
        skipped: 2,
        input_bytes: 10_000_000,
        output_bytes: 6_000_000,
        elapsed_secs: 12.5,
        ..Default::default()
    });
    app.screen = Screen::Summary;
    let summary = render(&mut app, 100, 30);
    assert!(summary.contains("Ahorro"));
}

#[test]
fn renders_help_and_confirmation() {
    let mut app = sample_app();
    app.overlay = Some(Overlay::Help { scroll: 0 });
    let help = render(&mut app, 100, 30);
    assert!(help.contains("Navegación"));
    assert!(help.contains("Privacidad"));

    app.overlay = Some(Overlay::ConfirmOverwrite {
        stage: 2,
        choice: 1,
        typed: "SOBRE".into(),
        cursor: 5,
        error: Some("Escribe SOBRESCRIBIR".into()),
    });
    let confirm = render(&mut app, 100, 30);
    assert!(confirm.contains("SOBRESCRIBIR"));
}

#[test]
fn tiny_terminal_shows_warning_without_panicking() {
    let mut app = sample_app();
    let text = render(&mut app, 40, 12);
    assert!(text.contains("demasiado pequeña"));
    assert!(text.contains("40"));
}

#[test]
fn compact_sizes_do_not_panic() {
    let mut app = sample_app();
    with_plan(&mut app);
    for (width, height) in [(60, 20), (80, 24), (50, 14), (120, 40)] {
        for screen in [
            Screen::Welcome,
            Screen::Purpose,
            Screen::Source,
            Screen::Analysis,
            Screen::Destination,
            Screen::Review,
            Screen::Summary,
        ] {
            app.screen = screen;
            let text = render(&mut app, width, height);
            assert!(!text.is_empty());
            assert_eq!(text.lines().count(), height as usize);
        }
    }
}

#[test]
fn prints_a_reference_frame_for_visual_review() {
    if std::env::var("IMGSHRINK_DUMP").is_err() {
        return;
    }
    let mut app = sample_app();
    with_plan(&mut app);
    app.screen = Screen::Review;
    println!("=== REVIEW 100x30 ===\n{}", render(&mut app, 100, 30));
    app.screen = Screen::Welcome;
    println!("=== WELCOME 60x20 ===\n{}", render(&mut app, 60, 20));
    app.screen = Screen::Purpose;
    println!("=== PURPOSE 60x20 ===\n{}", render(&mut app, 60, 20));
    app.screen = Screen::Destination;
    println!("=== DESTINATION 60x20 ===\n{}", render(&mut app, 60, 20));
}
