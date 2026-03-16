use crate::data::{self, Project, SessionInfo};

#[derive(Debug, Clone, PartialEq)]
pub enum View {
    ProjectList,
    RecentSessions,
    SessionList {
        project_dir: String,
        project_label: String,
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
    pub loading: bool,
    pub launch_cwd: String,
}

#[derive(Debug, Clone)]
pub struct LaunchAction {
    pub cwd: String,
    pub resume_id: Option<String>,
}

impl App {
    pub fn new() -> Self {
        let launch_cwd = std::env::current_dir()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
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
            loading: false,
            launch_cwd,
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
            View::RecentSessions | View::SessionList { .. } => {
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
        // +1 for the "New session" row in session views
        let total = self.total_rows();
        if self.selected >= total && total > 0 {
            self.selected = total - 1;
        }
    }

    /// Number of special rows before data items.
    /// ProjectList: "New session" = 1
    /// SessionList: "New session" + ".." = 2
    /// RecentSessions: "New session" = 1
    pub fn header_rows(&self) -> usize {
        match &self.view {
            View::SessionList { .. } => 2,
            View::ProjectList | View::RecentSessions => 1,
            _ => 0,
        }
    }

    fn total_rows(&self) -> usize {
        self.header_rows() + self.filtered.len()
    }

    /// Get the session index for the currently selected item,
    /// returning None if on a special row (.., New session).
    fn selected_session_idx(&self) -> Option<usize> {
        let hr = self.header_rows();
        if self.selected < hr {
            return None;
        }
        self.filtered.get(self.selected - hr).copied()
    }

    /// Check if "New session" row is currently selected (always row 0).
    fn is_new_session_selected(&self) -> bool {
        match &self.view {
            View::ProjectList | View::SessionList { .. } | View::RecentSessions => {
                self.selected == 0
            }
            _ => false,
        }
    }

    pub fn move_up(&mut self) {
        match &self.view {
            View::ConfirmDelete { .. } => {
                self.selected = 0;
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
            View::ConfirmDelete { .. } => {
                self.selected = 1;
            }
            _ => {
                let max = self.total_rows().saturating_sub(1);
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
    }

    pub fn pop_view(&mut self) {
        if let Some(prev) = self.view_stack.pop() {
            self.view = prev;
            self.selected = 0;
            self.filter_text.clear();
            self.filtering = false;
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
                if self.is_new_session_selected() {
                    self.launch_new_session();
                    return;
                }
                if let Some(&idx) = self.filtered.get(self.selected - 1) {
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
            View::SessionList { .. } => {
                if self.is_new_session_selected() {
                    self.launch_new_session();
                } else if self.selected == 1 {
                    // ".." — go back to project list
                    self.view_stack.clear();
                    self.view = View::ProjectList;
                    self.selected = 0;
                    self.filter_text.clear();
                    self.filtering = false;
                    self.load_projects();
                } else {
                    self.resume_selected();
                }
            }
            View::RecentSessions => {
                if self.is_new_session_selected() {
                    self.launch_new_session();
                } else {
                    self.resume_selected();
                }
            }
            View::ConfirmDelete { ref session, .. } => {
                if self.selected == 0 {
                    let _ = std::fs::remove_file(&session.jsonl_path);
                    let dir = session.jsonl_path.with_extension("");
                    let _ = std::fs::remove_dir_all(&dir);
                    self.status_msg = Some("Session deleted.".to_string());
                    self.pop_view();
                } else {
                    self.pop_view();
                }
            }
        }
    }

    fn resume_selected(&mut self) {
        if let Some(idx) = self.selected_session_idx() {
            let session = &self.sessions[idx];
            let cwd = session.cwd.replace(
                "~",
                &dirs::home_dir()
                    .unwrap_or_default()
                    .to_string_lossy(),
            );
            self.launch = Some(LaunchAction {
                cwd,
                resume_id: Some(session.session_id.clone()),
            });
        }
    }

    fn launch_new_session(&mut self) {
        // Use the project cwd if in a project session list, otherwise launch cwd
        let cwd = match &self.view {
            View::SessionList { .. } => {
                if let Some(first) = self.sessions.first() {
                    first.cwd.replace(
                        "~",
                        &dirs::home_dir()
                            .unwrap_or_default()
                            .to_string_lossy(),
                    )
                } else {
                    self.launch_cwd.clone()
                }
            }
            _ => self.launch_cwd.clone(),
        };
        self.launch = Some(LaunchAction {
            cwd,
            resume_id: None,
        });
    }

    pub fn new_session(&mut self) {
        if let Some(idx) = self.selected_session_idx() {
            let session = &self.sessions[idx];
            let cwd = session.cwd.replace(
                "~",
                &dirs::home_dir()
                    .unwrap_or_default()
                    .to_string_lossy(),
            );
            self.launch = Some(LaunchAction {
                cwd,
                resume_id: None,
            });
        } else {
            self.launch_new_session();
        }
    }

    pub fn delete_selected(&mut self) {
        if let Some(idx) = self.selected_session_idx() {
            let session = self.sessions[idx].clone();
            let return_view = Box::new(self.view.clone());
            self.push_view(View::ConfirmDelete {
                session,
                return_view,
            });
            self.selected = 1; // default to No
        }
    }

    pub fn force_delete_selected(&mut self) {
        if let Some(idx) = self.selected_session_idx() {
            let session = &self.sessions[idx];
            let _ = std::fs::remove_file(&session.jsonl_path);
            let dir = session.jsonl_path.with_extension("");
            let _ = std::fs::remove_dir_all(&dir);
            self.status_msg = Some("Session deleted.".to_string());
            // Reload current view
            match &self.view {
                View::RecentSessions => self.load_all_sessions(),
                View::SessionList { project_dir, .. } => {
                    let dir = project_dir.clone();
                    self.load_sessions_for_project(&dir);
                }
                _ => {}
            }
        }
    }

    pub fn toggle_view_mode(&mut self) {
        match &self.view {
            View::ProjectList => {
                self.view = View::RecentSessions;
                self.selected = 0;
                self.filter_text.clear();
                self.sessions.clear();
                self.filtered.clear();
                self.loading = true;
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

    pub fn finish_loading(&mut self) {
        if !self.loading {
            return;
        }
        self.loading = false;
        match &self.view {
            View::RecentSessions => self.load_all_sessions(),
            _ => {}
        }
    }
}

