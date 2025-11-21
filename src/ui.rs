use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, TableState, Wrap};

use crate::app::{App, ConfirmKind, FormKind, Mode, StatusKind};
use crate::model::{Config, Host};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Copy)]
pub struct Theme {
    pub bg: Color,
    pub panel: Color,
    pub accent: Color,
    pub accent_dim: Color,
    pub warn: Color,
    pub error: Color,
    pub text: Color,
    pub muted: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            bg: Color::Rgb(8, 14, 24),
            panel: Color::Rgb(16, 24, 36),
            accent: Color::Rgb(70, 185, 200),
            accent_dim: Color::Rgb(60, 150, 140),
            warn: Color::Rgb(230, 185, 90),
            error: Color::Rgb(230, 110, 110),
            text: Color::Gray,
            muted: Color::DarkGray,
        }
    }
}

pub fn render(frame: &mut Frame, app: &App) {
    let theme = Theme::default();
    let size = frame.size();

    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Length(3),
                Constraint::Min(10),
                Constraint::Length(2),
            ]
            .as_ref(),
        )
        .split(size);

    render_header(frame, outer[0], app, theme);
    render_body(frame, outer[1], app, theme);
    render_status(frame, outer[2], app, theme);

    if let Some(confirm) = app.confirm.clone() {
        render_modal_confirm(frame, app, confirm, theme);
    }

    if let Some(form) = app.form.as_ref() {
        render_modal_form(frame, form, &app.config, theme);
    }

    if app.show_help {
        render_help(frame, theme);
    }

    if matches!(app.mode, Mode::QuickConnect) {
        render_quickconnect(frame, app, theme);
    }

    if app.show_about {
        render_about(frame, theme);
    }
}

fn render_header(frame: &mut Frame, area: Rect, app: &App, theme: Theme) {
    let header = Paragraph::new(Text::from(vec![Line::from(vec![
        Span::styled(
            format!(" sshdb v{} ", VERSION),
            Style::default()
                .fg(Color::Black)
                .bg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            format!("{} hosts", app.config.hosts.len()),
            Style::default().fg(theme.muted),
        ),
        Span::raw("    "),
        Span::styled(
            "Enter",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(": connect   "),
        Span::styled(
            "/",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(": search   "),
        Span::styled(
            "n",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(": new  "),
        Span::styled(
            "e",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(": edit  "),
        Span::styled(
            "d",
            Style::default().fg(theme.warn).add_modifier(Modifier::BOLD),
        ),
        Span::raw(": delete  "),
        Span::styled(
            "g",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(": quick connect  "),
        Span::styled(
            "u",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(": undo  "),
        Span::styled(
            "q",
            Style::default()
                .fg(theme.muted)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(": quit  "),
        Span::styled(
            "?",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(": help"),
    ])]))
    .block(
        Block::default()
            .borders(Borders::NONE)
            .style(Style::default().bg(theme.bg)),
    );
    frame.render_widget(header, area);
}

fn render_body(frame: &mut Frame, area: Rect, app: &App, theme: Theme) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(48), Constraint::Percentage(52)].as_ref())
        .split(area);

    render_list(frame, chunks[0], app, theme);
    render_details(frame, chunks[1], app, theme);
}

fn render_list(frame: &mut Frame, area: Rect, app: &App, theme: Theme) {
    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(5)].as_ref())
        .margin(0)
        .split(area);

    let search_block = Block::default()
        .borders(Borders::ALL)
        .border_style(
            Style::default()
                .fg(if matches!(app.mode, Mode::Search) {
                    theme.accent
                } else {
                    theme.accent_dim
                })
                .bg(theme.panel),
        )
        .title("search");

    let search_text = Paragraph::new(Line::from(vec![
        Span::styled("/", Style::default().fg(theme.muted)),
        Span::raw(" "),
        Span::styled(
            if app.filter.is_empty() {
                "type to filter".to_string()
            } else {
                app.filter.clone()
            },
            Style::default().fg(theme.text),
        ),
    ]))
    .style(Style::default().bg(theme.panel))
    .block(search_block);
    frame.render_widget(search_text, inner[0]);
    if matches!(app.mode, Mode::Search) {
        let cursor_x = inner[0].x + 1 + 2 + app.filter.len() as u16;
        let cursor_y = inner[0].y + 1;
        frame.set_cursor(cursor_x, cursor_y);
    }

    let rows: Vec<Row> = app
        .filtered_indices
        .iter()
        .map(|idx| {
            let host = &app.config.hosts[*idx];
            let tags = if host.tags.is_empty() {
                "∙".to_string()
            } else {
                host.tags.join(" ")
            };
            Row::new(vec![
                Cell::from(host.name.clone())
                    .style(Style::default().fg(theme.text).add_modifier(Modifier::BOLD)),
                Cell::from(host.display_label()).style(Style::default().fg(theme.muted)),
                Cell::from(tags).style(Style::default().fg(theme.accent_dim)),
            ])
        })
        .collect();

    let mut state = TableState::default();
    if !app.filtered_indices.is_empty() {
        state.select(Some(app.selected));
    }

    let header = Row::new(vec![
        Cell::from("name"),
        Cell::from("target"),
        Cell::from("tags"),
    ])
    .style(
        Style::default()
            .fg(Color::Rgb(6, 24, 32))
            .bg(theme.accent)
            .add_modifier(Modifier::BOLD),
    )
    .bottom_margin(1);

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(30),
            Constraint::Percentage(45),
            Constraint::Percentage(25),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("hosts")
            .border_style(Style::default().fg(theme.accent_dim))
            .style(Style::default().bg(theme.panel)),
    )
    .highlight_style(
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD),
    )
    .highlight_symbol("□ ")
    .column_spacing(2);

    frame.render_stateful_widget(table, inner[1], &mut state);
}

fn render_details(frame: &mut Frame, area: Rect, app: &App, theme: Theme) {
    let content = if let Some(host) = app.current_host() {
        build_details(host, app, theme)
    } else {
        Paragraph::new("No host selected")
            .style(Style::default().fg(theme.muted))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.accent))
                    .style(Style::default().bg(theme.panel))
                    .title("details"),
            )
    };

    frame.render_widget(content, area);
}

fn build_details<'a>(host: &'a Host, app: &'a App, theme: Theme) -> Paragraph<'a> {
    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(vec![
        Span::styled(
            &host.name,
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            host.description
                .clone()
                .unwrap_or_else(|| "no description".into()),
            Style::default().fg(theme.text),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("host", Style::default().fg(theme.muted)),
        Span::raw(": "),
        Span::styled(&host.address, Style::default().fg(theme.text)),
    ]));
    if let Some(user) = &host.user {
        lines.push(Line::from(vec![
            Span::styled("user", Style::default().fg(theme.muted)),
            Span::raw(": "),
            Span::styled(user, Style::default().fg(theme.text)),
        ]));
    }
    if let Some(port) = host.port {
        lines.push(Line::from(vec![
            Span::styled("port", Style::default().fg(theme.muted)),
            Span::raw(": "),
            Span::styled(port.to_string(), Style::default().fg(theme.text)),
        ]));
    }
    if let Some(key) = host.key_path.as_ref().or(app.config.default_key.as_ref()) {
        lines.push(Line::from(vec![
            Span::styled("key", Style::default().fg(theme.muted)),
            Span::raw(": "),
            Span::styled(key, Style::default().fg(theme.text)),
        ]));
    }
    if let Some(bastion) = &host.bastion {
        let bastion_display = if let Some(bh) = app.config.find_host(bastion) {
            format!("{} ({})", bastion, bh.display_label())
        } else {
            format!("{} (not found)", bastion)
        };
        lines.push(Line::from(vec![
            Span::styled("bastion", Style::default().fg(theme.muted)),
            Span::raw(": "),
            Span::styled(bastion_display, Style::default().fg(theme.accent_dim)),
        ]));
    }
    if let Some(rc) = &host.remote_command {
        lines.push(Line::from(vec![
            Span::styled("remote", Style::default().fg(theme.muted)),
            Span::raw(": "),
            Span::styled(rc, Style::default().fg(theme.text)),
        ]));
    }
    if !host.tags.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("tags", Style::default().fg(theme.muted)),
            Span::raw(": "),
            Span::styled(host.tags.join(", "), Style::default().fg(theme.accent_dim)),
        ]));
    }

    Paragraph::new(Text::from(lines))
        .style(Style::default().bg(theme.panel))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.accent))
                .title("details"),
        )
}

fn render_status(frame: &mut Frame, area: Rect, app: &App, theme: Theme) {
    let (text, color) = match &app.status {
        Some(status) => {
            let c = match status.kind {
                StatusKind::Info => theme.accent,
                StatusKind::Warn => theme.warn,
                StatusKind::Error => theme.error,
            };
            (status.text.clone(), c)
        }
        None => ("Ready".into(), theme.muted),
    };

    let msg = format!(
        "{}   config: {}   dry-run: {}",
        text,
        app.config_path.display(),
        if app.dry_run { "on" } else { "off" }
    );

    let paragraph = Paragraph::new(msg)
        .alignment(Alignment::Left)
        .style(Style::default().fg(color).bg(theme.bg))
        .block(Block::default().borders(Borders::NONE));
    frame.render_widget(paragraph, area);
}

fn render_modal_confirm(frame: &mut Frame, app: &App, confirm: ConfirmKind, theme: Theme) {
    let area = centered_rect_clamped(68, 9, frame.size());
    let title = match &confirm {
        ConfirmKind::Delete => "delete host?",
        ConfirmKind::Connect { .. } => "connect with optional remote cmd",
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent))
        .title(title)
        .style(Style::default().bg(theme.panel));

    let content = match confirm {
        ConfirmKind::Delete => Paragraph::new("Press y/Enter to delete, Esc to cancel.")
            .style(Style::default().fg(theme.warn))
            .block(block)
            .alignment(Alignment::Center),
        ConfirmKind::Connect { extra_cmd } => {
            let preview = app
                .current_host()
                .map(|h| {
                    crate::ssh::command_preview(
                        h,
                        &app.config,
                        app.config.default_key.as_deref(),
                        Some(&extra_cmd),
                    )
                })
                .unwrap_or_else(|| "ssh ...".to_string());
            let lines = vec![
                Line::from(vec![
                    Span::styled(
                        "Remote command (optional): ",
                        Style::default().fg(theme.muted),
                    ),
                    Span::styled(extra_cmd, Style::default().fg(theme.text)),
                ]),
                Line::from(vec![
                    Span::styled("Preview: ", Style::default().fg(theme.muted)),
                    Span::styled(preview, Style::default().fg(theme.accent)),
                ]),
                Line::from(vec![Span::styled(
                    "Enter to connect, Esc to cancel",
                    Style::default().fg(theme.muted),
                )]),
            ];
            Paragraph::new(Text::from(lines))
                .wrap(Wrap { trim: true })
                .block(block)
        }
    };
    frame.render_widget(Clear, area);
    frame.render_widget(content, area);
}

fn render_modal_form(frame: &mut Frame, form: &crate::app::FormState, config: &Config, theme: Theme) {
    // Increase height if bastion dropdown is open
    let base_height = 18;
    let dropdown_height = if form.bastion_dropdown.is_some() { 10 } else { 0 };
    let area = centered_rect_clamped(75, base_height + dropdown_height, frame.size());
    let title = match form.kind {
        FormKind::Add => "new host",
        FormKind::Edit => "edit host",
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent))
        .title(title)
        .style(Style::default().bg(theme.panel));

    let mut rows: Vec<Line> = Vec::new();
    let mut cursor: Option<(u16, u16)> = None;
    let has_command = matches!(form.kind, FormKind::Add);
    let mut line_no: usize = 0;

    if has_command {
        rows.push(Line::from(Span::styled(
            "SSH command",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )));
        line_no += 1;
        if let Some(f) = form.fields.get(0) {
            let active = form.index == 0;
            rows.push(Line::from(vec![
                Span::styled(
                    format!("{:>16}", f.label),
                    Style::default().fg(if active {
                        theme.accent
                    } else {
                        theme.accent_dim
                    }),
                ),
                Span::raw("  "),
                Span::styled(
                    if f.value.is_empty() {
                        " ".into()
                    } else {
                        f.value.clone()
                    },
                    Style::default().fg(theme.text).add_modifier(if active {
                        Modifier::UNDERLINED
                    } else {
                        Modifier::empty()
                    }),
                ),
            ]));
            if active {
                let x = area.x + 1 + 16 + 2 + f.cursor as u16;
                let y = area.y + 1 + line_no as u16;
                cursor = Some((x, y));
            }
            line_no += 1;
        }
        rows.push(Line::from(Span::styled(
            "─────────────────────────",
            Style::default().fg(theme.muted),
        )));
        rows.push(Line::from(Span::styled(
            "(or fill in the fields below)",
            Style::default().fg(theme.muted),
        )));
        rows.push(Line::from(Span::styled(
            "─────────────────────────",
            Style::default().fg(theme.muted),
        )));
        rows.push(Line::from(Span::styled(
            "Fields",
            Style::default().fg(theme.accent_dim),
        )));
        line_no += 4;
    }

    let start_idx = if has_command { 1 } else { 0 };
    let bastion_field_idx = if has_command { 6 } else { 5 };
    for (local_idx, f) in form.fields.iter().enumerate().skip(start_idx) {
        let active = form.index == local_idx;
        let prefix = if active { "▌" } else { " " };
        rows.push(Line::from(vec![
            Span::styled(
                format!("{prefix}{:>14}", f.label),
                Style::default().fg(if active {
                    theme.accent
                } else {
                    theme.accent_dim
                }),
            ),
            Span::raw("  "),
            Span::styled(
                if f.value.is_empty() {
                    " ".into()
                } else {
                    f.value.clone()
                },
                Style::default().fg(theme.text).add_modifier(if active {
                    Modifier::UNDERLINED
                } else {
                    Modifier::empty()
                }),
            ),
        ]));
        if active {
            let x = area.x + 1 + 1 + 14 + 2 + f.cursor as u16;
            let y = area.y + 1 + line_no as u16;
            cursor = Some((x, y));
        }
        line_no += 1;
        
        // Render bastion dropdown if this is the bastion field and dropdown is open
        if local_idx == bastion_field_idx && form.bastion_dropdown.is_some() {
            if let Some(dropdown) = &form.bastion_dropdown {
                rows.push(Line::from(Span::raw("")));
                line_no += 1;
                rows.push(Line::from(vec![
                    Span::styled(
                        "  Available hosts:",
                        Style::default().fg(theme.muted),
                    ),
                ]));
                line_no += 1;
                
                let max_items = 8.min(dropdown.filtered_indices.len());
                for i in 0..max_items {
                    if let Some(host_idx) = dropdown.filtered_indices.get(i) {
                        if let Some(host) = config.hosts.get(*host_idx) {
                            let is_selected = i == dropdown.selected;
                            let prefix = if is_selected { "  ► " } else { "    " };
                            rows.push(Line::from(vec![
                                Span::styled(
                                    prefix,
                                    Style::default().fg(if is_selected {
                                        theme.accent
                                    } else {
                                        theme.muted
                                    }),
                                ),
                                Span::styled(
                                    host.name.clone(),
                                    Style::default()
                                        .fg(if is_selected {
                                            theme.accent
                                        } else {
                                            theme.text
                                        })
                                        .add_modifier(if is_selected {
                                            Modifier::BOLD
                                        } else {
                                            Modifier::empty()
                                        }),
                                ),
                                Span::raw("  "),
                                Span::styled(
                                    format!("({})", host.display_label()),
                                    Style::default().fg(theme.muted),
                                ),
                            ]));
                            line_no += 1;
                        }
                    }
                }
                if dropdown.filtered_indices.len() > max_items {
                    rows.push(Line::from(vec![
                        Span::styled(
                            format!("  ... and {} more", dropdown.filtered_indices.len() - max_items),
                            Style::default().fg(theme.muted),
                        ),
                    ]));
                    line_no += 1;
                }
                rows.push(Line::from(vec![
                    Span::styled(
                        "  (↑↓ to navigate, Enter to select, Esc to close, Space to toggle)",
                        Style::default().fg(theme.muted),
                    ),
                ]));
                line_no += 1;
            }
        }
        
        // Show hint when bastion field is active but dropdown is closed
        if local_idx == bastion_field_idx && active && form.bastion_dropdown.is_none() {
            rows.push(Line::from(vec![
                Span::styled(
                    "  (Press Space to browse hosts)",
                    Style::default().fg(theme.muted),
                ),
            ]));
            line_no += 1;
        }
    }

    if !has_command {
        rows.push(Line::from(Span::raw("")));
        let preview = form
            .build_host()
            .ok()
            .map(|h| crate::ssh::command_preview(&h, config, None, None))
            .unwrap_or_else(|| "fill required fields for preview".into());
        rows.push(Line::from(Span::styled(
            "Command preview:",
            Style::default().fg(theme.muted),
        )));
        rows.push(Line::from(Span::styled(
            preview,
            Style::default().fg(theme.accent_dim),
        )));
    }

    let paragraph = Paragraph::new(Text::from(rows))
        .block(block)
        .wrap(Wrap { trim: false });
    frame.render_widget(Clear, area);
    frame.render_widget(paragraph, area);
    if let Some((x, y)) = cursor {
        frame.set_cursor(x, y);
    }
}

fn centered_rect_clamped(width: u16, height: u16, r: Rect) -> Rect {
    let w = width.min(r.width.saturating_sub(2));
    let h = height.min(r.height.saturating_sub(2));
    let x = r.x + (r.width.saturating_sub(w)) / 2;
    let y = r.y + (r.height.saturating_sub(h)) / 2;
    Rect {
        x,
        y,
        width: w,
        height: h,
    }
}

fn render_help(frame: &mut Frame, theme: Theme) {
    let area = centered_rect_clamped(78, 16, frame.size());
    let items: Vec<Line> = crate::app::App::help_entries()
        .iter()
        .map(|(k, v)| {
            Line::from(vec![
                Span::styled(format!("{:>15}", k), Style::default().fg(theme.accent)),
                Span::raw("  "),
                Span::styled(*v, Style::default().fg(theme.text)),
            ])
        })
        .collect();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent))
        .title("keys");
    let paragraph = Paragraph::new(Text::from(items))
        .style(Style::default().bg(theme.panel))
        .block(block);
    frame.render_widget(Clear, area);
    frame.render_widget(paragraph, area);
}

fn render_quickconnect(frame: &mut Frame, app: &App, theme: Theme) {
    let area = centered_rect_clamped(70, 8, frame.size());
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent))
        .title("quick connect");
    let input = app.quick_input.clone().unwrap_or_default();
    let content_start_x = area.x + 1;
    let content_start_y = area.y + 1;
    let prefix_len = 4u16; // "ssh "
    let cursor_x = content_start_x + prefix_len + app.quick_cursor.min(input.len()) as u16;
    let cursor_y = content_start_y + 2;

    let lines = vec![
        Line::from(Span::styled(
            "Paste ssh user@host (or full ssh command), Enter to connect. Esc to cancel.",
            Style::default().fg(theme.muted),
        )),
        Line::from(Span::raw("")),
        Line::from(vec![
            Span::styled("ssh ", Style::default().fg(theme.muted)),
            Span::styled(
                if input.is_empty() {
                    " "
                } else {
                    input.as_str()
                },
                Style::default()
                    .fg(theme.text)
                    .add_modifier(Modifier::UNDERLINED),
            ),
        ]),
    ];

    let paragraph = Paragraph::new(Text::from(lines))
        .style(Style::default().bg(theme.panel))
        .block(block);
    frame.render_widget(Clear, area);
    frame.render_widget(paragraph, area);
    frame.set_cursor(cursor_x, cursor_y);
}

fn render_about(frame: &mut Frame, theme: Theme) {
    let area = centered_rect_clamped(70, 10, frame.size());
    let lines = vec![
        Line::from(Span::styled(
            format!("sshdb v{}", VERSION),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "Keyboard-first SSH library and launcher TUI.",
            Style::default().fg(theme.text),
        )),
        Line::from(Span::styled(
            "Author: Riccardo Iaconelli <riccardo@kde.org>",
            Style::default().fg(theme.text),
        )),
        Line::from(Span::styled(
            "License: GPL-3.0-only",
            Style::default().fg(theme.text),
        )),
        Line::from(Span::styled(
            "Source: github.com/ruphy/sshdb",
            Style::default().fg(theme.accent_dim),
        )),
        Line::from(Span::styled(
            "Press Esc/q/a to close",
            Style::default().fg(theme.muted),
        )),
    ];
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent))
        .title("about");
    let paragraph = Paragraph::new(Text::from(lines))
        .style(Style::default().bg(theme.panel))
        .block(block)
        .alignment(Alignment::Left);
    frame.render_widget(Clear, area);
    frame.render_widget(paragraph, area);
}
