use crate::data::{self, ConversationMessage, Project, SessionInfo};

#[derive(Debug, Clone, PartialEq)]
pub enum View {
    ProjectList,
    RecentSessions,
    SessionList {
        project_dir: String,
        project_label: String,
    },
    SessionDetail {
        session: SessionInfo,
    },
    Conversation {
        messages: Vec<ConversationMessage>,
        scroll: usize,
    },
    ConfirmDelete {
        session: SessionInfo,
        return_view: Box<View>,
    },
}

pub struct App {
    pub view: View,
    pub view_stack: Vec<View>,
    pub projects: Vec<Project>,
    pub sessions: Vec<SessionInfo>,
    pub filtered: Vec<usize>,
    pub selected: usize,
    pub filter_text: String,
    pub filtering: bool,
    pub quit: bool,
    pub launch: Option<LaunchAction>,
    pub status_msg: Option<String>,
    pub detail_menu_idx: usize,
}

#[derive(Debug, Clone)]
pub struct LaunchAction {
    pub cwd: String,
    pub resume_id: Option<String>,
}

pub const DETAIL_MENU_ITEMS: &[&str] = &[
    "Resume session",
    "New session in this directory",
    "View conversation",
    "Copy session ID",
    "Delete session",
    "Back",
];

impl App {
    pub fn new() -> Self {
        App {
            view: View::ProjectList,
            view_stack: Vec::new(),
            projects: Vec::new(),
            sessions: Vec::new(),
            filtered: Vec::new(),
            selected: 0,
            filter_text: String::new(),
            filtering: false,
            quit: false,
            launch: None,
            status_msg: None,
            detail_menu_idx: 0,
        }
    }

    pub fn load_projects(&mut self) {
        self.projects = data::list_projects();
        self.rebuild_filtered();
    }

    pub fn load_all_sessions(&mut self) {
        self.sessions = data::list_all_sessions();
        self.rebuild_filtered();
    }

    pub fn load_sessions_for_project(&mut self, dir_name: &str) {
        let proj_path = data::projects_dir().join(dir_name);
        self.sessions = data::list_sessions(&proj_path);
        self.rebuild_filtered();
    }

    pub fn rebuild_filtered(&mut self) {
        let query = self.filter_text.to_lowercase();
        match &self.view {
            View::ProjectList => {
                self.filtered = self
                    .projects
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| {
                        query.is_empty()
                            || p.display_path.to_lowercase().contains(&query)
                    })
                    .map(|(i, _)| i)
                    .collect();
            }
            View::RecentSessions
            | View::SessionList { .. } => {
                self.filtered = self
                    .sessions
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| {
                        query.is_empty()
                            || s.first_msg.to_lowercase().contains(&query)
                            || s.session_id.to_lowercase().contains(&query)
                            || s.cwd.to_lowercase().contains(&query)
                    })
                    .map(|(i, _)| i)
                    .collect();
            }
            _ => {}
        }
        if self.selected >= self.filtered.len() {
            self.selected = self.filtered.len().saturating_sub(1);
        }
    }

    pub fn move_up(&mut self) {
        match &self.view {
            View::SessionDetail { .. } => {
                if self.detail_menu_idx > 0 {
                    self.detail_menu_idx -= 1;
                }
            }
            View::Conversation { scroll, messages, .. } => {
                if *scroll > 0 {
                    let msgs = messages.clone();
                    self.view = View::Conversation {
                        messages: msgs,
                        scroll: scroll.saturating_sub(3),
                    };
                }
            }
            View::ConfirmDelete { .. } => {
                self.selected = if self.selected > 0 { 0 } else { 0 };
            }
            _ => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
            }
        }
    }

    pub fn move_down(&mut self) {
        match &self.view {
            View::SessionDetail { .. } => {
                if self.detail_menu_idx < DETAIL_MENU_ITEMS.len() - 1 {
                    self.detail_menu_idx += 1;
                }
            }
            View::Conversation { scroll, messages, .. } => {
                self.view = View::Conversation {
                    messages: messages.clone(),
                    scroll: scroll + 3,
                };
            }
            View::ConfirmDelete { .. } => {
                self.selected = if self.selected < 1 { 1 } else { 1 };
            }
            _ => {
                let max = self.filtered.len().saturating_sub(1);
                if self.selected < max {
                    self.selected += 1;
                }
            }
        }
    }

    pub fn push_view(&mut self, new_view: View) {
        let old = self.view.clone();
        self.view_stack.push(old);
        self.view = new_view;
        self.selected = 0;
        self.filter_text.clear();
        self.filtering = false;
        self.detail_menu_idx = 0;
    }

    pub fn pop_view(&mut self) {
        if let Some(prev) = self.view_stack.pop() {
            self.view = prev;
            self.selected = 0;
            self.filter_text.clear();
            self.filtering = false;
            // Reload data for the view we're returning to
            match &self.view {
                View::ProjectList => self.load_projects(),
                View::RecentSessions => self.load_all_sessions(),
                View::SessionList { project_dir, .. } => {
                    let dir = project_dir.clone();
                    self.load_sessions_for_project(&dir);
                }
                _ => {}
            }
        } else {
            self.quit = true;
        }
    }

    pub fn enter_selection(&mut self) {
        match self.view.clone() {
            View::ProjectList => {
                if let Some(&idx) = self.filtered.get(self.selected) {
                    let proj = &self.projects[idx];
                    let dir_name = proj.dir_name.clone();
                    let label = proj.display_path.clone();
                    self.push_view(View::SessionList {
                        project_dir: dir_name.clone(),
                        project_label: label,
                    });
                    self.load_sessions_for_project(&dir_name);
                }
            }
            View::RecentSessions | View::SessionList { .. } => {
                if let Some(&idx) = self.filtered.get(self.selected) {
                    let session = self.sessions[idx].clone();
                    let cwd = session.cwd.replace("~", &dirs::home_dir().unwrap_or_default().to_string_lossy());
                    self.launch = Some(LaunchAction {
                        cwd,
                        resume_id: Some(session.session_id.clone()),
                    });
                }
            }
            View::SessionDetail { ref session } => {
                let session = session.clone();
                match DETAIL_MENU_ITEMS[self.detail_menu_idx] {
                    "Resume session" => {
                        let cwd = session.cwd.replace("~", &dirs::home_dir().unwrap_or_default().to_string_lossy());
                        self.launch = Some(LaunchAction {
                            cwd,
                            resume_id: Some(session.session_id.clone()),
                        });
                    }
                    "New session in this directory" => {
                        let cwd = session.cwd.replace("~", &dirs::home_dir().unwrap_or_default().to_string_lossy());
                        self.launch = Some(LaunchAction {
                            cwd,
                            resume_id: None,
                        });
                    }
                    "View conversation" => {
                        let messages = data::read_conversation(&session.jsonl_path);
                        self.push_view(View::Conversation {
                            messages,
                            scroll: 0,
                        });
                    }
                    "Copy session ID" => {
                        copy_to_clipboard(&session.session_id);
                        self.status_msg =
                            Some(format!("Copied: {}", session.session_id));
                    }
                    "Delete session" => {
                        let return_view = Box::new(self.view.clone());
                        self.push_view(View::ConfirmDelete {
                            session,
                            return_view,
                        });
                        self.selected = 1; // default to No
                    }
                    "Back" => {
                        self.pop_view();
                    }
                    _ => {}
                }
            }
            View::ConfirmDelete {
                ref session,
                ..
            } => {
                if self.selected == 0 {
                    // Yes - delete
                    let _ = std::fs::remove_file(&session.jsonl_path);
                    let dir = session.jsonl_path.with_extension("");
                    let _ = std::fs::remove_dir_all(&dir);
                    self.status_msg = Some("Session deleted.".to_string());
                    // Pop the confirm view
                    self.view_stack.pop();
                    // Pop back past the detail view too
                    self.pop_view();
                } else {
                    // No - go back
                    self.view_stack.pop(); // remove confirm from stack
                    self.pop_view();
                    // Re-enter detail
                }
            }
            View::Conversation { .. } => {}
        }
    }

    pub fn show_detail_for_selected(&mut self) {
        match &self.view {
            View::RecentSessions | View::SessionList { .. } => {
                if let Some(&idx) = self.filtered.get(self.selected) {
                    let session = self.sessions[idx].clone();
                    self.push_view(View::SessionDetail { session });
                }
            }
            _ => {}
        }
    }

    pub fn delete_selected(&mut self) {
        match &self.view {
            View::RecentSessions | View::SessionList { .. } => {
                if let Some(&idx) = self.filtered.get(self.selected) {
                    let session = self.sessions[idx].clone();
                    let return_view = Box::new(self.view.clone());
                    self.push_view(View::ConfirmDelete {
                        session,
                        return_view,
                    });
                    self.selected = 1; // default to No
                }
            }
            _ => {}
        }
    }

    pub fn toggle_view_mode(&mut self) {
        match &self.view {
            View::ProjectList => {
                self.view = View::RecentSessions;
                self.selected = 0;
                self.filter_text.clear();
                self.load_all_sessions();
            }
            View::RecentSessions => {
                self.view = View::ProjectList;
                self.selected = 0;
                self.filter_text.clear();
                self.load_projects();
            }
            _ => {}
        }
    }
}

fn copy_to_clipboard(text: &str) {
    use std::io::Write;
    use std::process::{Command, Stdio};

    // Try wl-copy, xclip, xsel in order
    for (cmd, args) in [
        ("wl-copy", vec![]),
        ("xclip", vec!["-selection", "clipboard"]),
        ("xsel", vec!["--clipboard", "--input"]),
    ] {
        if let Ok(mut child) = Command::new(cmd)
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            let _ = child.wait();
            return;
        }
    }
}
