mod app;
mod data;
mod ui;

use app::{App, View};
use clap::Parser;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
};
use ratatui::prelude::*;
use std::io;
use std::os::unix::process::CommandExt;
use std::process::Command;

#[derive(Parser)]
#[command(name = "claude-continue", about = "Interactive explorer for Claude Code sessions")]
struct Cli {
    /// Search all sessions
    #[arg(short, long, value_name = "QUERY", num_args = 0..=1, default_missing_value = "")]
    search: Option<String>,

    /// Look up a session by full or partial ID
    #[arg(short, long, value_name = "ID")]
    id: Option<String>,

    /// Bare argument: treated as session ID fragment or search query
    #[arg(value_name = "QUERY")]
    query: Option<String>,
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();

    // Verify projects dir exists
    let projects_dir = data::projects_dir();
    if !projects_dir.is_dir() {
        eprintln!(
            "No Claude projects directory found at {}",
            projects_dir.display()
        );
        std::process::exit(1);
    }

    let mut app = App::new();

    // Handle CLI arguments
    if let Some(query) = &cli.id {
        let matches = data::lookup_session(query);
        if matches.is_empty() {
            eprintln!("No session found matching: {}", query);
            std::process::exit(1);
        }
        if matches.len() == 1 {
            let (sid, proj_path) = &matches[0];
            let jsonl = proj_path.join(format!("{}.jsonl", sid));
            let session = data::scan_session(&jsonl);
            app.push_view(View::SessionDetail {
                session,
            });
        } else {
            // Multiple matches - show search
            app.view = View::RecentSessions;
            app.load_all_sessions();
            app.filter_text = query.clone();
            app.filtering = true;
            app.rebuild_filtered();
        }
    } else if let Some(query) = &cli.search {
        app.view = View::RecentSessions;
        app.load_all_sessions();
        if !query.is_empty() {
            app.filter_text = query.clone();
            app.filtering = true;
            app.rebuild_filtered();
        }
    } else if let Some(query) = &cli.query {
        // Check if it looks like a UUID fragment
        if query.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
            let matches = data::lookup_session(query);
            if matches.len() == 1 {
                let (sid, proj_path) = &matches[0];
                let jsonl = proj_path.join(format!("{}.jsonl", sid));
                let session = data::scan_session(&jsonl);
                app.push_view(View::SessionDetail {
                    session,
                });
            } else {
                app.view = View::RecentSessions;
                app.load_all_sessions();
                app.filter_text = query.clone();
                app.filtering = true;
                app.rebuild_filtered();
            }
        } else {
            app.view = View::RecentSessions;
            app.load_all_sessions();
            app.filter_text = query.clone();
            app.filtering = true;
            app.rebuild_filtered();
        }
    } else {
        // Default: check if cwd has a project
        let cwd = std::env::current_dir()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        if let Some(proj_dir) = data::cwd_to_project_dir(&cwd) {
            let label = cwd
                .replace(
                    &dirs::home_dir()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string(),
                    "~",
                );
            app.view = View::SessionList {
                project_dir: proj_dir.clone(),
                project_label: label,
            };
            app.load_sessions_for_project(&proj_dir);
        } else {
            app.load_projects();
        }
    }

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Main loop
    let result = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result?;

    // If there's a launch action, exec claude
    if let Some(launch) = app.launch {
        let target_cwd = launch.cwd;
        std::env::set_current_dir(&target_cwd).unwrap_or_else(|e| {
            eprintln!("Failed to cd to {}: {}", target_cwd, e);
            std::process::exit(1);
        });

        let mut cmd = Command::new("claude");
        if let Some(id) = launch.resume_id {
            cmd.arg("--resume").arg(id);
        }
        let err = cmd.exec();
        eprintln!("Failed to exec claude: {}", err);
        std::process::exit(1);
    }

    Ok(())
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        // Clear status message after one frame
        if app.status_msg.is_some() {
            // Wait for a key, then clear
            if let Event::Key(_) = event::read()? {
                app.status_msg = None;
            }
            continue;
        }

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            // Global: Ctrl+C always quits
            if key.modifiers.contains(KeyModifiers::CONTROL)
                && key.code == KeyCode::Char('c')
            {
                return Ok(());
            }

            // Filter mode input
            if app.filtering {
                match key.code {
                    KeyCode::Esc => {
                        app.filter_text.clear();
                        app.filtering = false;
                        app.rebuild_filtered();
                    }
                    KeyCode::Enter => {
                        app.filtering = false;
                    }
                    KeyCode::Backspace => {
                        app.filter_text.pop();
                        app.rebuild_filtered();
                    }
                    KeyCode::Char(c) => {
                        app.filter_text.push(c);
                        app.rebuild_filtered();
                    }
                    KeyCode::Up => app.move_up(),
                    KeyCode::Down => app.move_down(),
                    _ => {}
                }
                continue;
            }

            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => {
                    match &app.view {
                        View::ProjectList | View::RecentSessions => {
                            if app.view_stack.is_empty() {
                                return Ok(());
                            }
                            app.pop_view();
                        }
                        _ => app.pop_view(),
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => app.move_up(),
                KeyCode::Down | KeyCode::Char('j') => app.move_down(),
                KeyCode::Enter => app.enter_selection(),
                KeyCode::Tab => app.toggle_view_mode(),
                KeyCode::Right | KeyCode::Char('l') => {
                    app.show_detail_for_selected()
                }
                KeyCode::Delete => app.delete_selected(),
                KeyCode::Char('d') => {
                    if key.modifiers.contains(KeyModifiers::CONTROL) {
                        // Page down in conversation
                        if let View::Conversation { messages, scroll } = &app.view {
                            app.view = View::Conversation {
                                messages: messages.clone(),
                                scroll: scroll + 20,
                            };
                        }
                    } else {
                        app.delete_selected();
                    }
                }
                KeyCode::Char('u') => {
                    if key.modifiers.contains(KeyModifiers::CONTROL) {
                        // Page up in conversation
                        if let View::Conversation { messages, scroll } = &app.view {
                            app.view = View::Conversation {
                                messages: messages.clone(),
                                scroll: scroll.saturating_sub(20),
                            };
                        }
                    }
                }
                KeyCode::Char('/') => {
                    match &app.view {
                        View::ProjectList
                        | View::RecentSessions
                        | View::SessionList { .. } => {
                            app.filtering = true;
                        }
                        _ => {}
                    }
                }
                KeyCode::PageDown => {
                    if let View::Conversation { messages, scroll } = &app.view {
                        app.view = View::Conversation {
                            messages: messages.clone(),
                            scroll: scroll + 20,
                        };
                    }
                }
                KeyCode::PageUp => {
                    if let View::Conversation { messages, scroll } = &app.view {
                        app.view = View::Conversation {
                            messages: messages.clone(),
                            scroll: scroll.saturating_sub(20),
                        };
                    }
                }
                _ => {}
            }
        }

        if app.quit || app.launch.is_some() {
            return Ok(());
        }
    }
}
