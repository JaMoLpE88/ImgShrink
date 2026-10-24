use std::io::IsTerminal;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use clap::Parser;
use imgshrink_core::config::{self, Config};
use imgshrink_core::discovery::{discover, InputSelection};
use imgshrink_core::jobs::{run_blocking, ItemResult, JobOptions};
use imgshrink_core::options::{OptimizeOptions, OutputFormat, Purpose};
use imgshrink_core::output::Destination;
use imgshrink_core::plan::{build_plan, ConflictPolicy};
use imgshrink_core::{APP_AUTHOR, APP_LICENSE, APP_NAME, APP_VERSION, APP_WEBSITE, REPO_URL};

/// Optimizador de imágenes local con interfaz de terminal.
#[derive(Parser, Debug)]
#[command(
    name = "imgshrink",
    version,
    about,
    long_about = None,
    after_help = "Autor: JaMoLpE88 · https://jmolo.es\nRepositorio: https://github.com/JaMoLpE88/ImgShrink\nLicencia: MIT"
)]
struct Args {
    /// Desactiva los colores de la interfaz.
    #[arg(long)]
    no_color: bool,

    /// Usa solo caracteres ASCII en la interfaz.
    #[arg(long)]
    ascii: bool,

    /// Desactiva la captura del ratón.
    #[arg(long)]
    no_mouse: bool,

    /// Número de trabajadores (por defecto, el de la configuración).
    #[arg(long, value_name = "N")]
    workers: Option<usize>,

    /// Nivel de registro: error, warn, info, debug o trace.
    #[arg(long, default_value = "info", value_name = "NIVEL")]
    log_level: String,

    /// Comprueba el motor y la escritura segura sin abrir la interfaz.
    #[arg(long)]
    selftest: bool,

    /// Muestra información del entorno y de los códecs.
    #[arg(long)]
    diagnose: bool,
}

fn main() -> std::process::ExitCode {
    let args = Args::parse();
    match run(args) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Error: {error:#}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn run(args: Args) -> Result<()> {
    if args.selftest {
        return selftest();
    }
    if args.diagnose {
        diagnose();
        return Ok(());
    }

    let _guard = init_logging(&args.log_level);

    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        bail!(
            "ImgShrink necesita una terminal interactiva.\n             Usa «imgshrink --help» para ver las opciones o «imgshrink --selftest» para comprobar el motor."
        );
    }

    let mut config = config::load();
    if let Some(workers) = args.workers {
        config.workers = workers.clamp(1, 16);
    }
    if args.ascii {
        config.ascii = true;
    }
    if args.no_mouse {
        config.mouse = false;
    }

    let ascii = args.ascii || config.ascii;
    let mouse = !args.no_mouse && config.mouse;
    imgshrink_tui::run(config, !args.no_color, ascii, mouse)
        .context("No se pudo iniciar la interfaz de terminal")?;
    Ok(())
}

fn init_logging(level: &str) -> Option<tracing_appender::non_blocking::WorkerGuard> {
    let directory = config::log_dir();
    if std::fs::create_dir_all(&directory).is_err() {
        return None;
    }
    let appender = tracing_appender::rolling::daily(directory, "imgshrink.log");
    let (writer, guard) = tracing_appender::non_blocking(appender);
    let filter = tracing_subscriber::EnvFilter::try_new(level)
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_writer(writer)
        .with_ansi(false)
        .with_target(false)
        .with_env_filter(filter)
        .try_init();
    tracing::info!("{} v{} iniciado", APP_NAME, APP_VERSION);
    Some(guard)
}

fn diagnose() {
    println!("{} v{}", APP_NAME, APP_VERSION);
    println!("Autor: {} · https://{}", APP_AUTHOR, APP_WEBSITE);
    println!("Repositorio: {}", REPO_URL);
    println!("Licencia: {}", APP_LICENSE);
    println!(
        "Sistema: {} {}",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    println!("Configuración: {}", config::config_path().display());
    println!("Estado y registros: {}", config::state_dir().display());
    println!(
        "Directorio actual: {}",
        std::env::current_dir()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|_| "desconocido".into())
    );
    println!("Formatos de entrada/salida: JPEG, PNG, WebP");
    println!("AVIF: no incluido en esta versión");
    println!(
        "Terminal: TERM={} COLORTERM={} WT_SESSION={}",
        std::env::var("TERM").unwrap_or_else(|_| "-".into()),
        std::env::var("COLORTERM").unwrap_or_else(|_| "-".into()),
        std::env::var("WT_SESSION")
            .map(|_| "sí")
            .unwrap_or_else(|_| "no")
    );
    println!("Trabajadores por defecto: {}", Config::default().workers);
}

fn selftest() -> Result<()> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!(
        "imgshrink-selftest-{}-{}",
        std::process::id(),
        stamp
    ));
    let input = root.join("entrada");
    let output = root.join("salida");
    std::fs::create_dir_all(&input).context("No se pudo crear el directorio de prueba")?;

    let result = (|| -> Result<()> {
        let mut image = image::RgbaImage::new(64, 48);
        for (x, y, pixel) in image.enumerate_pixels_mut() {
            *pixel = image::Rgba([
                (x * 4) as u8,
                (y * 5) as u8,
                ((x + y) * 3) as u8,
                if x < 8 { 100 } else { 255 },
            ]);
        }
        image
            .save(input.join("muestra.png"))
            .context("No se pudo generar el PNG de prueba")?;
        let rgb = image::DynamicImage::ImageRgba8(image.clone()).to_rgb8();
        rgb.save(input.join("muestra.jpg"))
            .context("No se pudo generar el JPEG de prueba")?;
        let encoder = webp::Encoder::from_rgba(image.as_raw(), image.width(), image.height());
        std::fs::write(input.join("muestra.webp"), &*encoder.encode_lossless())
            .context("No se pudo generar el WebP de prueba")?;

        let discovered = discover(
            &[InputSelection::directory(input.clone(), true)],
            Some(&output),
        );
        if discovered.items.len() != 3 {
            bail!(
                "Se esperaban 3 imágenes de prueba y se encontraron {}",
                discovered.items.len()
            );
        }
        if !discovered.warnings.is_empty() {
            bail!(
                "El descubrimiento generó avisos inesperados: {:?}",
                discovered.warnings
            );
        }

        let options = OptimizeOptions {
            output_format: OutputFormat::Auto,
            quality: 80,
            keep_if_larger: false,
            ..OptimizeOptions::for_purpose(Purpose::Custom)
        };
        let plan = build_plan(
            discovered.items,
            &options,
            Destination::Separate {
                root: output.clone(),
            },
            ConflictPolicy::Overwrite,
        );
        if plan.processable != 3 {
            bail!("El plan no considera procesables las 3 imágenes");
        }
        let (summary, results) = run_blocking(
            &plan,
            &options,
            &JobOptions {
                workers: 2,
                conflict_policy: ConflictPolicy::Overwrite,
                journal: None,
            },
        );
        if summary.succeeded != 3 || summary.failed != 0 {
            let details: Vec<String> = results
                .iter()
                .filter_map(|(_, result)| match result {
                    ItemResult::Failed { code, message } => Some(format!("{}: {}", code, message)),
                    _ => None,
                })
                .collect();
            bail!(
                "El procesamiento no fue correcto (éxitos {}, errores {}). {}",
                summary.succeeded,
                summary.failed,
                details.join(" | ")
            );
        }
        for name in ["muestra.png", "muestra.jpg", "muestra.webp"] {
            let path = output.join(name);
            if !path.exists() {
                bail!("Falta la salida {}", path.display());
            }
            image::open(&path)
                .with_context(|| format!("La salida {} no se puede decodificar", path.display()))?;
        }
        Ok(())
    })();

    let _ = std::fs::remove_dir_all(&root);
    result?;
    println!("Selftest OK: JPEG, PNG y WebP procesados, escritos y verificados.");
    println!("La escritura segura y la validación posterior han funcionado.");
    Ok(())
}
