use imgshrink_core::jobs::ItemResult;
use imgshrink_core::pathutil::{display_path, format_bytes, format_duration};
use imgshrink_core::{Purpose, APP_AUTHOR, APP_LICENSE, APP_VERSION, APP_WEBSITE, REPO_URL};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Gauge, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{truncate, App, InputTarget, Overlay, Screen};
use crate::theme::Theme;

pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    if area.width < 50 || area.height < 14 {
        draw_too_small(frame, area, app);
        return;
    }
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(6),
            Constraint::Length(2),
        ])
        .split(area);
    draw_header(frame, chunks[0], app);
    match app.screen {
        Screen::Welcome => draw_welcome(frame, chunks[1], app),
        Screen::Purpose => draw_purpose(frame, chunks[1], app),
        Screen::Source => draw_source(frame, chunks[1], app),
        Screen::Analysis => draw_analysis(frame, chunks[1], app),
        Screen::Destination => draw_destination(frame, chunks[1], app),
        Screen::Review => draw_review(frame, chunks[1], app),
        Screen::Running => draw_running(frame, chunks[1], app),
        Screen::Summary => draw_summary(frame, chunks[1], app),
    }
    draw_footer(frame, chunks[2], app);
    if let Some(overlay) = &app.overlay {
        draw_overlay(frame, area, app, overlay);
    }
}

fn draw_too_small(frame: &mut Frame, area: Rect, app: &App) {
    let text = format!(
        "La terminal es demasiado pequeña ({}\u{00D7}{}).\nSe necesitan al menos 50\u{00D7}14 columnas.\nAmplía la ventana o pulsa q para salir.",
        area.width, area.height
    );
    let paragraph = Paragraph::new(text)
        .style(Style::default().fg(app.theme.warning).bg(app.theme.bg))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

fn panel<'a>(title: &'a str, theme: Theme) -> Block<'a> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border))
        .title(Line::from(Span::styled(
            format!(" {} ", title),
            Style::default()
                .fg(theme.muted)
                .add_modifier(Modifier::BOLD),
        )))
        .style(Style::default().bg(theme.surface))
}

fn selected_panel<'a>(title: &'a str, theme: Theme, selected: bool) -> Block<'a> {
    let block = panel(title, theme);
    if selected {
        block.border_style(
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        block
    }
}

fn draw_header(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(app.theme.border));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let left = if app.ascii {
        " IMGSHRINK  ·  optimizador de imágenes".to_string()
    } else {
        " ◈ IMGSHRINK  ·  optimizador de imágenes".to_string()
    };
    let right = format!("{}  ", app.screen.step());
    let width = inner.width as usize;
    let pad = width
        .saturating_sub(left.chars().count())
        .saturating_sub(right.chars().count());
    let line = Line::from(vec![
        Span::styled(
            left,
            Style::default()
                .fg(app.theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" ".repeat(pad)),
        Span::styled(right, Style::default().fg(app.theme.muted)),
    ]);
    frame.render_widget(Paragraph::new(line), inner);
}

fn draw_footer(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(app.theme.border));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let hints = match app.screen {
        Screen::Welcome => "↑↓ mover · Enter elegir · ? ayuda · q salir",
        Screen::Purpose => "↑↓ elegir · Enter continuar · Esc volver",
        Screen::Source => {
            "Espacio marcar · Enter abrir · a usar carpeta · r subcarpetas · p continuar · e ruta"
        }
        Screen::Analysis => "Enter destino · ↑↓ avisos · Esc volver",
        Screen::Destination => "↑↓ modo · e editar · c conflictos · Enter revisar",
        Screen::Review => "o opciones · Enter procesar · ↑↓ avisos · Esc volver",
        Screen::Running => "c cancelar · e errores · q salir al terminar",
        Screen::Summary => "o abrir carpeta · n otra operación · q salir",
    };
    let line = match app.status_text() {
        Some(status) => Line::from(vec![
            Span::styled(" · ", Style::default().fg(app.theme.muted)),
            Span::styled(
                truncate(status, inner.width.saturating_sub(4) as usize),
                Style::default().fg(app.theme.accent),
            ),
        ]),
        None => Line::from(Span::styled(
            format!(" {}", hints),
            Style::default().fg(app.theme.muted),
        )),
    };
    frame.render_widget(Paragraph::new(line), inner);
}

fn draw_welcome(frame: &mut Frame, area: Rect, app: &App) {
    let width = area.width.min(76);
    let height = area.height.min(22);
    let content = centered(area, width, height);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(3),
            Constraint::Length(2),
            Constraint::Length(6),
            Constraint::Min(3),
        ])
        .split(content);

    let logo = if app.ascii {
        vec![
            Line::from(Span::styled(
                "I M G S H R I N K",
                Style::default()
                    .fg(app.theme.accent)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                "optimizador de imágenes local y portable",
                Style::default().fg(app.theme.muted),
            )),
        ]
    } else {
        vec![
            Line::from(Span::styled(
                "◈  I M G S H R I N K",
                Style::default()
                    .fg(app.theme.accent)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                "optimizador de imágenes local y portable",
                Style::default().fg(app.theme.muted),
            )),
        ]
    };
    frame.render_widget(Paragraph::new(logo).alignment(Alignment::Center), rows[0]);

    let version = vec![
        Line::from(vec![
            Span::styled(
                format!("v{}", APP_VERSION),
                Style::default().fg(app.theme.text),
            ),
            Span::styled(
                "   ·   JPEG · PNG · WebP",
                Style::default().fg(app.theme.muted),
            ),
        ]),
        Line::from(Span::styled(
            format!("por {} · {} · {}", APP_AUTHOR, APP_WEBSITE, APP_LICENSE),
            Style::default().fg(app.theme.accent),
        )),
    ];
    frame.render_widget(
        Paragraph::new(version).alignment(Alignment::Center),
        rows[1],
    );

    let subtitle = Line::from(Span::styled(
        "Optimiza por lotes sin subir nada a Internet.",
        Style::default().fg(app.theme.text),
    ));
    frame.render_widget(
        Paragraph::new(subtitle).alignment(Alignment::Center),
        rows[2],
    );

    let menu = ["Comenzar", "Ayuda", "Configuración", "Salir"];
    let menu_block = centered(
        Rect::new(rows[3].x, rows[3].y, rows[3].width, rows[3].height),
        34,
        5,
    );
    let lines: Vec<Line> = menu
        .iter()
        .enumerate()
        .map(|(index, label)| {
            if index == app.welcome_index {
                Line::from(vec![
                    Span::styled(
                        if app.ascii { " > " } else { " ▸ " },
                        Style::default()
                            .fg(app.theme.accent)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("{:<22}", label),
                        Style::default()
                            .fg(app.theme.accent)
                            .add_modifier(Modifier::BOLD),
                    ),
                ])
            } else {
                Line::from(vec![
                    Span::raw("   "),
                    Span::styled(
                        format!("{:<22}", label),
                        Style::default().fg(app.theme.text),
                    ),
                ])
            }
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), menu_block);

    let status_lines = vec![
        Line::from(Span::styled(
            "Originales intactos por defecto · Procesamiento local · Sin telemetría",
            Style::default().fg(app.theme.muted),
        )),
        Line::from(Span::styled(
            format!(
                "Motor: JPEG · PNG · WebP   |   {} trabajadores disponibles en la configuración",
                app.config.workers
            ),
            Style::default().fg(app.theme.muted),
        )),
        Line::from(Span::styled(
            if app.journal_pending > 0 {
                format!(
                    "Aviso: hay {} operación(es) inacabada(s) en el registro de recuperación.",
                    app.journal_pending
                )
            } else {
                "Consejo: pulsa ? en cualquier pantalla para ver la ayuda.".to_string()
            },
            Style::default().fg(if app.journal_pending > 0 {
                app.theme.warning
            } else {
                app.theme.muted
            }),
        )),
    ];
    frame.render_widget(
        Paragraph::new(status_lines)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true }),
        rows[4],
    );
}

fn draw_purpose(frame: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(12)])
        .split(area);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            " ¿Para qué vas a usar las imágenes?",
            Style::default()
                .fg(app.theme.text)
                .add_modifier(Modifier::BOLD),
        ))),
        chunks[0],
    );
    if chunks[1].height < 22 {
        draw_purpose_compact(frame, chunks[1], app);
        return;
    }
    let count = Purpose::ALL.len() as u16;
    let card_height = ((chunks[1].height.saturating_sub(2)) / count).max(3);
    let constraints: Vec<Constraint> = (0..count)
        .map(|_| Constraint::Length(card_height))
        .chain(std::iter::once(Constraint::Min(0)))
        .collect();
    let cards = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(chunks[1]);
    for (index, purpose) in Purpose::ALL.iter().enumerate() {
        let selected = index == app.purpose_index;
        let block = selected_panel(purpose.label(), app.theme, selected);
        let inner = block.inner(cards[index]);
        frame.render_widget(block, cards[index]);
        let marker = if selected {
            if app.ascii {
                "> "
            } else {
                "▸ "
            }
        } else {
            "  "
        };
        let lines = vec![
            Line::from(vec![
                Span::styled(
                    marker,
                    Style::default()
                        .fg(app.theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    purpose.label(),
                    Style::default()
                        .fg(if selected {
                            app.theme.accent
                        } else {
                            app.theme.text
                        })
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("   {}", purpose.tag()),
                    Style::default().fg(app.theme.muted),
                ),
            ]),
            Line::from(Span::styled(
                purpose.description(),
                Style::default().fg(app.theme.text),
            )),
            Line::from(Span::styled(
                profile_hint(*purpose),
                Style::default().fg(app.theme.muted),
            )),
        ];
        frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
    }
}

fn draw_purpose_compact(frame: &mut Frame, area: Rect, app: &App) {
    let mut lines = Vec::new();
    for (index, purpose) in Purpose::ALL.iter().enumerate() {
        let selected = index == app.purpose_index;
        let marker = if selected {
            if app.ascii {
                " > "
            } else {
                " ▸ "
            }
        } else {
            "   "
        };
        lines.push(Line::from(vec![
            Span::styled(
                marker,
                Style::default()
                    .fg(app.theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{:<18}", purpose.label()),
                Style::default()
                    .fg(if selected {
                        app.theme.accent
                    } else {
                        app.theme.text
                    })
                    .add_modifier(if selected {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            ),
            Span::styled(
                truncate(
                    purpose.description(),
                    area.width.saturating_sub(24) as usize,
                ),
                Style::default().fg(app.theme.muted),
            ),
        ]));
    }
    lines.push(Line::from(Span::styled(
        " Enter elige · ↑↓ mueve",
        Style::default().fg(app.theme.muted),
    )));
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), area);
}

fn profile_hint(purpose: Purpose) -> &'static str {
    match purpose {
        Purpose::Share => {
            "Salida: mismo formato · Calidad 82 (con pérdida en JPEG/WebP) · Sin redimensionar"
        }
        Purpose::SaveSpace => {
            "Salida: mismo formato · Sin pérdida en PNG/WebP · JPEG 90 · Sin redimensionar"
        }
        Purpose::Web => {
            "Salida: WebP · Calidad 80 · Sin redimensionar en v1 · Metadatos eliminados"
        }
        Purpose::Custom => "Salida y calidad configurables desde la revisión",
    }
}

fn draw_source(frame: &mut Frame, area: Rect, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(64), Constraint::Percentage(36)])
        .split(area);
    let block = panel("Explorar", app.theme);
    let inner = block.inner(chunks[0]);
    frame.render_widget(block, chunks[0]);

    let path_line = Line::from(vec![
        Span::styled(" ", Style::default()),
        Span::styled(
            truncate(
                &display_path(&app.source.cwd),
                inner.width.saturating_sub(2) as usize,
            ),
            Style::default().fg(app.theme.muted),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(path_line),
        Rect::new(inner.x, inner.y, inner.width, 1),
    );

    let list_area = Rect::new(
        inner.x,
        inner.y + 1,
        inner.width,
        inner.height.saturating_sub(1),
    );
    app.source.list_rect = list_area;
    if let Some(error) = &app.source.error {
        frame.render_widget(
            Paragraph::new(Span::styled(
                error.as_str(),
                Style::default().fg(app.theme.danger),
            ))
            .wrap(Wrap { trim: true }),
            list_area,
        );
    } else if app.source.entries.is_empty() {
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(Span::styled(
                    " No hay imágenes ni carpetas visibles.",
                    Style::default().fg(app.theme.muted),
                )),
                Line::from(Span::styled(
                    " Pulsa e para escribir otra ruta o h para mostrar archivos ocultos.",
                    Style::default().fg(app.theme.muted),
                )),
            ]),
            list_area,
        );
    } else {
        let rows = list_area.height as usize;
        let start = app
            .source
            .offset
            .min(app.source.entries.len().saturating_sub(1));
        let end = (start + rows).min(app.source.entries.len());
        let mut lines = Vec::new();
        for (index, entry) in app.source.entries[start..end].iter().enumerate() {
            let absolute_index = start + index;
            let selected = app.source.is_selected(&entry.path);
            let current = absolute_index == app.source.index;
            let marker = if current {
                if app.ascii {
                    ">"
                } else {
                    "▸"
                }
            } else {
                " "
            };
            let checkbox = if entry.is_dir {
                "DIR"
            } else if selected {
                "[x]"
            } else {
                "[ ]"
            };
            let icon = if entry.is_dir {
                if app.ascii {
                    "/"
                } else {
                    "▸"
                }
            } else if app.ascii {
                ""
            } else {
                "·"
            };
            let name_color = if entry.is_dir {
                app.theme.accent
            } else if selected {
                app.theme.success
            } else {
                app.theme.text
            };
            let suffix = if entry.is_dir { "/" } else { "" };
            let label = format!(
                " {marker} {checkbox} {icon} {name}{suffix}",
                marker = marker,
                checkbox = checkbox,
                icon = icon,
                name = truncate(&entry.name, 34),
                suffix = suffix
            );
            let size = if entry.is_dir {
                String::new()
            } else {
                format_bytes(entry.size)
            };
            let line = Line::from(vec![
                Span::styled(
                    label,
                    Style::default().fg(name_color).add_modifier(if current {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
                ),
                Span::styled(format!("  {}", size), Style::default().fg(app.theme.muted)),
            ]);
            let line = if current {
                line.style(Style::default().bg(app.theme.highlight))
            } else {
                line
            };
            lines.push(line);
        }
        frame.render_widget(Paragraph::new(lines), list_area);
    }

    let block = panel("Selección", app.theme);
    let inner = block.inner(chunks[1]);
    frame.render_widget(block, chunks[1]);
    let mut lines = vec![Line::from(vec![Span::styled(
        format!(
            " {} archivo(s) · {} carpeta(s)",
            app.source.selected_files.len(),
            app.source.selected_dirs.len()
        ),
        Style::default()
            .fg(app.theme.text)
            .add_modifier(Modifier::BOLD),
    )])];
    for file in app.source.selected_files.iter().take(6) {
        lines.push(Line::from(Span::styled(
            format!(
                "  · {}",
                truncate(&file.file_name().unwrap_or_default().to_string_lossy(), 30)
            ),
            Style::default().fg(app.theme.success),
        )));
    }
    for dir in app.source.selected_dirs.iter().take(4) {
        lines.push(Line::from(Span::styled(
            format!(
                "  ▸ {}/",
                truncate(&dir.file_name().unwrap_or_default().to_string_lossy(), 28)
            ),
            Style::default().fg(app.theme.accent),
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(" Subcarpetas: ", Style::default().fg(app.theme.muted)),
        Span::styled(
            if app.source.recursive { "sí" } else { "no" },
            Style::default().fg(if app.source.recursive {
                app.theme.warning
            } else {
                app.theme.text
            }),
        ),
        Span::styled("  (r)", Style::default().fg(app.theme.muted)),
    ]));
    lines.push(Line::from(Span::styled(
        if app.source.recursive {
            " Se recorrerán todos los subdirectorios."
        } else {
            " Solo el nivel actual de cada carpeta."
        },
        Style::default().fg(app.theme.muted),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " p  continuar al análisis",
        Style::default().fg(app.theme.accent),
    )));
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn draw_analysis(frame: &mut Frame, area: Rect, app: &App) {
    if let Some(error) = &app.analysis_error {
        let block = panel("Análisis", app.theme);
        let inner = block.inner(area);
        frame.render_widget(block, area);
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(Span::styled(
                    " No se pudo completar el análisis",
                    Style::default()
                        .fg(app.theme.danger)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    error.as_str(),
                    Style::default().fg(app.theme.text),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    " Esc vuelve a la selección.",
                    Style::default().fg(app.theme.muted),
                )),
            ])
            .wrap(Wrap { trim: true }),
            inner,
        );
        return;
    }
    let Some(plan) = &app.plan else {
        let block = panel("Analizando", app.theme);
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                format!("  {}  Analizando archivos…", app.spinner_frame()),
                Style::default()
                    .fg(app.theme.accent)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                format!(
                    "  {} selección(es) · {:.1} s transcurridos",
                    app.source.selections().len(),
                    app.analysis_started.elapsed().as_secs_f64()
                ),
                Style::default().fg(app.theme.muted),
            )),
            Line::from(Span::styled(
                "  Se leen cabeceras y metadatos; no se decodifican las imágenes completas.",
                Style::default().fg(app.theme.muted),
            )),
        ];
        frame.render_widget(Paragraph::new(lines), inner);
        return;
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(9), Constraint::Min(4)])
        .split(area);
    let block = panel("Resumen del análisis", app.theme);
    let inner = block.inner(chunks[0]);
    frame.render_widget(block, chunks[0]);
    let format = plan
        .items
        .first()
        .map(|item| app.options.effective_format(item.source.format).label())
        .unwrap_or("—");
    let lines = vec![
        Line::from(vec![
            Span::styled(" Encontradas: ", Style::default().fg(app.theme.muted)),
            Span::styled(
                format!("{}", plan.items.len()),
                Style::default()
                    .fg(app.theme.text)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("   Procesables: ", Style::default().fg(app.theme.muted)),
            Span::styled(
                format!("{}", plan.processable),
                Style::default().fg(if plan.processable == 0 {
                    app.theme.warning
                } else {
                    app.theme.success
                }),
            ),
            Span::styled("   Omitidas: ", Style::default().fg(app.theme.muted)),
            Span::styled(
                format!("{}", plan.skipped_at_plan()),
                Style::default().fg(app.theme.text),
            ),
        ]),
        Line::from(vec![
            Span::styled(" Tamaño total: ", Style::default().fg(app.theme.muted)),
            Span::styled(
                format_bytes(plan.total_input_bytes),
                Style::default().fg(app.theme.text),
            ),
            Span::styled("   Salida: ", Style::default().fg(app.theme.muted)),
            Span::styled(
                format!("{} · {}", format, app.destination.destination().label()),
                Style::default().fg(app.theme.text),
            ),
        ]),
        Line::from(vec![
            Span::styled(" Metadatos: ", Style::default().fg(app.theme.muted)),
            Span::styled(
                app.options.metadata.label(),
                Style::default().fg(app.theme.text),
            ),
            Span::styled("   Calidad: ", Style::default().fg(app.theme.muted)),
            Span::styled(
                if app.options.lossless {
                    "sin pérdida cuando el formato lo permite".to_string()
                } else {
                    format!("{}", app.options.quality)
                },
                Style::default().fg(app.theme.text),
            ),
        ]),
        Line::from(Span::styled(
            format!(
                " {} no compatibles/ignorados · {} con errores de lectura",
                app.analysis_skipped, app.analysis_unreadable
            ),
            Style::default().fg(app.theme.muted),
        )),
        Line::from(""),
        Line::from(Span::styled(
            " Enter elige el destino. Los originales no se tocarán salvo que elijas sobrescribir.",
            Style::default().fg(app.theme.accent),
        )),
    ];
    frame.render_widget(Paragraph::new(lines), inner);
    draw_warning_list(frame, chunks[1], app, plan.warnings.len(), |index| {
        plan.warnings.get(index).cloned()
    });
}

fn draw_warning_list<F>(frame: &mut Frame, area: Rect, app: &App, count: usize, getter: F)
where
    F: Fn(usize) -> Option<String>,
{
    let block = panel("Avisos", app.theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let rows = inner.height as usize;
    let start = (app.warning_scroll as usize).min(count.saturating_sub(1));
    let end = (start + rows).min(count);
    let mut lines = Vec::new();
    if count == 0 {
        lines.push(Line::from(Span::styled(
            " Sin avisos.",
            Style::default().fg(app.theme.muted),
        )));
    } else {
        for index in start..end {
            if let Some(message) = getter(index) {
                lines.push(Line::from(vec![
                    Span::styled(
                        if app.ascii { " ! " } else { " ⚠ " },
                        Style::default().fg(app.theme.warning),
                    ),
                    Span::styled(
                        truncate(&message, inner.width.saturating_sub(3) as usize),
                        Style::default().fg(app.theme.text),
                    ),
                ]));
            }
        }
        if count > rows {
            lines.push(Line::from(Span::styled(
                format!(" … {}/{} avisos (↑↓)", end, count),
                Style::default().fg(app.theme.muted),
            )));
        }
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
}

fn draw_destination(frame: &mut Frame, area: Rect, app: &App) {
    if area.height < 18 {
        return draw_destination_compact(frame, area, app);
    }
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(4),
            Constraint::Length(4),
            Constraint::Length(4),
            Constraint::Min(1),
        ])
        .split(area);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            " ¿Dónde quieres guardar los resultados?",
            Style::default()
                .fg(app.theme.text)
                .add_modifier(Modifier::BOLD),
        ))),
        chunks[0],
    );

    let cards = [
        (
            "Carpeta separada (recomendado)",
            format!(
                "Se conserva la estructura en: {}",
                display_path(&app.destination.root)
            ),
        ),
        (
            "Sobrescribir originales",
            "Requiere confirmación en dos pasos. Los originales se reemplazan.".to_string(),
        ),
        (
            "Junto al original con sufijo",
            format!(
                "Sufijo «{}» · la extensión depende del formato",
                app.destination.suffix
            ),
        ),
    ];
    for (index, (title, description)) in cards.iter().enumerate() {
        let selected = index == app.destination.mode;
        let block = selected_panel(title, app.theme, selected);
        let inner = block.inner(chunks[index + 1]);
        frame.render_widget(block, chunks[index + 1]);
        let lines = vec![
            Line::from(vec![
                Span::styled(
                    if selected {
                        if app.ascii {
                            "> "
                        } else {
                            "▸ "
                        }
                    } else {
                        "  "
                    },
                    Style::default().fg(if selected {
                        app.theme.accent
                    } else {
                        app.theme.muted
                    }),
                ),
                Span::styled(
                    description,
                    Style::default().fg(if selected {
                        app.theme.text
                    } else {
                        app.theme.muted
                    }),
                ),
            ]),
            Line::from(Span::styled(
                match index {
                    0 => " e  cambiar carpeta",
                    1 => " La confirmación se pedirá al continuar",
                    _ => " e  cambiar sufijo",
                },
                Style::default().fg(app.theme.muted),
            )),
        ];
        frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
    }
    let conflict = format!(
        " Conflictos: {}   ·   c para cambiar   ·   Enter revisar",
        app.destination.conflict.label()
    );
    let mut lines = vec![Line::from(Span::styled(
        conflict,
        Style::default().fg(app.theme.text),
    ))];
    if app.destination.mode == 1 {
        lines.push(Line::from(Span::styled(
            " Al sobrescribir se escribe un temporal, se valida y solo entonces se reemplaza.",
            Style::default().fg(app.theme.warning),
        )));
        lines.push(Line::from(Span::styled(
            " El cambio de formato no está permitido en este modo.",
            Style::default().fg(app.theme.muted),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            " Nunca se sobrescribe en silencio: la política elegida se aplica a todos.",
            Style::default().fg(app.theme.muted),
        )));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), chunks[4]);
}

fn draw_destination_compact(frame: &mut Frame, area: Rect, app: &App) {
    let cards = [
        (
            "Carpeta separada",
            format!("→ {}", display_path(&app.destination.root)),
        ),
        ("Sobrescribir originales", "doble confirmación".into()),
        (
            "Junto al original",
            format!("sufijo «{}»", app.destination.suffix),
        ),
    ];
    let mut lines = vec![Line::from(Span::styled(
        " ¿Dónde guardar?",
        Style::default()
            .fg(app.theme.text)
            .add_modifier(Modifier::BOLD),
    ))];
    for (index, (title, detail)) in cards.iter().enumerate() {
        let selected = index == app.destination.mode;
        lines.push(Line::from(vec![
            Span::styled(
                if selected {
                    if app.ascii {
                        " > "
                    } else {
                        " ▸ "
                    }
                } else {
                    "   "
                },
                Style::default().fg(app.theme.accent),
            ),
            Span::styled(
                format!("{:<24}", title),
                Style::default()
                    .fg(if selected {
                        app.theme.accent
                    } else {
                        app.theme.text
                    })
                    .add_modifier(if selected {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            ),
            Span::styled(
                truncate(detail, area.width.saturating_sub(30) as usize),
                Style::default().fg(app.theme.muted),
            ),
        ]));
    }
    lines.push(Line::from(Span::styled(
        format!(" Conflictos: {}", app.destination.conflict.label()),
        Style::default().fg(app.theme.text),
    )));
    lines.push(Line::from(Span::styled(
        " ↑↓ modo · e editar · c conflictos · Enter revisar",
        Style::default().fg(app.theme.muted),
    )));
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), area);
}

fn draw_review(frame: &mut Frame, area: Rect, app: &App) {
    let Some(plan) = &app.plan else {
        return;
    };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(10), Constraint::Min(4)])
        .split(area);
    let block = panel("Revisión de la operación", app.theme);
    let inner = block.inner(chunks[0]);
    frame.render_widget(block, chunks[0]);
    let mode = plan.destination.describe();
    let mut lines = vec![
        Line::from(vec![
            Span::styled(" Finalidad: ", Style::default().fg(app.theme.muted)),
            Span::styled(app.purpose.label(), Style::default().fg(app.theme.text)),
            Span::styled("   Imágenes: ", Style::default().fg(app.theme.muted)),
            Span::styled(
                format!("{} ({} procesables)", plan.items.len(), plan.processable),
                Style::default().fg(app.theme.text),
            ),
        ]),
        Line::from(vec![
            Span::styled(" Tamaño de entrada: ", Style::default().fg(app.theme.muted)),
            Span::styled(
                format_bytes(plan.total_input_bytes),
                Style::default().fg(app.theme.text),
            ),
            Span::styled("   Formato: ", Style::default().fg(app.theme.muted)),
            Span::styled(
                app.options.output_format.label(),
                Style::default().fg(app.theme.text),
            ),
            Span::styled(
                "   Calidad/compresión: ",
                Style::default().fg(app.theme.muted),
            ),
            Span::styled(
                if app.options.lossless {
                    "sin pérdida".to_string()
                } else {
                    format!("{}", app.options.quality)
                },
                Style::default().fg(app.theme.text),
            ),
        ]),
        Line::from(vec![
            Span::styled(" Redimensionado: ", Style::default().fg(app.theme.muted)),
            Span::styled(
                app.options
                    .max_dimension
                    .map(|max| format!("máx. {} px (sin ampliar)", max))
                    .unwrap_or_else(|| "no se cambia el tamaño".into()),
                Style::default().fg(app.theme.text),
            ),
            Span::styled("   Metadatos: ", Style::default().fg(app.theme.muted)),
            Span::styled(
                app.options.metadata.label(),
                Style::default().fg(app.theme.text),
            ),
        ]),
        Line::from(vec![
            Span::styled(" Destino: ", Style::default().fg(app.theme.muted)),
            Span::styled(mode, Style::default().fg(app.theme.text)),
        ]),
        Line::from(vec![
            Span::styled(" Conflictos: ", Style::default().fg(app.theme.muted)),
            Span::styled(
                app.destination.conflict.label(),
                Style::default().fg(app.theme.text),
            ),
            Span::styled(
                format!("   ·   {} omitidas de antemano", plan.skipped_at_plan()),
                Style::default().fg(app.theme.muted),
            ),
        ]),
    ];
    if plan.processable == 0 {
        lines.push(Line::from(Span::styled(
            " No hay imágenes procesables. Vuelve atrás y ajusta la selección o el formato.",
            Style::default().fg(app.theme.danger),
        )));
    } else if matches!(plan.destination, imgshrink_core::Destination::InPlace) {
        lines.push(Line::from(Span::styled(
            " Modo destructivo confirmado: los originales se reemplazarán solo tras validar cada resultado.",
            Style::default().fg(app.theme.warning),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            " Los originales se conservan. Al terminar se generará un informe verificable.",
            Style::default().fg(app.theme.accent),
        )));
    }
    lines.push(Line::from(Span::styled(
        " Enter procesa · Esc vuelve al destino",
        Style::default().fg(app.theme.muted),
    )));
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);

    draw_warning_list(frame, chunks[1], app, plan.warnings.len(), |index| {
        plan.warnings.get(index).cloned()
    });
}

fn draw_running(frame: &mut Frame, area: Rect, app: &App) {
    let Some(running) = &app.running else {
        return;
    };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(6),
            Constraint::Min(3),
        ])
        .split(area);

    let total = running.summary.found.max(1);
    let processed = running.summary.processed();
    let ratio = (processed as f64 / total as f64).clamp(0.0, 1.0);
    let gauge = Gauge::default()
        .block(panel("Progreso global", app.theme))
        .gauge_style(
            Style::default()
                .fg(app.theme.accent)
                .bg(app.theme.highlight)
                .add_modifier(Modifier::BOLD),
        )
        .ratio(ratio)
        .label(format!(
            "{}/{} · {:.0}%",
            processed,
            running.summary.found,
            ratio * 100.0
        ));
    frame.render_widget(gauge, chunks[0]);

    let mut stats = vec![
        Line::from(vec![
            Span::styled("   Éxitos: ", Style::default().fg(app.theme.muted)),
            Span::styled(
                format!("{}", running.summary.succeeded),
                Style::default().fg(app.theme.success),
            ),
            Span::styled("   Errores: ", Style::default().fg(app.theme.muted)),
            Span::styled(
                format!("{}", running.summary.failed),
                Style::default().fg(app.theme.danger),
            ),
            Span::styled("   Omitidas: ", Style::default().fg(app.theme.muted)),
            Span::styled(
                format!("{}", running.summary.skipped),
                Style::default().fg(app.theme.warning),
            ),
            Span::styled("   Canceladas: ", Style::default().fg(app.theme.muted)),
            Span::styled(
                format!("{}", running.summary.cancelled),
                Style::default().fg(app.theme.muted),
            ),
        ]),
        Line::from(vec![
            Span::styled("   Tiempo: ", Style::default().fg(app.theme.muted)),
            Span::styled(
                format_duration(running.started.elapsed()),
                Style::default().fg(app.theme.text),
            ),
            Span::styled("   Restante: ", Style::default().fg(app.theme.muted)),
            Span::styled(
                app.running_eta()
                    .map(format_duration)
                    .unwrap_or_else(|| "calculando…".into()),
                Style::default().fg(app.theme.text),
            ),
            Span::styled("   Datos: ", Style::default().fg(app.theme.muted)),
            Span::styled(
                format!(
                    "{} → {}",
                    format_bytes(running.summary.input_bytes),
                    format_bytes(running.summary.output_bytes)
                ),
                Style::default().fg(app.theme.text),
            ),
        ]),
    ];
    if running.cancel.load(std::sync::atomic::Ordering::Relaxed) {
        stats.push(Line::from(Span::styled(
            "   Cancelación solicitada: se termina la imagen en curso y no se inician más.",
            Style::default().fg(app.theme.warning),
        )));
    }
    frame.render_widget(Paragraph::new(stats), chunks[1]);

    let block = panel("Imagen actual", app.theme);
    let inner = block.inner(chunks[2]);
    frame.render_widget(block, chunks[2]);
    let current_lines =
        if let Some(item) = app.current_file() {
            vec![
            Line::from(vec![
                Span::styled(
                    format!(" {} ", app.spinner_frame()),
                    Style::default().fg(app.theme.accent).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    truncate(
                        &item
                            .path
                            .file_name()
                            .map(|name| name.to_string_lossy().to_string())
                            .unwrap_or_else(|| display_path(&item.path)),
                        inner.width.saturating_sub(12) as usize,
                    ),
                    Style::default().fg(app.theme.text).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(Span::styled(
                format!("   {}", display_path(&item.path)),
                Style::default().fg(app.theme.muted),
            )),
            Line::from(vec![
                Span::styled("   Etapa: ", Style::default().fg(app.theme.muted)),
                Span::styled(
                    app.stage_label(),
                    Style::default().fg(app.theme.accent).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("   ·   {} → {}", item.format.label(), format_bytes(item.size)),
                    Style::default().fg(app.theme.muted),
                ),
            ]),
            Line::from(Span::styled(
                "   El progreso por imagen es por etapas: el códec no expone porcentajes internos.",
                Style::default().fg(app.theme.muted),
            )),
        ]
        } else {
            vec![Line::from(Span::styled(
                " Preparando el primer archivo…",
                Style::default().fg(app.theme.muted),
            ))]
        };
    frame.render_widget(
        Paragraph::new(current_lines).wrap(Wrap { trim: true }),
        inner,
    );

    let block = panel("Incidencias", app.theme);
    let inner = block.inner(chunks[3]);
    frame.render_widget(block, chunks[3]);
    let failures = app.failures();
    let mut lines = Vec::new();
    if failures.is_empty() {
        lines.push(Line::from(Span::styled(
            " Sin errores por ahora. Pulsa e para ver el detalle cuando haya incidencias.",
            Style::default().fg(app.theme.muted),
        )));
    } else {
        for (path, message) in failures
            .iter()
            .rev()
            .take(inner.height.saturating_sub(1) as usize)
        {
            lines.push(Line::from(vec![
                Span::styled(" ✕ ", Style::default().fg(app.theme.danger)),
                Span::styled(truncate(path, 36), Style::default().fg(app.theme.text)),
                Span::styled(
                    format!("  {}", truncate(message, 60)),
                    Style::default().fg(app.theme.muted),
                ),
            ]));
        }
        lines.push(Line::from(Span::styled(
            format!(" {} error(es) · e abre la lista completa", failures.len()),
            Style::default().fg(app.theme.muted),
        )));
    }
    frame.render_widget(Paragraph::new(lines), inner);
}

fn draw_summary(frame: &mut Frame, area: Rect, app: &App) {
    let Some(summary) = &app.summary else {
        frame.render_widget(
            Paragraph::new(" Aún no hay resultados.").style(Style::default().fg(app.theme.muted)),
            area,
        );
        return;
    };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Length(8),
            Constraint::Min(4),
        ])
        .split(area);
    let saved = summary.saved_bytes();
    let percent = summary.saved_percent();
    let (headline, color) = if saved >= 0 {
        (
            format!(" Ahorro: {}  ({:.1}%)", format_bytes(saved as u64), percent),
            app.theme.success,
        )
    } else {
        (
            format!(
                " Aumento: {}  ({:.1}%)",
                format_bytes((-saved) as u64),
                percent.abs()
            ),
            app.theme.warning,
        )
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                headline.replace('.', ","),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                " Solo se comparan las imágenes procesadas correctamente.",
                Style::default().fg(app.theme.muted),
            )),
        ]),
        chunks[0],
    );

    let block = panel("Resultados", app.theme);
    let inner = block.inner(chunks[1]);
    frame.render_widget(block, chunks[1]);
    let lines = vec![
        Line::from(vec![
            Span::styled(" Encontradas: ", Style::default().fg(app.theme.muted)),
            Span::styled(
                format!("{}", summary.found),
                Style::default().fg(app.theme.text),
            ),
            Span::styled("   Correctas: ", Style::default().fg(app.theme.muted)),
            Span::styled(
                format!("{}", summary.succeeded),
                Style::default().fg(app.theme.success),
            ),
            Span::styled("   Omitidas: ", Style::default().fg(app.theme.muted)),
            Span::styled(
                format!("{}", summary.skipped),
                Style::default().fg(app.theme.warning),
            ),
            Span::styled("   Errores: ", Style::default().fg(app.theme.muted)),
            Span::styled(
                format!("{}", summary.failed),
                Style::default().fg(app.theme.danger),
            ),
            Span::styled("   Canceladas: ", Style::default().fg(app.theme.muted)),
            Span::styled(
                format!("{}", summary.cancelled),
                Style::default().fg(app.theme.muted),
            ),
        ]),
        Line::from(vec![
            Span::styled(" Original: ", Style::default().fg(app.theme.muted)),
            Span::styled(
                format_bytes(summary.input_bytes),
                Style::default().fg(app.theme.text),
            ),
            Span::styled("   Resultante: ", Style::default().fg(app.theme.muted)),
            Span::styled(
                format_bytes(summary.output_bytes),
                Style::default().fg(app.theme.text),
            ),
            Span::styled("   Tiempo: ", Style::default().fg(app.theme.muted)),
            Span::styled(
                format_duration(std::time::Duration::from_secs_f64(summary.elapsed_secs)),
                Style::default().fg(app.theme.text),
            ),
        ]),
        Line::from(vec![
            Span::styled(" Salida: ", Style::default().fg(app.theme.muted)),
            Span::styled(
                app.plan
                    .as_ref()
                    .map(|plan| match &plan.destination {
                        imgshrink_core::Destination::Separate { root } => display_path(root),
                        imgshrink_core::Destination::InPlace => "originales reemplazados".into(),
                        imgshrink_core::Destination::Suffix { .. } => {
                            "junto a los originales".into()
                        }
                    })
                    .unwrap_or_else(|| "—".into()),
                Style::default().fg(app.theme.text),
            ),
        ]),
        Line::from(vec![
            Span::styled(" Informe: ", Style::default().fg(app.theme.muted)),
            Span::styled(
                app.report_path
                    .as_ref()
                    .map(|path| display_path(path))
                    .unwrap_or_else(|| {
                        app.report_error
                            .clone()
                            .unwrap_or_else(|| "no generado".into())
                    }),
                Style::default().fg(if app.report_path.is_some() {
                    app.theme.text
                } else {
                    app.theme.danger
                }),
            ),
        ]),
    ];
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);

    let mut actions = vec![
        Line::from(Span::styled(
            " o  abrir la carpeta de salida",
            Style::default().fg(app.theme.accent),
        )),
        Line::from(Span::styled(
            " n  nueva operación",
            Style::default().fg(app.theme.accent),
        )),
        Line::from(Span::styled(
            " q  salir",
            Style::default().fg(app.theme.accent),
        )),
    ];
    if !app.results.is_empty() {
        let failed = app
            .results
            .iter()
            .filter(|(_, result)| matches!(result, ItemResult::Failed { .. }))
            .count();
        if failed > 0 {
            actions.push(Line::from(Span::styled(
                format!(
                    " {} archivo(s) fallaron; revisa el informe para el detalle.",
                    failed
                ),
                Style::default().fg(app.theme.danger),
            )));
        }
    }
    frame.render_widget(Paragraph::new(actions), chunks[2]);
}

fn draw_overlay(frame: &mut Frame, area: Rect, app: &App, overlay: &Overlay) {
    frame.render_widget(Clear, area);
    frame.render_widget(
        Block::default().style(Style::default().bg(Color::Rgb(5, 8, 12))),
        area,
    );
    match overlay {
        Overlay::Help { scroll } => {
            let width = area.width.saturating_sub(6).min(82);
            let height = area.height.saturating_sub(4).min(26);
            let rect = centered(area, width, height);
            frame.render_widget(Clear, rect);
            let block = panel("Ayuda · ? o Esc para cerrar", app.theme);
            let inner = block.inner(rect);
            frame.render_widget(block, rect);
            let lines = help_lines(app);
            let start = (*scroll as usize).min(lines.len().saturating_sub(1));
            let visible: Vec<Line> = lines
                .into_iter()
                .skip(start)
                .take(inner.height as usize)
                .collect();
            frame.render_widget(Paragraph::new(visible).wrap(Wrap { trim: false }), inner);
        }
        Overlay::Settings => {
            let rect = centered(area, 66.min(area.width), 18.min(area.height));
            frame.render_widget(Clear, rect);
            let block = panel("Configuración", app.theme);
            let inner = block.inner(rect);
            frame.render_widget(block, rect);
            let lines = vec![
                Line::from(vec![
                    Span::styled(" Trabajadores: ", Style::default().fg(app.theme.muted)),
                    Span::styled(
                        format!("{}", app.config.workers),
                        Style::default()
                            .fg(app.theme.accent)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled("   (+ / -)", Style::default().fg(app.theme.muted)),
                ]),
                Line::from(vec![
                    Span::styled(" Iconos ASCII: ", Style::default().fg(app.theme.muted)),
                    Span::styled(
                        if app.config.ascii { "sí" } else { "no" },
                        Style::default().fg(app.theme.text),
                    ),
                    Span::styled("   (a)", Style::default().fg(app.theme.muted)),
                ]),
                Line::from(vec![
                    Span::styled(
                        " Guardar solo si es menor: ",
                        Style::default().fg(app.theme.muted),
                    ),
                    Span::styled(
                        if app.config.keep_if_larger {
                            "sí"
                        } else {
                            "no"
                        },
                        Style::default().fg(app.theme.text),
                    ),
                    Span::styled("   (k)", Style::default().fg(app.theme.muted)),
                ]),
                Line::from(vec![
                    Span::styled(" Ratón: ", Style::default().fg(app.theme.muted)),
                    Span::styled(
                        if app.mouse { "activado" } else { "desactivado" },
                        Style::default().fg(app.theme.text),
                    ),
                    Span::styled(
                        "   (se configura al iniciar)",
                        Style::default().fg(app.theme.muted),
                    ),
                ]),
                Line::from(""),
                Line::from(Span::styled(
                    format!(
                        " Configuración: {}",
                        display_path(&imgshrink_core::config::config_path())
                    ),
                    Style::default().fg(app.theme.muted),
                )),
                Line::from(Span::styled(
                    format!(
                        " Estado y registros: {}",
                        display_path(&imgshrink_core::config::state_dir())
                    ),
                    Style::default().fg(app.theme.muted),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    " s guardar · Esc cerrar",
                    Style::default().fg(app.theme.accent),
                )),
            ];
            frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
        }
        Overlay::ConfirmOverwrite {
            stage,
            choice,
            typed,
            cursor,
            error,
        } => {
            let rect = centered(area, 68.min(area.width), 14.min(area.height));
            frame.render_widget(Clear, rect);
            let block = panel("Sobrescribir originales", app.theme);
            let inner = block.inner(rect);
            frame.render_widget(block, rect);
            let mut lines = vec![
                Line::from(Span::styled(
                    " Los archivos originales serán reemplazados.",
                    Style::default()
                        .fg(app.theme.danger)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    format!(
                        " Se reemplazarán {} archivo(s) ({}).",
                        app.plan.as_ref().map(|plan| plan.processable).unwrap_or(0),
                        app.plan
                            .as_ref()
                            .map(|plan| {
                                let bytes = plan
                                    .items
                                    .iter()
                                    .filter(|item| item.skip.is_none())
                                    .map(|item| item.source.size)
                                    .sum();
                                format_bytes(bytes)
                            })
                            .unwrap_or_else(|| "0 B".into())
                    ),
                    Style::default().fg(app.theme.text),
                )),
                Line::from(Span::styled(
                    " No se puede deshacer salvo que tengas una copia propia.",
                    Style::default().fg(app.theme.muted),
                )),
                Line::from(""),
            ];
            if *stage == 1 {
                lines.push(Line::from(vec![
                    Span::styled(
                        if *choice == 0 { " ▶ " } else { "   " },
                        Style::default().fg(app.theme.accent),
                    ),
                    Span::styled(
                        "[ Cancelar ]",
                        Style::default().fg(if *choice == 0 {
                            app.theme.accent
                        } else {
                            app.theme.text
                        }),
                    ),
                    Span::raw("    "),
                    Span::styled(
                        if *choice == 1 { " ▶ " } else { "   " },
                        Style::default().fg(app.theme.accent),
                    ),
                    Span::styled(
                        "[ Continuar ]",
                        Style::default().fg(if *choice == 1 {
                            app.theme.danger
                        } else {
                            app.theme.text
                        }),
                    ),
                ]));
                lines.push(Line::from(Span::styled(
                    " ←→ elegir · Enter confirmar · Esc cancelar",
                    Style::default().fg(app.theme.muted),
                )));
            } else {
                lines.push(Line::from(Span::styled(
                    " Escribe SOBRESCRIBIR para habilitar la acción:",
                    Style::default().fg(app.theme.warning),
                )));
                lines.push(input_line(typed, *cursor, app.theme));
                if let Some(error) = error {
                    lines.push(Line::from(Span::styled(
                        format!(" {}", error),
                        Style::default().fg(app.theme.danger),
                    )));
                }
                lines.push(Line::from(Span::styled(
                    " Enter confirma solo si el texto coincide exactamente.",
                    Style::default().fg(app.theme.muted),
                )));
            }
            frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
        }
        Overlay::Errors { scroll } => {
            let rect = centered(area, 76.min(area.width), 22.min(area.height));
            frame.render_widget(Clear, rect);
            let block = panel("Errores e incidencias", app.theme);
            let inner = block.inner(rect);
            frame.render_widget(block, rect);
            let failures = app.failures();
            let mut lines = Vec::new();
            if failures.is_empty() {
                lines.push(Line::from(Span::styled(
                    " No hay errores registrados en esta operación.",
                    Style::default().fg(app.theme.muted),
                )));
            } else {
                for (index, (path, message)) in failures
                    .iter()
                    .enumerate()
                    .skip(*scroll as usize)
                    .take(inner.height as usize)
                {
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!(" {:>2}. ", index + 1),
                            Style::default().fg(app.theme.muted),
                        ),
                        Span::styled(truncate(path, 30), Style::default().fg(app.theme.text)),
                        Span::styled(
                            format!("  {}", message),
                            Style::default().fg(app.theme.danger),
                        ),
                    ]));
                }
            }
            frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
        }
        Overlay::Options { index, .. } => {
            let rect = centered(area, 76.min(area.width), 20.min(area.height));
            frame.render_widget(Clear, rect);
            let block = panel("Opciones de optimización", app.theme);
            let inner = block.inner(rect);
            frame.render_widget(block, rect);
            let format = app.options.output_format.label();
            let resize = app
                .options
                .max_dimension
                .map(|max| format!("máx. {} px", max))
                .unwrap_or_else(|| "sin cambios".into());
            let rows = [
                ("Formato de salida", format.to_string()),
                (
                    "Calidad",
                    if app.options.lossless {
                        "sin pérdida (no aplica)".into()
                    } else {
                        format!("{}", app.options.quality)
                    },
                ),
                (
                    "Compresión",
                    if app.options.lossless {
                        "sin pérdida".into()
                    } else {
                        "con pérdida".into()
                    },
                ),
                ("Redimensionar", resize),
                ("Metadatos", app.options.metadata.label().to_string()),
                (
                    "Guardar solo si es menor",
                    if app.options.keep_if_larger {
                        "sí"
                    } else {
                        "no"
                    }
                    .into(),
                ),
                ("Esfuerzo PNG", format!("{} / 6", app.options.png_effort)),
                ("Aplicar y volver", String::new()),
            ];
            let mut lines = Vec::new();
            for (row, (label, value)) in rows.iter().enumerate() {
                let selected = row == *index;
                let marker = if selected {
                    if app.ascii {
                        "> "
                    } else {
                        "▸ "
                    }
                } else {
                    "  "
                };
                let style = if selected {
                    Style::default()
                        .fg(app.theme.accent)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(app.theme.text)
                };
                lines.push(Line::from(vec![
                    Span::styled(marker, Style::default().fg(app.theme.accent)),
                    Span::styled(format!("{:<27}", label), style),
                    Span::styled(value.to_string(), Style::default().fg(app.theme.muted)),
                ]));
            }
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                " ←→ cambiar · ↑↓ fila · Enter aplicar · Esc descartar",
                Style::default().fg(app.theme.muted),
            )));
            frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
        }
        Overlay::TextInput {
            target,
            value,
            cursor,
            error,
        } => {
            let rect = centered(area, 72.min(area.width), 9.min(area.height));
            frame.render_widget(Clear, rect);
            let title = match target {
                InputTarget::SourcePath => "Ir a carpeta",
                InputTarget::SeparatePath => "Carpeta de salida",
                InputTarget::Suffix => "Sufijo de salida",
            };
            let block = panel(title, app.theme);
            let inner = block.inner(rect);
            frame.render_widget(block, rect);
            let mut lines = vec![
                Line::from(Span::styled(
                    match target {
                        InputTarget::SourcePath => " Escribe o pega una ruta de carpeta existente.",
                        InputTarget::SeparatePath => {
                            " Escribe o pega la carpeta donde se guardarán los resultados."
                        }
                        InputTarget::Suffix => " Ejemplo: -imgshrink → vacaciones-imgshrink.webp",
                    },
                    Style::default().fg(app.theme.muted),
                )),
                Line::from(""),
                input_line(value, *cursor, app.theme),
            ];
            if let Some(error) = error {
                lines.push(Line::from(Span::styled(
                    format!(" {}", error),
                    Style::default().fg(app.theme.danger),
                )));
            }
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                " Enter aplicar · Esc cancelar",
                Style::default().fg(app.theme.accent),
            )));
            frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
        }
    }
}

fn help_lines(app: &App) -> Vec<Line<'static>> {
    let accent = Style::default()
        .fg(app.theme.accent)
        .add_modifier(Modifier::BOLD);
    let muted = Style::default().fg(app.theme.muted);
    vec![
        Line::from(Span::styled("Flujo", accent)),
        Line::from(Span::styled(" 1. Finalidad  2. Selección  3. Análisis  4. Destino  5. Revisión  6. Proceso  7. Resumen", muted)),
        Line::from(""),
        Line::from(Span::styled("Navegación", accent)),
        Line::from(Span::styled(" ↑↓ o j/k mover · Enter abrir/elegir · Espacio marcar · Esc volver · Tab no se usa", muted)),
        Line::from(Span::styled(" ? ayuda · q salir · Ctrl+C cancelar/salir", muted)),
        Line::from(""),
        Line::from(Span::styled("Selección", accent)),
        Line::from(Span::styled(" e o / escribir ruta · a usar la carpeta actual · r subcarpetas · x limpiar", muted)),
        Line::from(Span::styled(" h mostrar ocultos · Retroceso subir de carpeta · p continuar", muted)),
        Line::from(""),
        Line::from(Span::styled("Destino y seguridad", accent)),
        Line::from(Span::styled(" Por defecto: carpeta separada y originales intactos.", muted)),
        Line::from(Span::styled(" Sobrescribir exige dos confirmaciones y la palabra SOBRESCRIBIR.", muted)),
        Line::from(Span::styled(" Cada resultado se escribe en un temporal, se valida y después se mueve.", muted)),
        Line::from(""),
        Line::from(Span::styled("Privacidad", accent)),
        Line::from(Span::styled(" Todo el procesamiento es local. No se envía ninguna imagen, ruta o estadística.", muted)),
        Line::from(Span::styled(" El informe se guarda junto a la salida o en el directorio de estado.", muted)),
        Line::from(""),
        Line::from(Span::styled("Acerca de", accent)),
        Line::from(Span::styled(
            format!(
                " ImgShrink {} · {} · {} · {}",
                APP_VERSION, APP_AUTHOR, APP_WEBSITE, APP_LICENSE
            ),
            muted,
        )),
        Line::from(Span::styled(
            format!(" Repositorio: {}", REPO_URL),
            muted,
        )),
        Line::from(""),
        Line::from(Span::styled("Límites de la v1", accent)),
        Line::from(Span::styled(" JPEG, PNG y WebP estáticos. AVIF, GIF animado y vídeo quedan fuera.", muted)),
        Line::from(Span::styled(" El progreso por imagen es por etapas; no se inventan porcentajes.", muted)),
    ]
}

fn input_line(value: &str, cursor: usize, theme: Theme) -> Line<'static> {
    let characters: Vec<char> = value.chars().collect();
    let position = cursor.min(characters.len());
    let before: String = characters[..position].iter().collect();
    let after: String = characters[position..].iter().collect();
    Line::from(vec![
        Span::styled(" ▏", Style::default().fg(theme.muted)),
        Span::styled(before, Style::default().fg(theme.text)),
        Span::styled(
            "▏",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(after, Style::default().fg(theme.text)),
    ])
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect::new(x, y, width, height)
}
