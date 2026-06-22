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

    let projects_dir = data::projects_dir();
    if !projects_dir.is_dir() {
        eprintln!(
            "No Claude projects directory found at {}",
            projects_dir.display()
        );
        std::process::exit(1);
    }

    let mut app = App::new();

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
            // Show recent view filtered to this session
            app.view = View::RecentSessions;
            app.load_all_sessions();
            app.filter_text = session.session_id[..8].to_string();
            app.filtering = true;
            app.rebuild_filtered();
        } else {
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
        if query.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
            let matches = data::lookup_session(query);
            if matches.len() == 1 {
                let (sid, proj_path) = &matches[0];
                let jsonl = proj_path.join(format!("{}.jsonl", sid));
                let session = data::scan_session(&jsonl);
                app.view = View::RecentSessions;
                app.load_all_sessions();
                app.filter_text = session.session_id[..8].to_string();
                app.filtering = true;
                app.rebuild_filtered();
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
        let cwd = std::env::current_dir()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        if let Some(proj_dir) = data::cwd_to_project_dir(&cwd) {
            let label = cwd.replace(
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

    let result = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result?;

    if let Some(launch) = app.launch {
        let target_cwd = launch.cwd;

        // Container-aware resume: sessions created inside a dev container record
        // their cwd as an in-container path (/workspaces/...). That path doesn't
        // exist on the host, so instead of cd-ing locally we locate the running
        // dev container whose workspace mount covers it and exec Claude *inside*
        // it via `devcontainer exec`.
        if let Some(host_folder) = resolve_container_workspace(&target_cwd) {
            let inner = match &launch.resume_id {
                Some(id) => format!(
                    "cd {} && exec claude --resume {}",
                    sh_quote(&target_cwd),
                    sh_quote(id)
                ),
                None => format!("cd {} && exec claude", sh_quote(&target_cwd)),
            };
            eprintln!("→ resuming inside dev container ({})", host_folder);
            let mut cmd = Command::new("devcontainer");
            cmd.arg("exec").arg("--workspace-folder").arg(&host_folder);
            // Containers inherit neither COLORTERM nor TERM from the host, so
            // Claude would render with degraded colours. Forward the host's
            // values so truecolour carries through.
            for var in ["TERM", "COLORTERM"] {
                if let Ok(val) = std::env::var(var) {
                    cmd.arg("--remote-env").arg(format!("{}={}", var, val));
                }
            }
            let err = cmd.arg("bash").arg("-lc").arg(&inner).exec();
            eprintln!("Failed to exec devcontainer: {}", err);
            std::process::exit(1);
        }

        std::env::set_current_dir(&target_cwd).unwrap_or_else(|e| {
            eprintln!("Failed to cd to {}: {}", target_cwd, e);
            if target_cwd.starts_with("/workspaces/") {
                eprintln!(
                    "(this looks like a dev-container path, but no running container \
                     mounts it — start it first, e.g. `cwt new <repo> <task>`)"
                );
            }
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

/// Single-quote a string for safe use inside a `bash -lc` command.
fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// If `cwd` is an in-container path served by a running dev container, return the
/// host workspace folder (the container's `devcontainer.local_folder`) so we can
/// `devcontainer exec` into it. Returns None for ordinary host paths.
///
/// Matches by finding the running dev container with the longest bind-mount
/// Destination that is a prefix of `cwd`, so sessions in subdirectories of the
/// workspace still resolve.
fn resolve_container_workspace(cwd: &str) -> Option<String> {
    if !cwd.starts_with("/workspaces/") {
        return None;
    }
    let out = Command::new("docker")
        .args(["ps", "-q", "--filter", "label=devcontainer.local_folder"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let ids = String::from_utf8_lossy(&out.stdout);
    let mut best: Option<(usize, String)> = None;
    for id in ids.split_whitespace() {
        let insp = Command::new("docker")
            .args([
                "inspect",
                "-f",
                "{{range .Mounts}}{{.Source}}\t{{.Destination}}\n{{end}}",
                id,
            ])
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&insp.stdout);
        for line in text.lines() {
            if let Some((src, dest)) = line.split_once('\t') {
                let covers = !dest.is_empty()
                    && (cwd == dest || cwd.starts_with(&format!("{}/", dest)));
                if covers
                    && best.as_ref().map_or(true, |(len, _)| dest.len() > *len)
                {
                    best = Some((dest.len(), src.to_string()));
                }
            }
        }
    }
    best.map(|(_, src)| src)
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        if app.loading {
            app.finish_loading();
            continue;
        }

        // Status message: wait for any key then clear
        if app.status_msg.is_some() {
            if let Event::Key(_) = event::read()? {
                app.status_msg = None;
            }
            continue;
        }

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            if key.modifiers.contains(KeyModifiers::CONTROL)
                && key.code == KeyCode::Char('c')
            {
                return Ok(());
            }

            // Filter mode
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
                KeyCode::Char('n') => app.new_session(),
                KeyCode::Delete => app.delete_selected(),
                KeyCode::Char('d') => app.delete_selected(),
                KeyCode::Char('D') => app.force_delete_selected(),
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
                _ => {}
            }
        }

        if app.quit || app.launch.is_some() {
            return Ok(());
        }
    }
}
