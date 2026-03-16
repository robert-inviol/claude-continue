use crate::app::{App, View};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
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

enum ActiveTab {
    Folders,
    Recent,
    Project(String),
}

fn draw_chrome(f: &mut Frame, area: Rect, app: &App, active: &ActiveTab) -> Rect {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header
            Constraint::Length(2), // tab bar
            Constraint::Min(1),   // content
            Constraint::Length(1), // filter
        ])
        .split(area);

    // Header
    let title = match active {
        ActiveTab::Project(label) => format!("  {}  ", label),
        _ => "  Claude Sessions  ".to_string(),
    };
    let header = Paragraph::new(title)
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
    let (folders_style, recent_style) = match active {
        ActiveTab::Folders => (
            Style::default().fg(PINK).add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            Style::default().fg(DIM),
        ),
        ActiveTab::Recent => (
            Style::default().fg(DIM),
            Style::default().fg(PINK).add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        ),
        ActiveTab::Project(_) => (
            Style::default().fg(DIM),
            Style::default().fg(DIM),
        ),
    };

    let help = match active {
        ActiveTab::Folders | ActiveTab::Project(_) => {
            "      tab: switch  enter: resume/new  d/D: delete  /: filter  esc: back"
        }
        ActiveTab::Recent => {
            "      enter: resume/new  d/D: delete  /: filter  esc: quit"
        }
    };

    let tabs = Line::from(vec![
        Span::styled(" Folders ", folders_style),
        Span::styled(" | ", Style::default().fg(DIM)),
        Span::styled(" Recent ", recent_style),
        Span::styled(help, Style::default().fg(DIM)),
    ]);
    f.render_widget(Paragraph::new(tabs), chunks[1]);

    // Filter bar
    if app.filtering {
        let filter = Paragraph::new(format!("/{}", app.filter_text))
            .style(Style::default().fg(YELLOW));
        f.render_widget(filter, chunks[3]);
    }

    chunks[2] // return content area
}

fn draw_project_list(f: &mut Frame, app: &App, area: Rect) {
    let content = draw_chrome(f, area, app, &ActiveTab::Folders);

    let list_width = content.width as usize;
    let path_width = list_width.saturating_sub(28);

    let mut items: Vec<ListItem> = Vec::new();

    {
        let style = if app.selected == 0 {
            Style::default().fg(GREEN).bg(SURFACE).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(GREEN)
        };
        items.push(ListItem::new("+ New session").style(style));
    }

    let project_items: Vec<ListItem> = app
        .filtered
        .iter()
        .enumerate()
        .map(|(i, &idx)| {
            let p = &app.projects[idx];
            let content = format!(
                "{:<pw$}  {:>3} sessions  {}",
                p.display_path, p.session_count, p.time_ago,
                pw = path_width
            );
            let style = if i + 1 == app.selected {
                Style::default().fg(BLUE).bg(SURFACE)
            } else {
                Style::default().fg(TEXT)
            };
            ListItem::new(content).style(style)
        })
        .collect();
    items.extend(project_items);

    f.render_widget(List::new(items), content);
}

fn draw_recent_sessions(f: &mut Frame, app: &App, area: Rect) {
    let content = draw_chrome(f, area, app, &ActiveTab::Recent);

    if app.loading {
        let loading = Paragraph::new("  Loading sessions...")
            .style(Style::default().fg(YELLOW));
        f.render_widget(loading, content);
    } else {
        draw_session_items(f, app, content, true);
    }
}

fn draw_session_list(
    f: &mut Frame,
    app: &App,
    area: Rect,
    project_label: &str,
) {
    let content = draw_chrome(f, area, app, &ActiveTab::Project(project_label.to_string()));
    draw_session_items(f, app, content, false);
}

fn draw_session_items(
    f: &mut Frame,
    app: &App,
    area: Rect,
    show_project: bool,
) {
    let has_dotdot = !show_project;
    let header_rows = app.header_rows();

    let mut items: Vec<ListItem> = Vec::new();

    // "+ New session" always first
    {
        let style = if app.selected == 0 {
            Style::default().fg(GREEN).bg(SURFACE).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(GREEN)
        };
        items.push(ListItem::new("+ New session").style(style));
    }

    // ".." for per-project view
    if has_dotdot {
        let style = if app.selected == 1 {
            Style::default().fg(BLUE).bg(SURFACE)
        } else {
            Style::default().fg(DIM)
        };
        items.push(ListItem::new("..").style(style));
    }

    let width = area.width as usize;
    let fixed_cols = 42;
    let remaining = width.saturating_sub(fixed_cols);

    let (proj_width, msg_width) = if show_project {
        let pw = (remaining * 30 / 100).max(15);
        let mw = remaining.saturating_sub(pw + 2);
        (pw, mw)
    } else {
        (0, remaining)
    };

    let session_items: Vec<ListItem> = app
        .filtered
        .iter()
        .enumerate()
        .map(|(i, &idx)| {
            let s = &app.sessions[idx];
            let msg_display: String =
                s.first_msg.chars().take(msg_width).collect();
            let project_col = if show_project {
                let label: String = if s.cwd.chars().count() > proj_width {
                    let skip = s.cwd.chars().count().saturating_sub(proj_width - 2);
                    format!("..{}", s.cwd.chars().skip(skip).collect::<String>())
                } else {
                    s.cwd.clone()
                };
                format!("  {:<pw$}", label, pw = proj_width)
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
            let style = if i + header_rows == app.selected {
                Style::default().fg(BLUE).bg(SURFACE)
            } else {
                Style::default().fg(TEXT)
            };
            ListItem::new(content).style(style)
        })
        .collect();

    items.extend(session_items);

    // Scroll to keep selection visible
    let visible_height = area.height as usize;
    let offset = if app.selected >= visible_height {
        app.selected - visible_height + 1
    } else {
        0
    };

    let items_to_show: Vec<ListItem> = items.into_iter().skip(offset).collect();

    let list = List::new(items_to_show);
    f.render_widget(list, area);
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
