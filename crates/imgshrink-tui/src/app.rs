use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent, MouseEventKind,
};
use imgshrink_core::config::{self, Config};
use imgshrink_core::discovery::{self, InputSelection, SourceItem};
use imgshrink_core::jobs::{self, ItemResult, JobEvent, JobOptions, Stage};
use imgshrink_core::journal::Journal;
use imgshrink_core::options::Purpose;
use imgshrink_core::output::Destination;
use imgshrink_core::pathutil::{display_path, format_bytes, validate_suffix};
use imgshrink_core::plan::{self, ConflictPolicy, OperationPlan};
use imgshrink_core::stats::Summary;
use imgshrink_core::{report, OptimizeOptions};
use ratatui::layout::Rect;

use crate::terminal::Tui;
use crate::theme::Theme;

const LIST_ROWS: usize = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Welcome,
    Purpose,
    Source,
    Analysis,
    Destination,
    Review,
    Running,
    Summary,
}

impl Screen {
    pub fn step(self) -> &'static str {
        match self {
            Screen::Welcome => "Inicio",
            Screen::Purpose => "1 · Finalidad",
            Screen::Source => "2 · Imágenes",
            Screen::Analysis => "3 · Análisis",
            Screen::Destination => "4 · Destino",
            Screen::Review => "5 · Revisión",
            Screen::Running => "6 · Procesando",
            Screen::Summary => "7 · Resumen",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputTarget {
    SourcePath,
    SeparatePath,
    Suffix,
}

#[derive(Clone, Debug)]
pub enum Overlay {
    Help {
        scroll: u16,
    },
    Settings,
    ConfirmOverwrite {
        stage: u8,
        choice: usize,
        typed: String,
        cursor: usize,
        error: Option<String>,
    },
    Errors {
        scroll: u16,
    },
    TextInput {
        target: InputTarget,
        value: String,
        cursor: usize,
        error: Option<String>,
    },
    Options {
        index: usize,
        backup: OptimizeOptions,
    },
}

#[derive(Clone, Debug)]
pub struct BrowseEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub size: u64,
}

pub struct SourceScreen {
    pub cwd: PathBuf,
    pub entries: Vec<BrowseEntry>,
    pub index: usize,
    pub offset: usize,
    pub selected_files: Vec<PathBuf>,
    pub selected_dirs: Vec<PathBuf>,
    pub recursive: bool,
    pub show_hidden: bool,
    pub error: Option<String>,
    pub list_rect: Rect,
}

impl SourceScreen {
    pub fn new(start: Option<PathBuf>) -> Self {
        let cwd = start
            .filter(|path| path.is_dir())
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."));
        let mut screen = Self {
            cwd,
            entries: Vec::new(),
            index: 0,
            offset: 0,
            selected_files: Vec::new(),
            selected_dirs: Vec::new(),
            recursive: false,
            show_hidden: false,
            error: None,
            list_rect: Rect::default(),
        };
        screen.refresh();
        screen
    }

    pub fn refresh(&mut self) {
        self.entries.clear();
        self.error = None;
        let reader = match std::fs::read_dir(&self.cwd) {
            Ok(reader) => reader,
            Err(error) => {
                self.error = Some(format!(
                    "No se pudo abrir «{}»: {}",
                    display_path(&self.cwd),
                    error
                ));
                return;
            }
        };
        for entry in reader.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if !self.show_hidden && name.starts_with('.') {
                continue;
            }
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(_) => continue,
            };
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                self.entries.push(BrowseEntry {
                    name,
                    path,
                    is_dir: true,
                    size: 0,
                });
            } else if file_type.is_file() && is_image_extension(&path) {
                self.entries.push(BrowseEntry {
                    name,
                    path: path.clone(),
                    is_dir: false,
                    size: entry.metadata().map(|metadata| metadata.len()).unwrap_or(0),
                });
            }
        }
        self.entries.sort_by(|a, b| {
            b.is_dir
                .cmp(&a.is_dir)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        self.index = self.index.min(self.entries.len().saturating_sub(1));
        self.ensure_visible();
    }

    pub fn ensure_visible(&mut self) {
        if self.index < self.offset {
            self.offset = self.index;
        } else if self.index >= self.offset + LIST_ROWS {
            self.offset = self.index + 1 - LIST_ROWS;
        }
        if self.offset > self.entries.len().saturating_sub(1) {
            self.offset = self.entries.len().saturating_sub(LIST_ROWS);
        }
    }

    pub fn current(&self) -> Option<&BrowseEntry> {
        self.entries.get(self.index)
    }

    pub fn is_selected(&self, path: &Path) -> bool {
        self.selected_files.iter().any(|selected| selected == path)
    }

    pub fn toggle_current(&mut self) -> Option<String> {
        let entry = self.current()?.clone();
        if entry.is_dir {
            return None;
        }
        if let Some(position) = self
            .selected_files
            .iter()
            .position(|path| path == &entry.path)
        {
            self.selected_files.remove(position);
            Some(format!("Se quitó «{}» de la selección", entry.name))
        } else {
            self.selected_files.push(entry.path.clone());
            Some(format!("Se añadió «{}» a la selección", entry.name))
        }
    }

    pub fn add_current_dir(&mut self) -> String {
        if self.selected_dirs.iter().any(|path| path == &self.cwd) {
            return "La carpeta actual ya estaba seleccionada".into();
        }
        self.selected_dirs.push(self.cwd.clone());
        format!("Carpeta añadida: {}", display_path(&self.cwd))
    }

    pub fn selections(&self) -> Vec<InputSelection> {
        let mut selections = Vec::new();
        for file in &self.selected_files {
            selections.push(InputSelection::file(file.clone()));
        }
        for dir in &self.selected_dirs {
            selections.push(InputSelection::directory(dir.clone(), self.recursive));
        }
        selections
    }

    pub fn clear(&mut self) {
        self.selected_files.clear();
        self.selected_dirs.clear();
    }

    pub fn parent(&mut self) {
        if let Some(parent) = self.cwd.parent() {
            self.cwd = parent.to_path_buf();
            self.index = 0;
            self.offset = 0;
            self.refresh();
        }
    }
}

fn is_image_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "jpg" | "jpeg" | "jpe" | "jfif" | "png" | "webp"
            )
        })
        .unwrap_or(false)
}

pub struct DestinationScreen {
    pub mode: usize,
    pub root: PathBuf,
    pub suffix: String,
    pub conflict: ConflictPolicy,
    pub root_custom: bool,
}

impl DestinationScreen {
    pub fn new(source: &SourceScreen) -> Self {
        Self {
            mode: 0,
            root: default_output_root(source),
            suffix: "-imgshrink".into(),
            conflict: ConflictPolicy::Skip,
            root_custom: false,
        }
    }

    pub fn destination(&self) -> Destination {
        match self.mode {
            1 => Destination::InPlace,
            2 => Destination::Suffix {
                suffix: self.suffix.clone(),
            },
            _ => Destination::Separate {
                root: self.root.clone(),
            },
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.mode == 2 {
            validate_suffix(&self.suffix)?;
        }
        if self.mode == 0 && self.root.as_os_str().is_empty() {
            return Err("Indica una carpeta de salida".into());
        }
        Ok(())
    }
}

fn default_output_root(source: &SourceScreen) -> PathBuf {
    if source.selected_dirs.len() == 1 && source.selected_files.is_empty() {
        let dir = &source.selected_dirs[0];
        let name = dir
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "imagenes".into());
        return dir
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(format!("{}-imgshrink", name));
    }
    source.cwd.join("imgshrink-output")
}

pub struct RunningState {
    pub rx: crossbeam_channel::Receiver<JobEvent>,
    pub cancel: Arc<AtomicBool>,
    pub started: Instant,
    pub summary: Summary,
    pub current_id: Option<usize>,
    pub current_stage: Stage,
    pub failures: Vec<(String, String)>,
    pub handle: Option<std::thread::JoinHandle<()>>,
}

pub struct AnalysisOutcome {
    items: Vec<SourceItem>,
    plan: OperationPlan,
    warnings: Vec<String>,
    skipped: usize,
    unreadable: usize,
}

pub struct App {
    pub config: Config,
    pub color: bool,
    pub ascii: bool,
    pub mouse: bool,
    pub theme: Theme,
    pub screen: Screen,
    pub should_quit: bool,
    pub purpose: Purpose,
    pub purpose_index: usize,
    pub welcome_index: usize,
    pub options: OptimizeOptions,
    pub source: SourceScreen,
    pub analysis_rx: Option<Receiver<AnalysisOutcome>>,
    pub analysis_started: Instant,
    pub analysis_error: Option<String>,
    pub analysis_skipped: usize,
    pub analysis_unreadable: usize,
    pub discovered: Vec<SourceItem>,
    pub extra_warnings: Vec<String>,
    pub plan: Option<OperationPlan>,
    pub destination: DestinationScreen,
    pub running: Option<RunningState>,
    pub summary: Option<Summary>,
    pub results: Vec<(usize, ItemResult)>,
    pub report_path: Option<PathBuf>,
    pub report_error: Option<String>,
    pub overlay: Option<Overlay>,
    pub status: Option<(String, Instant)>,
    pub spinner: usize,
    pub warning_scroll: u16,
    pub quit_after_finish: bool,
    pub journal_pending: usize,
}

impl App {
    pub fn new(config: Config, color: bool, ascii: bool, mouse: bool) -> Self {
        let mut source = SourceScreen::new(config.last_dir.clone());
        source.recursive = false;
        let destination = DestinationScreen::new(&source);
        let journal_pending = Journal::new(config::journal_path()).pending().len();
        Self {
            config,
            color,
            ascii,
            mouse,
            theme: if color { Theme::dark() } else { Theme::mono() },
            screen: Screen::Welcome,
            should_quit: false,
            purpose: Purpose::Share,
            purpose_index: 0,
            welcome_index: 0,
            options: OptimizeOptions::for_purpose(Purpose::Share),
            source,
            analysis_rx: None,
            analysis_started: Instant::now(),
            analysis_error: None,
            analysis_skipped: 0,
            analysis_unreadable: 0,
            discovered: Vec::new(),
            extra_warnings: Vec::new(),
            plan: None,
            destination,
            running: None,
            summary: None,
            results: Vec::new(),
            report_path: None,
            report_error: None,
            overlay: None,
            status: None,
            spinner: 0,
            warning_scroll: 0,
            quit_after_finish: false,
            journal_pending,
        }
    }

    pub fn run(&mut self, terminal: &mut Tui) -> std::io::Result<()> {
        loop {
            terminal.draw(|frame| crate::ui::draw(frame, self))?;
            if crossterm::event::poll(Duration::from_millis(50))? {
                if let Ok(event) = crossterm::event::read() {
                    self.handle_event(event);
                }
            }
            self.tick();
            if self.should_quit {
                break;
            }
        }
        self.config.last_dir = Some(self.source.cwd.clone());
        let _ = config::save(&self.config);
        Ok(())
    }

    pub fn tick(&mut self) {
        self.spinner = self.spinner.wrapping_add(1);
        if let Some(rx) = self.analysis_rx.take() {
            match rx.try_recv() {
                Ok(outcome) => {
                    self.discovered = outcome.items;
                    self.plan = Some(outcome.plan);
                    self.extra_warnings = outcome.warnings;
                    self.analysis_skipped = outcome.skipped;
                    self.analysis_unreadable = outcome.unreadable;
                    self.analysis_error = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    self.analysis_rx = Some(rx);
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.analysis_error = Some("El análisis se interrumpió inesperadamente".into());
                }
            }
        }

        let mut events = Vec::new();
        if let Some(running) = &self.running {
            while let Ok(event) = running.rx.try_recv() {
                events.push(event);
            }
        }
        for event in events {
            self.handle_job_event(event);
        }

        if let Some((_, at)) = &self.status {
            if at.elapsed() > Duration::from_secs(6) {
                self.status = None;
            }
        }
    }

    pub fn set_status(&mut self, message: impl Into<String>) {
        self.status = Some((message.into(), Instant::now()));
    }

    fn handle_event(&mut self, event: Event) {
        match event {
            Event::Key(key) if key.kind == KeyEventKind::Press => self.handle_key(key),
            Event::Mouse(mouse) => self.handle_mouse(mouse),
            Event::Paste(text) => self.handle_paste(text),
            _ => {}
        }
    }

    fn handle_key(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.request_quit();
            return;
        }
        if self.overlay.is_some() {
            self.handle_overlay_key(key);
            return;
        }
        if key.code == KeyCode::Char('?') {
            self.overlay = Some(Overlay::Help { scroll: 0 });
            return;
        }
        if key.code == KeyCode::Char('q') {
            self.request_quit();
            return;
        }
        match self.screen {
            Screen::Welcome => self.key_welcome(key),
            Screen::Purpose => self.key_purpose(key),
            Screen::Source => self.key_source(key),
            Screen::Analysis => self.key_analysis(key),
            Screen::Destination => self.key_destination(key),
            Screen::Review => self.key_review(key),
            Screen::Running => self.key_running(key),
            Screen::Summary => self.key_summary(key),
        }
    }

    fn key_welcome(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.welcome_index = self.welcome_index.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.welcome_index = (self.welcome_index + 1).min(3);
            }
            KeyCode::Enter => match self.welcome_index {
                0 => self.screen = Screen::Purpose,
                1 => self.overlay = Some(Overlay::Help { scroll: 0 }),
                2 => self.overlay = Some(Overlay::Settings),
                _ => self.request_quit(),
            },
            KeyCode::Esc => self.should_quit = true,
            _ => {}
        }
    }

    fn key_purpose(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.purpose_index = self.purpose_index.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.purpose_index = (self.purpose_index + 1).min(3);
            }
            KeyCode::Enter => {
                self.purpose = Purpose::ALL[self.purpose_index];
                self.options = OptimizeOptions::for_purpose(self.purpose);
                self.screen = Screen::Source;
            }
            KeyCode::Esc => self.screen = Screen::Welcome,
            _ => {}
        }
    }

    fn key_source(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.source.index = self.source.index.saturating_sub(1);
                self.source.ensure_visible();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !self.source.entries.is_empty() {
                    self.source.index = (self.source.index + 1).min(self.source.entries.len() - 1);
                    self.source.ensure_visible();
                }
            }
            KeyCode::PageUp => {
                self.source.index = self.source.index.saturating_sub(LIST_ROWS);
                self.source.ensure_visible();
            }
            KeyCode::PageDown => {
                if !self.source.entries.is_empty() {
                    self.source.index =
                        (self.source.index + LIST_ROWS).min(self.source.entries.len() - 1);
                    self.source.ensure_visible();
                }
            }
            KeyCode::Home => {
                self.source.index = 0;
                self.source.offset = 0;
            }
            KeyCode::End => {
                self.source.index = self.source.entries.len().saturating_sub(1);
                self.source.ensure_visible();
            }
            KeyCode::Enter => {
                if let Some(entry) = self.source.current().cloned() {
                    if entry.is_dir {
                        self.source.cwd = entry.path;
                        self.source.index = 0;
                        self.source.offset = 0;
                        self.source.refresh();
                    } else if let Some(message) = self.source.toggle_current() {
                        self.set_status(message);
                    }
                }
            }
            KeyCode::Char(' ') => {
                if let Some(message) = self.source.toggle_current() {
                    self.set_status(message);
                }
            }
            KeyCode::Char('a') => {
                let message = self.source.add_current_dir();
                self.set_status(message);
            }
            KeyCode::Char('r') => {
                self.source.recursive = !self.source.recursive;
                self.set_status(if self.source.recursive {
                    "Subcarpetas incluidas"
                } else {
                    "Subcarpetas excluidas"
                });
            }
            KeyCode::Char('x') => {
                self.source.clear();
                self.set_status("Selección vaciada");
            }
            KeyCode::Char('h') => {
                self.source.show_hidden = !self.source.show_hidden;
                self.source.refresh();
            }
            KeyCode::Char('e') | KeyCode::Char('/') => {
                self.overlay = Some(Overlay::TextInput {
                    target: InputTarget::SourcePath,
                    value: display_path(&self.source.cwd),
                    cursor: display_path(&self.source.cwd).chars().count(),
                    error: None,
                });
            }
            KeyCode::Backspace => self.source.parent(),
            KeyCode::Char('p') => self.start_analysis(),
            KeyCode::Esc => self.screen = Screen::Purpose,
            _ => {}
        }
    }

    fn key_analysis(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => {
                if self.plan.is_some() {
                    self.screen = Screen::Destination;
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.warning_scroll = self.warning_scroll.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.warning_scroll = self.warning_scroll.saturating_add(1);
            }
            KeyCode::Esc => {
                self.analysis_rx = None;
                self.screen = Screen::Source;
            }
            _ => {}
        }
    }

    fn key_destination(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.destination.mode = self.destination.mode.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.destination.mode = (self.destination.mode + 1).min(2);
            }
            KeyCode::Char('e') => match self.destination.mode {
                0 => {
                    let value = display_path(&self.destination.root);
                    self.overlay = Some(Overlay::TextInput {
                        target: InputTarget::SeparatePath,
                        cursor: value.chars().count(),
                        value,
                        error: None,
                    });
                }
                2 => {
                    let value = self.destination.suffix.clone();
                    self.overlay = Some(Overlay::TextInput {
                        target: InputTarget::Suffix,
                        cursor: value.chars().count(),
                        value,
                        error: None,
                    });
                }
                _ => self.set_status("El modo de sobrescritura no usa esta opción"),
            },
            KeyCode::Char('c') => {
                self.destination.conflict = match self.destination.conflict {
                    ConflictPolicy::Skip => ConflictPolicy::Overwrite,
                    ConflictPolicy::Overwrite => ConflictPolicy::Rename,
                    _ => ConflictPolicy::Skip,
                };
            }
            KeyCode::Enter => {
                if let Err(error) = self.destination.validate() {
                    self.set_status(error);
                    return;
                }
                if self.destination.mode == 1 {
                    self.rebuild_plan();
                    self.overlay = Some(Overlay::ConfirmOverwrite {
                        stage: 1,
                        choice: 0,
                        typed: String::new(),
                        cursor: 0,
                        error: None,
                    });
                } else {
                    self.rebuild_plan();
                    self.screen = Screen::Review;
                }
            }
            KeyCode::Esc => self.screen = Screen::Analysis,
            _ => {}
        }
    }

    fn key_review(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => self.start_job(),
            KeyCode::Up | KeyCode::Char('k') => {
                self.warning_scroll = self.warning_scroll.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.warning_scroll = self.warning_scroll.saturating_add(1);
            }
            KeyCode::Char('o') => {
                self.overlay = Some(Overlay::Options {
                    index: 0,
                    backup: self.options.clone(),
                });
            }
            KeyCode::Esc => self.screen = Screen::Destination,
            _ => {}
        }
    }

    fn key_running(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('c') => self.cancel_job(),
            KeyCode::Char('e') => self.overlay = Some(Overlay::Errors { scroll: 0 }),
            KeyCode::Char('q') => self.request_quit(),
            _ => {}
        }
    }

    fn key_summary(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('o') => self.open_output(),
            KeyCode::Char('n') => self.reset_for_new(),
            KeyCode::Esc => self.screen = Screen::Welcome,
            _ => {}
        }
    }

    fn handle_overlay_key(&mut self, key: KeyEvent) {
        let Some(mut overlay) = self.overlay.take() else {
            return;
        };
        let mut keep = true;
        match &mut overlay {
            Overlay::Help { scroll } => match key.code {
                KeyCode::Esc | KeyCode::Char('?') => keep = false,
                KeyCode::Up | KeyCode::Char('k') => *scroll = scroll.saturating_sub(1),
                KeyCode::Down | KeyCode::Char('j') => *scroll = scroll.saturating_add(1),
                _ => {}
            },
            Overlay::Settings => match key.code {
                KeyCode::Esc => keep = false,
                KeyCode::Char('+') => {
                    self.config.workers = (self.config.workers + 1).min(16);
                }
                KeyCode::Char('-') => {
                    self.config.workers = self.config.workers.saturating_sub(1).max(1);
                }
                KeyCode::Char('a') => {
                    self.ascii = !self.ascii;
                    self.config.ascii = self.ascii;
                }
                KeyCode::Char('k') => {
                    self.config.keep_if_larger = !self.config.keep_if_larger;
                    self.options.keep_if_larger = self.config.keep_if_larger;
                }
                KeyCode::Char('s') => {
                    let _ = config::save(&self.config);
                    self.set_status("Configuración guardada");
                }
                _ => {}
            },
            Overlay::ConfirmOverwrite {
                stage,
                choice,
                typed,
                cursor,
                error,
            } => {
                if *stage == 1 {
                    match key.code {
                        KeyCode::Left | KeyCode::Char('h') => *choice = 0,
                        KeyCode::Right | KeyCode::Char('l') => *choice = 1,
                        KeyCode::Esc => keep = false,
                        KeyCode::Enter => {
                            if *choice == 0 {
                                keep = false;
                            } else {
                                *stage = 2;
                            }
                        }
                        _ => {}
                    }
                } else {
                    match key.code {
                        KeyCode::Esc => keep = false,
                        KeyCode::Enter => {
                            if typed == "SOBRESCRIBIR" {
                                keep = false;
                                self.rebuild_plan();
                                self.screen = Screen::Review;
                            } else {
                                *error = Some(
                                    "Escribe SOBRESCRIBIR en mayúsculas para continuar".into(),
                                );
                            }
                        }
                        KeyCode::Backspace => {
                            text_backspace(typed, cursor);
                            *error = None;
                        }
                        KeyCode::Left => *cursor = cursor.saturating_sub(1),
                        KeyCode::Right => *cursor = (*cursor + 1).min(typed.chars().count()),
                        KeyCode::Char(character) => {
                            text_insert(typed, cursor, character);
                            *error = None;
                        }
                        _ => {}
                    }
                }
            }
            Overlay::Options { index, backup } => match key.code {
                KeyCode::Esc => {
                    self.options = backup.clone();
                    self.rebuild_plan();
                    keep = false;
                }
                KeyCode::Enter => {
                    self.rebuild_plan();
                    keep = false;
                }
                KeyCode::Up | KeyCode::Char('k') => *index = index.saturating_sub(1),
                KeyCode::Down | KeyCode::Char('j') => *index = (*index + 1).min(7),
                KeyCode::Left | KeyCode::Char('h') => self.change_option(*index, -1),
                KeyCode::Right | KeyCode::Char('l') => self.change_option(*index, 1),
                _ => {}
            },
            Overlay::Errors { scroll } => match key.code {
                KeyCode::Esc => keep = false,
                KeyCode::Up | KeyCode::Char('k') => *scroll = scroll.saturating_sub(1),
                KeyCode::Down | KeyCode::Char('j') => *scroll = scroll.saturating_add(1),
                _ => {}
            },
            Overlay::TextInput {
                target,
                value,
                cursor,
                error,
            } => match key.code {
                KeyCode::Esc => keep = false,
                KeyCode::Enter => {
                    let applied = match target {
                        InputTarget::SourcePath => {
                            let path = PathBuf::from(value.trim());
                            if path.is_dir() {
                                self.source.cwd = path;
                                self.source.index = 0;
                                self.source.offset = 0;
                                self.source.refresh();
                                Ok(())
                            } else {
                                Err("La ruta no es una carpeta existente".to_string())
                            }
                        }
                        InputTarget::SeparatePath => {
                            let path = PathBuf::from(value.trim());
                            let parent_ok = path
                                .parent()
                                .map(|parent| parent.as_os_str().is_empty() || parent.is_dir())
                                .unwrap_or(false);
                            if path.is_dir() || (!path.exists() && parent_ok) {
                                self.destination.root = path;
                                self.destination.root_custom = true;
                                Ok(())
                            } else {
                                Err("La carpeta no existe y su carpeta padre tampoco".to_string())
                            }
                        }
                        InputTarget::Suffix => validate_suffix(value).map(|suffix| {
                            self.destination.suffix = suffix;
                        }),
                    };
                    match applied {
                        Ok(()) => keep = false,
                        Err(message) => *error = Some(message),
                    }
                }
                KeyCode::Backspace => {
                    text_backspace(value, cursor);
                    *error = None;
                }
                KeyCode::Left => *cursor = cursor.saturating_sub(1),
                KeyCode::Right => *cursor = (*cursor + 1).min(value.chars().count()),
                KeyCode::Home => *cursor = 0,
                KeyCode::End => *cursor = value.chars().count(),
                KeyCode::Char(character) => {
                    text_insert(value, cursor, character);
                    *error = None;
                }
                _ => {}
            },
        }
        if keep {
            self.overlay = Some(overlay);
        }
    }

    fn handle_paste(&mut self, text: String) {
        let cleaned: String = text
            .chars()
            .filter(|character| !character.is_control())
            .collect();
        if cleaned.is_empty() {
            return;
        }
        match &mut self.overlay {
            Some(Overlay::TextInput { value, cursor, .. }) => {
                paste_text(value, cursor, &cleaned);
            }
            Some(Overlay::ConfirmOverwrite {
                stage,
                typed,
                cursor,
                ..
            }) if *stage == 2 => {
                paste_text(typed, cursor, &cleaned);
            }
            _ => {}
        }
    }

    fn handle_mouse(&mut self, mouse: MouseEvent) {
        match mouse.kind {
            MouseEventKind::ScrollDown => match self.screen {
                Screen::Source => {
                    if !self.source.entries.is_empty() {
                        self.source.index =
                            (self.source.index + 1).min(self.source.entries.len() - 1);
                        self.source.ensure_visible();
                    }
                }
                _ => {
                    self.warning_scroll = self.warning_scroll.saturating_add(1);
                }
            },
            MouseEventKind::ScrollUp => match self.screen {
                Screen::Source => {
                    self.source.index = self.source.index.saturating_sub(1);
                    self.source.ensure_visible();
                }
                _ => {
                    self.warning_scroll = self.warning_scroll.saturating_sub(1);
                }
            },
            MouseEventKind::Down(crossterm::event::MouseButton::Left)
                if self.screen == Screen::Source =>
            {
                let rect = self.source.list_rect;
                if mouse.row >= rect.y && mouse.row < rect.y + rect.height {
                    let row = (mouse.row - rect.y) as usize;
                    let index = self.source.offset + row;
                    if index < self.source.entries.len() {
                        self.source.index = index;
                        self.source.ensure_visible();
                    }
                }
            }
            _ => {}
        }
    }

    fn change_option(&mut self, index: usize, delta: i32) {
        match index {
            0 => {
                let formats = [
                    imgshrink_core::OutputFormat::Auto,
                    imgshrink_core::OutputFormat::Jpeg,
                    imgshrink_core::OutputFormat::Png,
                    imgshrink_core::OutputFormat::WebP,
                ];
                let current = formats
                    .iter()
                    .position(|format| *format == self.options.output_format)
                    .unwrap_or(0);
                let next = (current as i32 + delta).rem_euclid(formats.len() as i32) as usize;
                self.options.output_format = formats[next];
                if self.options.output_format == imgshrink_core::OutputFormat::Jpeg {
                    self.options.lossless = false;
                }
            }
            1 => {
                let quality = self.options.quality as i32 + delta * 5;
                self.options.quality = quality.clamp(1, 100) as u8;
            }
            2 => {
                if matches!(
                    self.options.output_format,
                    imgshrink_core::OutputFormat::Jpeg
                ) {
                    self.set_status("JPEG no admite compresión sin pérdida");
                } else {
                    self.options.lossless = !self.options.lossless;
                }
            }
            3 => {
                let values = [None, Some(1280), Some(1920), Some(2560), Some(4096)];
                let current = values
                    .iter()
                    .position(|value| *value == self.options.max_dimension)
                    .unwrap_or(0);
                let next = (current as i32 + delta).rem_euclid(values.len() as i32) as usize;
                self.options.max_dimension = values[next];
            }
            4 => {
                self.options.metadata = match self.options.metadata {
                    imgshrink_core::MetadataPolicy::ColorOnly => {
                        imgshrink_core::MetadataPolicy::StripAll
                    }
                    imgshrink_core::MetadataPolicy::StripAll => {
                        imgshrink_core::MetadataPolicy::ColorOnly
                    }
                };
            }
            5 => self.options.keep_if_larger = !self.options.keep_if_larger,
            6 => {
                let effort = self.options.png_effort as i32 + delta;
                self.options.png_effort = effort.clamp(0, 6) as u8;
            }
            7 => {}
            _ => {}
        }
    }

    fn start_analysis(&mut self) {
        let selections = self.source.selections();
        if selections.is_empty() {
            self.set_status("Selecciona al menos un archivo o carpeta (Espacio / a)");
            return;
        }
        if self.destination.mode == 0 && !self.destination.root_custom {
            self.destination.root = default_output_root(&self.source);
        }
        let options = self.options.clone();
        let destination = self.destination.destination();
        let policy = self.destination.conflict;
        let exclude = match &destination {
            Destination::Separate { root } => Some(root.clone()),
            _ => None,
        };
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let discovered = discovery::discover(&selections, exclude.as_deref());
            let mut plan =
                plan::build_plan(discovered.items.clone(), &options, destination, policy);
            plan.warnings.extend(discovered.warnings.clone());
            let _ = tx.send(AnalysisOutcome {
                items: discovered.items,
                plan,
                warnings: discovered.warnings,
                skipped: discovered.skipped,
                unreadable: discovered.unreadable,
            });
        });
        self.analysis_rx = Some(rx);
        self.analysis_started = Instant::now();
        self.analysis_error = None;
        self.plan = None;
        self.screen = Screen::Analysis;
    }

    fn rebuild_plan(&mut self) {
        if self.discovered.is_empty() && self.plan.is_none() {
            return;
        }
        let destination = self.destination.destination();
        let mut plan = plan::build_plan(
            self.discovered.clone(),
            &self.options,
            destination,
            self.destination.conflict,
        );
        plan.warnings.extend(self.extra_warnings.clone());
        self.plan = Some(plan);
    }

    fn start_job(&mut self) {
        let Some(plan) = self.plan.clone() else {
            return;
        };
        if plan.processable == 0 {
            self.set_status("No hay imágenes procesables con esta configuración");
            return;
        }
        let options = self.options.clone();
        let huge = plan.items.iter().any(|item| {
            item.skip.is_none()
                && (item.source.probe.width as u64 * item.source.probe.height as u64) > 20_000_000
        });
        let workers = if huge {
            self.config.workers.clamp(1, 2)
        } else {
            self.config.workers.max(1)
        };
        let policy = match self.destination.conflict {
            ConflictPolicy::Ask => ConflictPolicy::Skip,
            other => other,
        };
        let journal = Arc::new(Journal::new(config::journal_path()));
        let (tx, rx) = crossbeam_channel::bounded(1024);
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_for_job = Arc::clone(&cancel);
        let job = JobOptions {
            workers,
            conflict_policy: policy,
            journal: Some(journal),
        };
        let plan_for_job = plan.clone();
        let options_for_job = options.clone();
        let handle = std::thread::spawn(move || {
            jobs::run(&plan_for_job, &options_for_job, &job, tx, cancel_for_job);
        });
        self.running = Some(RunningState {
            rx,
            cancel,
            started: Instant::now(),
            summary: Summary {
                found: plan.items.len(),
                ..Default::default()
            },
            current_id: None,
            current_stage: Stage::Decode,
            failures: Vec::new(),
            handle: Some(handle),
        });
        self.summary = None;
        self.results.clear();
        self.failures_clear();
        self.quit_after_finish = false;
        self.screen = Screen::Running;
    }

    fn failures_clear(&mut self) {
        if let Some(running) = &mut self.running {
            running.failures.clear();
        }
    }

    fn handle_job_event(&mut self, event: JobEvent) {
        match event {
            JobEvent::Stage { id, stage } => {
                if let Some(running) = &mut self.running {
                    running.current_id = Some(id);
                    running.current_stage = stage;
                }
            }
            JobEvent::ItemFinished { id, result } => {
                if let ItemResult::Failed { message, .. } = &result {
                    let path = self
                        .plan
                        .as_ref()
                        .and_then(|plan| plan.items.iter().find(|item| item.source.id == id))
                        .map(|item| display_path(&item.source.path))
                        .unwrap_or_else(|| format!("archivo #{}", id));
                    if let Some(running) = &mut self.running {
                        running.failures.push((path, message.clone()));
                    }
                }
                let _ = id;
            }
            JobEvent::Totals { summary } => {
                if let Some(running) = &mut self.running {
                    running.summary = summary;
                }
            }
            JobEvent::Finished { summary, results } => {
                self.summary = Some(summary);
                self.results = results;
                self.write_report();
                self.finish_running();
                self.screen = Screen::Summary;
            }
        }
    }

    fn finish_running(&mut self) {
        if let Some(mut running) = self.running.take() {
            if let Some(handle) = running.handle.take() {
                let _ = handle.join();
            }
        }
        if self.quit_after_finish {
            self.should_quit = true;
        }
    }

    fn write_report(&mut self) {
        let (Some(plan), Some(summary)) = (&self.plan, &self.summary) else {
            return;
        };
        let path = match &plan.destination {
            Destination::Separate { root } => root.join("imgshrink-report.json"),
            _ => {
                let stamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|duration| duration.as_secs())
                    .unwrap_or(0);
                config::state_dir()
                    .join("reports")
                    .join(format!("imgshrink-report-{}.json", stamp))
            }
        };
        match report::write(&path, plan, &self.options, summary, &self.results) {
            Ok(()) => {
                self.report_path = Some(path);
                self.report_error = None;
            }
            Err(error) => {
                self.report_path = None;
                self.report_error = Some(error.to_string());
            }
        }
    }

    fn cancel_job(&mut self) {
        if let Some(running) = &mut self.running {
            running.cancel.store(true, Ordering::Relaxed);
            self.set_status("Cancelando… se terminará la imagen en curso");
        }
    }

    fn request_quit(&mut self) {
        if self.running.is_some() {
            self.quit_after_finish = true;
            self.cancel_job();
        } else {
            self.should_quit = true;
        }
    }

    fn reset_for_new(&mut self) {
        let cwd = self.source.cwd.clone();
        let mut source = SourceScreen::new(Some(cwd));
        source.recursive = false;
        self.source = source;
        self.destination = DestinationScreen::new(&self.source);
        self.plan = None;
        self.discovered.clear();
        self.extra_warnings.clear();
        self.summary = None;
        self.results.clear();
        self.report_path = None;
        self.report_error = None;
        self.warning_scroll = 0;
        self.screen = Screen::Welcome;
        self.welcome_index = 0;
        self.purpose_index = 0;
        self.purpose = Purpose::Share;
        self.options = OptimizeOptions::for_purpose(Purpose::Share);
    }

    fn open_output(&mut self) {
        let Some(plan) = &self.plan else {
            return;
        };
        let target = match &plan.destination {
            Destination::Separate { root } => Some(root.clone()),
            _ => plan
                .items
                .first()
                .and_then(|item| item.source.path.parent().map(|parent| parent.to_path_buf())),
        };
        let Some(target) = target else {
            self.set_status("No se pudo determinar la carpeta de salida");
            return;
        };
        let result = if cfg!(windows) {
            std::process::Command::new("explorer").arg(&target).spawn()
        } else if cfg!(target_os = "macos") {
            std::process::Command::new("open").arg(&target).spawn()
        } else {
            std::process::Command::new("xdg-open").arg(&target).spawn()
        };
        match result {
            Ok(_) => self.set_status(format!("Abriendo {}", display_path(&target))),
            Err(error) => self.set_status(format!("No se pudo abrir la carpeta: {}", error)),
        }
    }

    pub fn current_file(&self) -> Option<&SourceItem> {
        let running = self.running.as_ref()?;
        let id = running.current_id?;
        self.plan
            .as_ref()?
            .items
            .iter()
            .find(|item| item.source.id == id)
            .map(|item| &item.source)
    }

    pub fn running_eta(&self) -> Option<Duration> {
        let running = self.running.as_ref()?;
        let done = running.summary.processed();
        if done < 3 {
            return None;
        }
        let remaining = running.summary.found.saturating_sub(done);
        if remaining == 0 {
            return None;
        }
        let elapsed = running.started.elapsed().as_secs_f64();
        Some(Duration::from_secs_f64(
            elapsed / done as f64 * remaining as f64,
        ))
    }

    pub fn failures(&self) -> &[(String, String)] {
        self.running
            .as_ref()
            .map(|running| running.failures.as_slice())
            .unwrap_or(&[])
    }

    pub fn stage_label(&self) -> &'static str {
        self.running
            .as_ref()
            .map(|running| running.current_stage.label())
            .unwrap_or("Preparando")
    }

    pub fn spinner_frame(&self) -> &'static str {
        if self.ascii {
            const FRAMES: [&str; 4] = ["|", "/", "-", "\\"];
            FRAMES[self.spinner % FRAMES.len()]
        } else {
            const FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
            FRAMES[self.spinner % FRAMES.len()]
        }
    }

    pub fn status_text(&self) -> Option<&str> {
        self.status.as_ref().map(|(message, _)| message.as_str())
    }

    /// Entrada de teclado para pruebas de integración de la interfaz.
    #[doc(hidden)]
    pub fn test_key(&mut self, code: KeyCode) {
        self.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
    }

    #[doc(hidden)]
    pub fn test_paste(&mut self, text: &str) {
        self.handle_paste(text.to_string());
    }
}

fn text_insert(value: &mut String, cursor: &mut usize, character: char) {
    let mut characters: Vec<char> = value.chars().collect();
    let position = (*cursor).min(characters.len());
    characters.insert(position, character);
    *value = characters.into_iter().collect();
    *cursor = position + 1;
}

fn paste_text(value: &mut String, cursor: &mut usize, text: &str) {
    let mut characters: Vec<char> = value.chars().collect();
    let position = (*cursor).min(characters.len());
    let insert: Vec<char> = text.chars().collect();
    let count = insert.len();
    characters.splice(position..position, insert);
    *value = characters.into_iter().collect();
    *cursor = position + count;
}

fn text_backspace(value: &mut String, cursor: &mut usize) {
    if *cursor == 0 {
        return;
    }
    let mut characters: Vec<char> = value.chars().collect();
    let position = (*cursor).min(characters.len()) - 1;
    characters.remove(position);
    *value = characters.into_iter().collect();
    *cursor = position;
}

pub fn truncate(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if text.chars().count() <= width {
        return text.to_string();
    }
    if width <= 1 {
        return "…".into();
    }
    let mut result: String = text.chars().take(width - 1).collect();
    result.push('…');
    result
}

pub fn bytes_label(bytes: u64) -> String {
    format_bytes(bytes)
}
