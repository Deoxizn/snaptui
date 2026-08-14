use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Cell, Clear, List, ListItem, Paragraph, Row, Table, Tabs};
use ratatui::{Frame, Terminal};

use std::io::Stdout;

use crate::app::{App, Popup, Tab, LIMIT_KEYS};
use crate::fmt::{format_date, human_size, snapshot_type};

pub fn draw(term: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut App) {
    let _ = term.draw(|f| draw_frame(f, app));
}

fn draw_frame(f: &mut Frame<'_>, app: &mut App) {
    let theme = &app.theme;
    let area = f.area();

    f.render_widget(
        Block::new().style(Style::default().bg(theme.bg)),
        area,
    );

    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(area);

    draw_header(f, app, chunks[0]);
    match app.tab {
        Tab::Snapshots => draw_snapshots(f, app, chunks[1]),
        Tab::Configs => draw_configs(f, app, chunks[1]),
        Tab::Limits => draw_limits(f, app, chunks[1]),
    }
    draw_status(f, app, chunks[2]);
    draw_footer(f, app, chunks[3]);

    if let Some(popup) = app.popup.clone() {
        draw_popup(f, app, &popup);
    }
}

fn title_span(app: &App) -> Line<'static> {
    let theme = &app.theme;
    let title = "snaptui";
    let version = "btrfs snapshot manager";
    let name = app
        .current_config()
        .map(|c| c.name.clone())
        .unwrap_or_else(|| "no config".into());
    let total = human_size(app.fs_total);
    let free = human_size(app.fs_free);
    let pct = if app.fs_total > 0 {
        ((app.fs_total - app.fs_free) as f64 / app.fs_total as f64 * 100.0) as u8
    } else {
        0
    };
    Line::from(vec![
        Span::styled(title, Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
        Span::styled("  —  ", Style::default().fg(theme.muted)),
        Span::styled(version, Style::default().fg(theme.muted)),
        Span::styled("  |  config: ", Style::default().fg(theme.muted)),
        Span::styled(name, Style::default().fg(theme.fg)),
        Span::styled("  |  disk: ", Style::default().fg(theme.muted)),
        Span::styled(format!("{free} free of {total} ({pct}% used)"), Style::default().fg(theme.cyan)),
    ])
}

fn draw_header(f: &mut Frame<'_>, app: &mut App, area: Rect) {
    let theme = &app.theme;
    let titles = ["Snapshots", "Configs", "Limits"];
    let selected = match app.tab {
        Tab::Snapshots => 0,
        Tab::Configs => 1,
        Tab::Limits => 2,
    };
    let tabs = Tabs::new(titles)
        .select(selected)
        .block(Block::new().borders(Borders::TOP).border_style(Style::default().fg(theme.muted)))
        .style(Style::default().fg(theme.muted))
        .highlight_style(Style::default().fg(theme.accent).add_modifier(Modifier::BOLD))
        .divider("|");
    let inner = Layout::vertical([Constraint::Length(1), Constraint::Length(2)]).split(area);
    f.render_widget(Paragraph::new(title_span(app)).style(Style::default().fg(theme.fg)), inner[0]);
    f.render_widget(tabs, inner[1]);
}

fn draw_snapshots(f: &mut Frame<'_>, app: &mut App, area: Rect) {
    let theme = &app.theme;
    let block = Block::bordered()
        .border_style(Style::default().fg(theme.muted))
        .title(Line::from(format!(
            " Snapshots ({}) — space=select · d=delete · c=create · e=edit · a=all · r=refresh ",
            app.snapshots.len()
        )));

    let widths = [
        Constraint::Length(4),
        Constraint::Length(7),
        Constraint::Length(16),
        Constraint::Min(20),
        Constraint::Length(10),
        Constraint::Length(6),
        Constraint::Length(8),
        Constraint::Length(10),
    ];

    let mut rows: Vec<Row> = Vec::with_capacity(app.snapshots.len());
    for snap in &app.snapshots {
        let selected = app.selected.contains(&snap.id);
        let used = app.used.get(&snap.id).copied();
        let size_cell = match used {
            Some(bytes) => human_size(bytes),
            None => "–".into(),
        };
        let owner = app
            .users
            .get(&snap.uid)
            .cloned()
            .unwrap_or_else(|| snap.uid.to_string());
        let cells = vec![
            Cell::from(snap.id.to_string()).style(Style::default().fg(theme.cyan)),
            Cell::from(snapshot_type(snap.kind))
                .style(Style::default().fg(match snap.kind {
                    0 => theme.fg,
                    1 => theme.warn,
                    _ => theme.ok,
                })),
            Cell::from(format_date(snap.date)).style(Style::default().fg(theme.muted)),
            Cell::from(if snap.description.is_empty() { "-" } else { &snap.description })
                .style(Style::default().fg(theme.fg)),
            Cell::from(if snap.cleanup.is_empty() { "-" } else { &snap.cleanup })
                .style(Style::default().fg(theme.muted)),
            Cell::from(owner).style(Style::default().fg(theme.muted)),
            Cell::from(size_cell).style(Style::default().fg(theme.cyan)),
        ];
        let row = Row::new(cells).style(if selected {
            Style::default().fg(theme.accent).add_modifier(Modifier::DIM)
        } else {
            Style::default()
        });
        rows.push(row);
    }

    let table = Table::new(rows, widths)
        .header(
            Row::new(vec![
                Cell::from(Span::styled("#", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD))),
                Cell::from(Span::styled("type", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD))),
                Cell::from(Span::styled("date", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD))),
                Cell::from(Span::styled("description", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD))),
                Cell::from(Span::styled("cleanup", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD))),
                Cell::from(Span::styled("uid", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD))),
                Cell::from(Span::styled("size", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD))),
            ]))
            .block(block)
            .row_highlight_style(Style::default().bg(theme.surface))
            .highlight_symbol("> ")
            .column_spacing(2);
    f.render_stateful_widget(table, area, &mut app.snapshot_state);
}

fn draw_configs(f: &mut Frame<'_>, app: &mut App, area: Rect) {
    let theme = &app.theme;
    let block = Block::bordered()
        .border_style(Style::default().fg(theme.muted))
        .title(format!(" Configs ({}) — enter=view snapshots ", app.configs.len()));

    let mut items: Vec<ListItem> = Vec::new();
    for (i, config) in app.configs.iter().enumerate() {
        let count = if i == app.config_idx {
            format!("  [{} snapshots]", app.snapshots.len())
        } else {
            String::new()
        };
        let allowed = config
            .values
            .get("ALLOW_USERS")
            .cloned()
            .unwrap_or_default();
        let mut spans = vec![
            Span::styled(&config.name, Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
            Span::styled("  subvol: ", Style::default().fg(theme.muted)),
            Span::styled(&config.subvolume, Style::default().fg(theme.fg)),
        ];
        if !allowed.is_empty() {
            spans.push(Span::styled(
                format!("  allowed: {allowed}"),
                Style::default().fg(theme.ok),
            ));
        }
        spans.push(Span::styled(count, Style::default().fg(theme.muted)));
        items.push(ListItem::new(Line::from(spans)));
    }

    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().bg(theme.surface).add_modifier(Modifier::BOLD));
    f.render_stateful_widget(list, area, &mut app.config_state);
}

fn draw_limits(f: &mut Frame<'_>, app: &mut App, area: Rect) {
    let theme = &app.theme;
    let dirty = app.limit_dirty.len();
    let block = Block::bordered()
        .border_style(Style::default().fg(theme.muted))
        .title(Line::from(format!(
            " Limits for {} — enter=edit · s=save({}) · x=discard · r=reload ",
            app.current_config().map(|c| c.name.clone()).unwrap_or_default(),
            dirty
        )));

    let mut items: Vec<ListItem> = Vec::new();
    for (key, editable) in LIMIT_KEYS {
        let value = app.current_limit_value(key);
        let is_dirty = app.limit_dirty.contains(key);
        let mut spans = vec![
            Span::styled(key, Style::default().fg(if editable { theme.cyan } else { theme.muted })),
            Span::styled("  =  ", Style::default().fg(theme.muted)),
            Span::styled(value, Style::default().fg(if is_dirty { theme.warn } else { theme.fg })),
        ];
        if is_dirty {
            spans.push(Span::styled("  [unsaved]", Style::default().fg(theme.warn)));
        }
        items.push(ListItem::new(Line::from(spans)));
    }

    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().bg(theme.surface).add_modifier(Modifier::BOLD));
    f.render_stateful_widget(list, area, &mut app.limit_state);
}

fn draw_status(f: &mut Frame<'_>, app: &mut App, area: Rect) {
    let theme = &app.theme;
    let msg = if let Some(at) = app.status_at {
        if at.elapsed().as_secs() < 6 {
            app.status.clone()
        } else {
            String::new()
        }
    } else {
        String::new()
    };
    f.render_widget(
        Paragraph::new(msg)
            .style(Style::default().fg(theme.warn))
            .block(Block::new().borders(Borders::TOP).border_style(Style::default().fg(theme.bg))),
        area,
    );
}

fn draw_footer(f: &mut Frame<'_>, app: &mut App, area: Rect) {
    let theme = &app.theme;
    let hints = match app.tab {
        Tab::Snapshots => "↑/↓ move · space select · d delete · c create · e edit · a select all · i init storage · esc clear",
        Tab::Configs => "↑/↓ move · enter open · i init storage",
        Tab::Limits => "↑/↓ move · enter edit · s save · x discard · r reload",
    };
    let line = Line::from(vec![
        Span::styled(format!("{hints}   "), Style::default().fg(theme.muted)),
        Span::styled("tab", Style::default().fg(theme.accent)),
        Span::styled(" next · ", Style::default().fg(theme.muted)),
        Span::styled("r", Style::default().fg(theme.accent)),
        Span::styled(" refresh · ", Style::default().fg(theme.muted)),
        Span::styled("q", Style::default().fg(theme.accent)),
        Span::styled(" quit", Style::default().fg(theme.muted)),
    ]);
    f.render_widget(
        Paragraph::new(line)
            .alignment(Alignment::Left)
            .style(Style::default().bg(theme.bg)),
        area,
    );
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}

fn draw_popup(f: &mut Frame<'_>, app: &mut App, popup: &Popup) {
    let theme = &app.theme;
    f.render_widget(Clear, f.area());

    let (title, content, width, height): (String, Vec<Line>, u16, u16) = match popup {
        Popup::ConfirmDelete { ids } => {
            let names: Vec<String> = ids.iter().map(|id| format!("#{id}")).collect();
            let list = names.join(", ");
            (
                "Confirm delete".into(),
                vec![
                    Line::from(format!("Deleting: {list}")),
                    Line::from("This cannot be undone."),
                    Line::from(""),
                    Line::from("y = delete, n/esc = cancel"),
                ],
                50,
                9,
            )
        }
        Popup::CreateSnapshot { description, cleanup, focus, error } => {
            let mut lines = vec![Line::from("Create single snapshot")];
            lines.push(Line::from(""));
            lines.push(input_line("description", description, *focus == 0, theme));
            lines.push(input_line("cleanup", cleanup, *focus == 1, theme));
            lines.push(Line::from(""));
            if let Some(err) = error {
                lines.push(Line::from(Span::styled(err, Style::default().fg(theme.error))));
            }
            lines.push(Line::from("enter = create, tab = next field, esc = cancel"));
            ("Create snapshot".into(), lines, 60, 11)
        }
        Popup::EditSnapshot { id, description, cleanup, focus } => {
            let mut lines = vec![Line::from(format!("Edit snapshot #{id}"))];
            lines.push(Line::from(""));
            lines.push(input_line("description", description, *focus == 0, theme));
            lines.push(input_line("cleanup", cleanup, *focus == 1, theme));
            lines.push(Line::from(""));
            lines.push(Line::from("enter = save, tab = next field, esc = cancel"));
            ("Edit snapshot".into(), lines, 60, 10)
        }
        Popup::EditLimit { key, value } => {
            let mut lines = vec![Line::from(format!("Edit {key}"))];
            lines.push(Line::from(""));
            lines.push(input_line("value", value, true, theme));
            lines.push(Line::from(""));
            lines.push(Line::from("enter = apply, esc = cancel"));
            ("Edit limit".into(), lines, 50, 8)
        }
        Popup::Message { text } => {
            let mut lines = vec![Line::from("snaptui")];
            lines.push(Line::from(""));
            lines.push(Line::from(text.clone()));
            lines.push(Line::from(""));
            lines.push(Line::from("enter/esc = ok"));
            ("Notice".into(), lines, 50, 8)
        }
    };

    let area = centered(f.area(), width, height);
    let block = Block::bordered()
        .border_style(Style::default().fg(theme.accent))
        .title(Line::from(Span::styled(
            title,
            Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
        )))
        .bg(theme.bg);
    f.render_widget(Clear, area);
    f.render_widget(Paragraph::new(Text::from(content)).block(block), area);
}

fn input_line<'a>(
    label: &'a str,
    value: &'a str,
    focused: bool,
    theme: &crate::theme::Theme,
) -> Line<'a> {
    let marker = if focused { "> " } else { "  " };
    Line::from(vec![
        Span::styled(marker, Style::default().fg(theme.accent)),
        Span::styled(format!("{label}: "), Style::default().fg(theme.muted)),
        Span::styled(value, Style::default().fg(if focused { theme.fg } else { theme.muted })),
    ])
}
