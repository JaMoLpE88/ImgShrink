//! Genera páginas HTML con capturas reales de la TUI para el README.
//! Ejecutar con: cargo run -p imgshrink-tui --example gallery
//! Después, scripts/capture-screenshots.ps1 las convierte en PNG con Chrome.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};

use imgshrink_core::config::Config;
use imgshrink_core::discovery::SourceItem;
use imgshrink_core::format::ImageFormat;
use imgshrink_core::jobs::{ItemResult, Stage};
use imgshrink_core::options::{OptimizeOptions, Purpose};
use imgshrink_core::output::Destination;
use imgshrink_core::plan::{build_plan, ConflictPolicy, OperationPlan};
use imgshrink_core::probe::Probe;
use imgshrink_core::stats::Summary;
use imgshrink_tui::app::{App, BrowseEntry, Overlay, RunningState, Screen};
use ratatui::backend::TestBackend;
use ratatui::style::{Color, Modifier};
use ratatui::Terminal;

const COLS: u16 = 120;
const ROWS: u16 = 34;
const BASE: &str = r"C:\Users\ana\Pictures\Viaje";

fn main() {
    let out = PathBuf::from("docs/images");
    std::fs::create_dir_all(&out).expect("crear docs/images");

    let mut cover = app();
    cover.screen = Screen::Welcome;
    page("cover", &mut cover, &out, "Bienvenida");

    let mut selection = app();
    selection.screen = Screen::Source;
    selection.source.cwd = PathBuf::from(BASE);
    selection.source.recursive = true;
    selection.source.entries = vec![
        dir("Roma"),
        dir("Playa"),
        dir("Familia"),
        file("IMG_1042.jpg", 4_800_000),
        file("IMG_1043.jpg", 5_200_000),
        file("captura.png", 1_100_000),
        file("mapa.webp", 640_000),
        file("panoramica.jpg", 12_400_000),
    ];
    selection.source.index = 5;
    selection.source.selected_files = vec![
        PathBuf::from(BASE).join("IMG_1042.jpg"),
        PathBuf::from(BASE).join("IMG_1043.jpg"),
        PathBuf::from(BASE).join("panoramica.jpg"),
    ];
    page("selection", &mut selection, &out, "Selección");

    let items = sample_items(148);
    let plan = plan_for(&items);
    let mut analysis = app();
    analysis.screen = Screen::Analysis;
    analysis.discovered = items.clone();
    analysis.plan = Some(plan.clone());
    analysis.analysis_skipped = 6;
    analysis.analysis_unreadable = 2;
    page("analysis", &mut analysis, &out, "Análisis");

    let mut running = app();
    running.ascii = true;
    running.screen = Screen::Running;
    running.discovered = items.clone();
    running.plan = Some(plan.clone());
    let (_tx, rx) = crossbeam_channel::unbounded();
    running.running = Some(RunningState {
        rx,
        cancel: Arc::new(AtomicBool::new(false)),
        started: Instant::now() - Duration::from_secs(42),
        summary: Summary {
            found: 148,
            succeeded: 86,
            failed: 2,
            skipped: 4,
            cancelled: 0,
            input_bytes: plan.total_input_bytes,
            output_bytes: 318_400_000,
            elapsed_secs: 42.0,
        },
        current_id: Some(92),
        current_stage: Stage::Encode,
        failures: vec![
            (
                r"C:\Users\ana\Pictures\Viaje\Playa\IMG_1044.jpg".into(),
                "El archivo está dañado o truncado (IS-IMG-002)".into(),
            ),
            (
                r"C:\Users\ana\Pictures\Viaje\Roma\IMG_1061.jpg".into(),
                "No se pudo leer el archivo: acceso denegado".into(),
            ),
        ],
        handle: None,
    });
    page("running", &mut running, &out, "Procesando");

    let mut summary = app();
    summary.screen = Screen::Summary;
    summary.discovered = items.clone();
    summary.plan = Some(plan.clone());
    summary.summary = Some(Summary {
        found: 148,
        succeeded: 142,
        skipped: 4,
        failed: 2,
        cancelled: 0,
        input_bytes: plan.total_input_bytes,
        output_bytes: 318_400_000,
        elapsed_secs: 68.4,
    });
    summary.report_path = Some(PathBuf::from(
        r"C:\Users\ana\Pictures\Viaje-imgshrink\imgshrink-report.json",
    ));
    summary.results = vec![
        (
            0,
            ItemResult::Failed {
                code: "IS-IMG-002".into(),
                message: "Archivo dañado".into(),
            },
        ),
        (
            1,
            ItemResult::Failed {
                code: "IS-IO-001".into(),
                message: "Sin permisos".into(),
            },
        ),
    ];
    page("summary", &mut summary, &out, "Resumen");

    let small = clean_items(12);
    let small_plan = plan_for(&small);
    let mut overwrite = app();
    overwrite.screen = Screen::Destination;
    overwrite.discovered = small;
    overwrite.plan = Some(small_plan);
    overwrite.destination.mode = 1;
    overwrite.overlay = Some(Overlay::ConfirmOverwrite {
        stage: 2,
        choice: 1,
        typed: "SOBRESCRIBIR".into(),
        cursor: 12,
        error: None,
    });
    page("overwrite", &mut overwrite, &out, "Confirmación");

    println!("HTML generado en {}", out.display());
    println!("Ejecuta scripts/capture-screenshots.ps1 para obtener los PNG.");
}

fn app() -> App {
    App::new(
        Config {
            workers: 4,
            ..Config::default()
        },
        true,
        false,
        true,
    )
}

fn dir(name: &str) -> BrowseEntry {
    BrowseEntry {
        name: name.into(),
        path: PathBuf::from(BASE).join(name),
        is_dir: true,
        size: 0,
    }
}

fn file(name: &str, size: u64) -> BrowseEntry {
    BrowseEntry {
        name: name.into(),
        path: PathBuf::from(BASE).join(name),
        is_dir: false,
        size,
    }
}

fn sample_items(count: usize) -> Vec<SourceItem> {
    let mut items = Vec::with_capacity(count);
    for index in 0..count {
        let format = match index % 4 {
            0 => ImageFormat::Jpeg,
            1 => ImageFormat::Jpeg,
            2 => ImageFormat::Png,
            _ => ImageFormat::WebP,
        };
        let folder = match index % 3 {
            0 => "Roma",
            1 => "Playa",
            _ => "Familia",
        };
        let name = format!("IMG_{:04}.{}", 1000 + index, format.extension());
        let relative = PathBuf::from(folder).join(&name);
        let path = PathBuf::from(BASE).join(&relative);
        let mut probe = Probe {
            format: Some(format),
            width: 4032,
            height: 3024,
            color: if format == ImageFormat::Jpeg {
                "Rgb8".into()
            } else {
                "Rgba8".into()
            },
            has_alpha: format != ImageFormat::Jpeg,
            ..Default::default()
        };
        if index >= count.saturating_sub(6) {
            if index % 2 == 0 {
                probe.animated = true;
            } else {
                probe.error = Some("Cabecera truncada".into());
            }
        }
        items.push(SourceItem {
            id: index,
            path,
            relative,
            size: 3_100_000 + (index as u64 * 47_000) % 5_200_000,
            modified: None,
            format,
            probe,
        });
    }
    items
}

fn clean_items(count: usize) -> Vec<SourceItem> {
    let mut items = sample_items(count);
    for item in &mut items {
        item.probe.animated = false;
        item.probe.error = None;
    }
    items
}

fn plan_for(items: &[SourceItem]) -> OperationPlan {
    let options = OptimizeOptions::for_purpose(Purpose::Share);
    let mut plan = build_plan(
        items.to_vec(),
        &options,
        Destination::Separate {
            root: PathBuf::from(format!("{}-imgshrink", BASE)),
        },
        ConflictPolicy::Skip,
    );
    plan.warnings = vec![
        "«IMG_1027.png» tiene transparencia y se aplanará sobre fondo RGB al guardar JPEG.".into(),
        "«IMG_1031.webp» tiene perfil ICC; la v1 no lo incrusta en WebP.".into(),
        "«IMG_1058.png» usa 16 bits por canal; se convertirá a 8 bits.".into(),
        "«IMG_1074.jpg» es muy pequeño; probablemente no mejore y se omitirá si crece.".into(),
        "2 archivos no se pudieron leer y 4 son animados; se omitirán.".into(),
    ];
    plan
}

struct CellData {
    symbol: String,
    fg: Color,
    bg: Color,
    modifier: Modifier,
}

fn render(app: &mut App) -> Vec<Vec<CellData>> {
    let backend = TestBackend::new(COLS, ROWS);
    let mut terminal = Terminal::new(backend).expect("terminal de prueba");
    terminal
        .draw(|frame| imgshrink_tui::ui::draw(frame, app))
        .expect("dibujar");
    let buffer = terminal.backend().buffer();
    let mut rows = Vec::with_capacity(ROWS as usize);
    for y in 0..ROWS {
        let mut row = Vec::with_capacity(COLS as usize);
        for x in 0..COLS {
            let cell = buffer.cell((x, y)).expect("celda");
            row.push(CellData {
                symbol: cell.symbol().to_string(),
                fg: cell.fg,
                bg: cell.bg,
                modifier: cell.modifier,
            });
        }
        rows.push(row);
    }
    rows
}

fn page(name: &str, app: &mut App, out: &Path, title: &str) {
    let rows = render(app);
    let mut body = String::new();
    for row in rows {
        let mut x = 0usize;
        while x < row.len() {
            let style = (row[x].fg, row[x].bg, row[x].modifier);
            let start = x;
            x += 1;
            while x < row.len() && (row[x].fg, row[x].bg, row[x].modifier) == style {
                x += 1;
            }
            let width = x - start;
            body.push_str(&format!(
                "<span style=\"{};width:{}ch;\">",
                style_css(style.0, style.1, style.2),
                width
            ));
            for cell in &row[start..x] {
                body.push_str(&escape(if cell.symbol.is_empty() {
                    " "
                } else {
                    &cell.symbol
                }));
            }
            body.push_str("</span>");
        }
        body.push('\n');
    }

    let mut html = String::new();
    html.push_str("<!doctype html><html lang=\"es\"><head><meta charset=\"utf-8\">");
    html.push_str("<meta name=\"color-scheme\" content=\"dark\">");
    html.push_str("<title>ImgShrink · ");
    html.push_str(title);
    html.push_str("</title><style>");
    html.push_str(CSS);
    html.push_str("</style></head><body><div class=\"terminal\">");
    html.push_str("<div class=\"titlebar\"><div class=\"dots\"><i class=\"red\"></i><i class=\"yellow\"></i><i class=\"green\"></i></div>");
    html.push_str(
        "<div class=\"title\">imgshrink — 120 × 34</div><div class=\"site\">jmolo.es</div></div>",
    );
    html.push_str(&format!(
        "<div class=\"screen\" style=\"width:{}ch\">",
        COLS
    ));
    html.push_str(&body);
    html.push_str("</div></div></body></html>");
    let path = out.join(format!("{}.html", name));
    std::fs::write(&path, html).expect("escribir HTML");
    println!("{}", path.display());
}

const CSS: &str = r#"
:root { color-scheme: dark; }
html, body { margin: 0; padding: 0; }
body {
  min-height: 100vh;
  display: flex;
  align-items: center;
  justify-content: center;
  background: radial-gradient(1100px 620px at 50% 0%, #10202a 0%, #05080c 62%);
  font-family: "Cascadia Mono", "Consolas", "DejaVu Sans Mono", monospace;
}
.terminal {
  background: #0d1117;
  border: 1px solid #2b3440;
  border-radius: 14px;
  box-shadow: 0 32px 90px rgba(0, 0, 0, .58), 0 0 0 1px rgba(255,255,255,.025) inset;
  overflow: hidden;
}
.titlebar {
  height: 36px;
  display: grid;
  grid-template-columns: 1fr auto 1fr;
  align-items: center;
  padding: 0 16px;
  background: #161b22;
  border-bottom: 1px solid #21262d;
  color: #8b949e;
  font-size: 12px;
}
.dots { display: flex; gap: 7px; }
.dots i { display: block; width: 11px; height: 11px; border-radius: 50%; }
.red { background: #ff5f56; }
.yellow { background: #ffbd2e; }
.green { background: #27c93f; }
.title { color: #b6c2cf; letter-spacing: .4px; }
.site { text-align: right; color: #58d6be; }
.screen {
  box-sizing: content-box;
  padding: 16px 20px;
  font-size: 15px;
  line-height: 18px;
  white-space: pre;
  color: #dce3eb;
  font-variant-ligatures: none;
  font-feature-settings: "liga" 0, "calt" 0;
}
.screen span {
  display: inline-block;
  height: 18px;
  line-height: 18px;
  vertical-align: top;
}
"#;

fn style_css(fg: Color, bg: Color, modifier: Modifier) -> String {
    let (fg, bg) = if modifier.contains(Modifier::REVERSED) {
        (bg, fg)
    } else {
        (fg, bg)
    };
    let mut css = format!("color:{};", color_css(fg, true));
    if bg != Color::Reset {
        css.push_str(&format!("background:{};", color_css(bg, false)));
    }
    if modifier.contains(Modifier::BOLD) {
        css.push_str("font-weight:700;");
    }
    if modifier.contains(Modifier::DIM) {
        css.push_str("opacity:.62;");
    }
    if modifier.contains(Modifier::ITALIC) {
        css.push_str("font-style:italic;");
    }
    if modifier.contains(Modifier::UNDERLINED) {
        css.push_str("text-decoration:underline;");
    }
    if modifier.contains(Modifier::CROSSED_OUT) {
        css.push_str("text-decoration:line-through;");
    }
    css
}

fn color_css(color: Color, foreground: bool) -> String {
    let fallback = if foreground { "#dce3eb" } else { "#0d1117" };
    match color {
        Color::Reset => fallback.into(),
        Color::Black => "#0b0f14".into(),
        Color::Red => "#ff5f56".into(),
        Color::Green => "#6ed282".into(),
        Color::Yellow => "#ebbe5a".into(),
        Color::Blue => "#6aa8ff".into(),
        Color::Magenta => "#d18cff".into(),
        Color::Cyan => "#58d6be".into(),
        Color::Gray => "#9aa7b4".into(),
        Color::DarkGray => "#5d6a78".into(),
        Color::LightRed => "#ff8a80".into(),
        Color::LightGreen => "#9be8a8".into(),
        Color::LightYellow => "#f5d98a".into(),
        Color::LightBlue => "#9dc7ff".into(),
        Color::LightMagenta => "#e0b0ff".into(),
        Color::LightCyan => "#8ee9d8".into(),
        Color::White => "#f0f6fc".into(),
        Color::Rgb(r, g, b) => format!("rgb({},{},{})", r, g, b),
        Color::Indexed(index) => format!("rgb({},{},{})", index, index, index),
    }
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(character),
        }
    }
    out
}
