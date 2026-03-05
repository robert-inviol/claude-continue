use crate::app::{App, View, DETAIL_MENU_ITEMS};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};

// Catppuccin Mocha palette
const PINK: Color = Color::Rgb(245, 194, 231);
const BLUE: Color = Color::Rgb(137, 180, 250);
const GREEN: Color = Color::Rgb(166, 227, 161);
const YELLOW: Color = Color::Rgb(249, 226, 175);
const RED: Color = Color::Rgb(243, 139, 168);
const DIM: Color = Color::Rgb(108, 112, 134);
const SURFACE: Color = Color::Rgb(49, 50, 68);
const TEXT: Color = Color::Rgb(205, 214, 244);

pub fn draw(f: &mut Frame, app: &App) {
    let size = f.area();

    match &app.view {
        View::ProjectList => draw_project_list(f, app, size),
        View::RecentSessions => draw_recent_sessions(f, app, size),
        View::SessionList {
            project_label, ..
        } => draw_session_list(f, app, size, project_label),
        View::SessionDetail { session } => {
            draw_session_detail(f, app, size, session)
        }
        View::Conversation { messages, scroll } => {
            draw_conversation(f, size, messages, *scroll)
        }
        View::ConfirmDelete { session, .. } => {
            draw_confirm_delete(f, app, size, session)
        }
    }

    // Status message overlay
    if let Some(msg) = &app.status_msg {
        let area = centered_rect(50, 3, size);
        f.render_widget(Clear, area);
        let p = Paragraph::new(msg.as_str())
            .style(Style::default().fg(GREEN))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(GREEN)),
            );
        f.render_widget(p, area);
    }
}

fn draw_project_list(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header
            Constraint::Length(2), // tab bar + help
            Constraint::Min(1),   // list
            Constraint::Length(1), // filter
        ])
        .split(area);

    // Header
    let header = Paragraph::new("  Claude Sessions  ")
        .style(
            Style::default()
                .fg(PINK)
                .add_modifier(Modifier::BOLD),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(BLUE)),
        );
    f.render_widget(header, chunks[0]);

    // Tab bar
    let tabs = Line::from(vec![
        Span::styled(
            " Folders ",
            Style::default()
                .fg(PINK)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        ),
        Span::styled(" | ", Style::default().fg(DIM)),
        Span::styled(" Recent ", Style::default().fg(DIM)),
        Span::styled(
            "          tab: switch  enter: open  /: filter  esc: quit",
            Style::default().fg(DIM),
        ),
    ]);
    f.render_widget(Paragraph::new(tabs), chunks[1]);

    // Project list
    let items: Vec<ListItem> = app
        .filtered
        .iter()
        .enumerate()
        .map(|(i, &idx)| {
            let p = &app.projects[idx];
            let content = format!(
                "{:<45}  {:>3} sessions  {}",
                p.display_path, p.session_count, p.time_ago
            );
            let style = if i == app.selected {
                Style::default().fg(BLUE).bg(SURFACE)
            } else {
                Style::default().fg(TEXT)
            };
            ListItem::new(content).style(style)
        })
        .collect();

    let list = List::new(items);
    f.render_widget(list, chunks[2]);

    // Filter bar
    if app.filtering {
        let filter = Paragraph::new(format!("/{}", app.filter_text))
            .style(Style::default().fg(YELLOW));
        f.render_widget(filter, chunks[3]);
    }
}

fn draw_recent_sessions(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(2),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area);

    let header = Paragraph::new("  Claude Sessions  ")
        .style(
            Style::default()
                .fg(PINK)
                .add_modifier(Modifier::BOLD),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(BLUE)),
        );
    f.render_widget(header, chunks[0]);

    let tabs = Line::from(vec![
        Span::styled(" Folders ", Style::default().fg(DIM)),
        Span::styled(" | ", Style::default().fg(DIM)),
        Span::styled(
            " Recent ",
            Style::default()
                .fg(PINK)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        ),
        Span::styled(
            "      enter: resume  del: delete  right: details  /: filter  esc: quit",
            Style::default().fg(DIM),
        ),
    ]);
    f.render_widget(Paragraph::new(tabs), chunks[1]);

    draw_session_items(f, app, chunks[2], true);

    if app.filtering {
        let filter = Paragraph::new(format!("/{}", app.filter_text))
            .style(Style::default().fg(YELLOW));
        f.render_widget(filter, chunks[3]);
    }
}

fn draw_session_list(
    f: &mut Frame,
    app: &App,
    area: Rect,
    project_label: &str,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(2),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area);

    let header = Paragraph::new(format!("  {}  ", project_label))
        .style(
            Style::default()
                .fg(PINK)
                .add_modifier(Modifier::BOLD),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(BLUE)),
        );
    f.render_widget(header, chunks[0]);

    let help = Line::from(vec![
        Span::styled(
            format!("{} sessions  ", app.sessions.len()),
            Style::default().fg(DIM),
        ),
        Span::styled(
            "enter: resume  del: delete  right: details  /: filter  esc: back",
            Style::default().fg(DIM),
        ),
    ]);
    f.render_widget(Paragraph::new(help), chunks[1]);

    draw_session_items(f, app, chunks[2], false);

    if app.filtering {
        let filter = Paragraph::new(format!("/{}", app.filter_text))
            .style(Style::default().fg(YELLOW));
        f.render_widget(filter, chunks[3]);
    }
}

fn draw_session_items(
    f: &mut Frame,
    app: &App,
    area: Rect,
    show_project: bool,
) {
    let items: Vec<ListItem> = app
        .filtered
        .iter()
        .enumerate()
        .map(|(i, &idx)| {
            let s = &app.sessions[idx];
            let msg_display: String =
                s.first_msg.chars().take(55).collect();
            let project_col = if show_project {
                let label: String = if s.cwd.len() > 20 {
                    format!("..{}", &s.cwd[s.cwd.len().saturating_sub(18)..])
                } else {
                    s.cwd.clone()
                };
                format!("  {:<20}", label)
            } else {
                String::new()
            };
            let content = format!(
                "{:<10}  {:>3} msgs  {:>7}  {:<8}{}  {}",
                s.last_ts_ago,
                s.total_msgs,
                s.file_size,
                &s.session_id[..8.min(s.session_id.len())],
                project_col,
                msg_display
            );
            let style = if i == app.selected {
                Style::default().fg(BLUE).bg(SURFACE)
            } else {
                Style::default().fg(TEXT)
            };
            ListItem::new(content).style(style)
        })
        .collect();

    // Calculate offset to keep selection visible
    let visible_height = area.height as usize;
    let offset = if app.selected >= visible_height {
        app.selected - visible_height + 1
    } else {
        0
    };

    let items_to_show: Vec<ListItem> = items
        .into_iter()
        .skip(offset)
        .collect();

    let list = List::new(items_to_show);
    f.render_widget(list, area);
}

fn draw_session_detail(
    f: &mut Frame,
    app: &App,
    area: Rect,
    session: &crate::data::SessionInfo,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // header
            Constraint::Length(3),  // first message
            Constraint::Length(10), // details
            Constraint::Min(1),    // menu
        ])
        .split(area);

    // Header
    let header = Paragraph::new("  Session Detail  ")
        .style(
            Style::default()
                .fg(PINK)
                .add_modifier(Modifier::BOLD),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(BLUE)),
        );
    f.render_widget(header, chunks[0]);

    // First message
    let msg_lines = vec![
        Line::from(Span::styled(
            "First message:",
            Style::default()
                .fg(BLUE)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            format!("  \"{}\"", session.first_msg),
            Style::default()
                .fg(GREEN)
                .add_modifier(Modifier::ITALIC),
        )),
    ];
    f.render_widget(Paragraph::new(msg_lines), chunks[1]);

    // Details
    let started = if session.first_ts.is_empty() {
        "unknown".to_string()
    } else {
        format!("{} ({})", session.first_ts_ago, session.first_ts)
    };
    let last = if session.last_ts.is_empty() {
        "unknown".to_string()
    } else {
        format!("{} ({})", session.last_ts_ago, session.last_ts)
    };

    let detail_lines = vec![
        Line::from(Span::styled(
            format!("  {:<16} {}", "Session ID:", session.session_id),
            Style::default().fg(DIM),
        )),
        Line::from(Span::styled(
            format!("  {:<16} {}", "Working Dir:", session.cwd),
            Style::default().fg(DIM),
        )),
        Line::from(Span::styled(
            format!("  {:<16} {}", "Model:", session.model),
            Style::default().fg(DIM),
        )),
        Line::from(Span::styled(
            format!("  {:<16} {}", "Started:", started),
            Style::default().fg(DIM),
        )),
        Line::from(Span::styled(
            format!("  {:<16} {}", "Last Activity:", last),
            Style::default().fg(DIM),
        )),
        Line::from(Span::styled(
            format!(
                "  {:<16} {} user / {} assistant ({} total)",
                "Messages:",
                session.user_msgs,
                session.assistant_msgs,
                session.total_msgs
            ),
            Style::default().fg(DIM),
        )),
        Line::from(Span::styled(
            format!("  {:<16} {}", "Log Size:", session.file_size),
            Style::default().fg(DIM),
        )),
    ];
    f.render_widget(Paragraph::new(detail_lines), chunks[2]);

    // Menu
    let menu_items: Vec<ListItem> = DETAIL_MENU_ITEMS
        .iter()
        .enumerate()
        .map(|(i, &item)| {
            let style = if i == app.detail_menu_idx {
                Style::default().fg(BLUE).bg(SURFACE)
            } else {
                Style::default().fg(TEXT)
            };
            ListItem::new(format!("  {}", item)).style(style)
        })
        .collect();

    let menu = List::new(menu_items);
    f.render_widget(menu, chunks[3]);
}

fn draw_conversation(
    f: &mut Frame,
    area: Rect,
    messages: &[crate::data::ConversationMessage],
    scroll: usize,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area);

    let header = Paragraph::new("  Conversation  ")
        .style(
            Style::default()
                .fg(PINK)
                .add_modifier(Modifier::BOLD),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(BLUE)),
        );
    f.render_widget(header, chunks[0]);

    let mut lines: Vec<Line> = Vec::new();
    for msg in messages {
        let ts_short = if msg.timestamp.len() > 16 {
            &msg.timestamp[11..16]
        } else {
            ""
        };

        let (prefix_style, prefix_text) = if msg.role == "user" {
            (
                Style::default()
                    .fg(BLUE)
                    .add_modifier(Modifier::BOLD),
                format!("[{}] You:", ts_short),
            )
        } else {
            (
                Style::default()
                    .fg(GREEN)
                    .add_modifier(Modifier::BOLD),
                format!("[{}] Claude:", ts_short),
            )
        };

        lines.push(Line::from(Span::styled(prefix_text, prefix_style)));

        let text = if msg.role == "assistant" && msg.text.len() > 800 {
            format!("{}...", &msg.text[..797])
        } else {
            msg.text.clone()
        };

        for line in text.lines() {
            lines.push(Line::from(Span::styled(
                format!("  {}", line),
                Style::default().fg(TEXT),
            )));
        }
        lines.push(Line::from(""));
    }

    let para = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((scroll as u16, 0));
    f.render_widget(para, chunks[1]);

    let help = Paragraph::new("  Up/Down: scroll  Esc/q: back")
        .style(Style::default().fg(DIM));
    f.render_widget(help, chunks[2]);
}

fn draw_confirm_delete(
    f: &mut Frame,
    app: &App,
    area: Rect,
    session: &crate::data::SessionInfo,
) {
    let popup = centered_rect(50, 8, area);
    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Confirm Delete ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(RED));

    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(1),
        ])
        .split(inner);

    let msg = Paragraph::new(format!(
        "Delete session {}...?",
        &session.session_id[..8.min(session.session_id.len())]
    ))
    .style(Style::default().fg(YELLOW));
    f.render_widget(msg, chunks[0]);

    let choices = vec!["Yes", "No"];
    let items: Vec<ListItem> = choices
        .iter()
        .enumerate()
        .map(|(i, &c)| {
            let style = if i == app.selected {
                Style::default().fg(RED).bg(SURFACE)
            } else {
                Style::default().fg(TEXT)
            };
            ListItem::new(format!("  {}", c)).style(style)
        })
        .collect();
    f.render_widget(List::new(items), chunks[1]);
}

fn centered_rect(percent_x: u16, height: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(r.height.saturating_sub(height) / 2),
            Constraint::Length(height),
            Constraint::Min(0),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
