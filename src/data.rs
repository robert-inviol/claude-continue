use chrono::{DateTime, Utc};
use serde_json::Value;
use std::cmp::Reverse;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Project {
    pub dir_name: String,
    pub display_path: String,
    pub session_count: usize,
    pub newest_mtime: f64,
    pub time_ago: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SessionInfo {
    pub session_id: String,
    pub first_msg: String,
    pub first_ts: String,
    pub last_ts: String,
    pub cwd: String,
    pub model: String,
    pub user_msgs: usize,
    pub assistant_msgs: usize,
    pub total_msgs: usize,
    pub file_size: String,
    pub first_ts_ago: String,
    pub last_ts_ago: String,
    pub user_messages: Vec<String>,
    pub proj_path: PathBuf,
    pub jsonl_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConversationMessage {
    pub role: String,
    pub text: String,
    pub timestamp: String,
}

pub fn projects_dir() -> PathBuf {
    dirs::home_dir()
        .expect("no home directory")
        .join(".claude")
        .join("projects")
}

fn home_dir_str() -> String {
    dirs::home_dir()
        .expect("no home directory")
        .to_string_lossy()
        .to_string()
}

pub fn time_ago_from_epoch(epoch_secs: f64) -> String {
    let now = Utc::now().timestamp() as f64;
    let secs = (now - epoch_secs).max(0.0) as u64;
    if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86400 {
        format!("{}h ago", secs / 3600)
    } else if secs < 604800 {
        format!("{}d ago", secs / 86400)
    } else {
        let dt = DateTime::from_timestamp(epoch_secs as i64, 0)
            .unwrap_or_default();
        dt.format("%Y-%m-%d").to_string()
    }
}

pub fn time_ago_from_iso(ts: &str) -> String {
    if ts.is_empty() {
        return "unknown".to_string();
    }
    let ts_clean = ts.replace("Z", "+00:00");
    match DateTime::parse_from_rfc3339(&ts_clean) {
        Ok(dt) => {
            let now = Utc::now();
            let secs = (now - dt.with_timezone(&Utc))
                .num_seconds()
                .max(0) as u64;
            if secs < 60 {
                format!("{}s ago", secs)
            } else if secs < 3600 {
                format!("{}m ago", secs / 60)
            } else if secs < 86400 {
                format!("{}h ago", secs / 3600)
            } else if secs < 604800 {
                format!("{}d ago", secs / 86400)
            } else {
                dt.format("%Y-%m-%d").to_string()
            }
        }
        Err(_) => {
            if ts.len() >= 10 {
                ts[..10].to_string()
            } else {
                "unknown".to_string()
            }
        }
    }
}

fn fmt_size(bytes: u64) -> String {
    if bytes > 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes > 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

pub fn list_projects() -> Vec<Project> {
    let base = projects_dir();
    let home = home_dir_str();

    let Ok(entries) = fs::read_dir(&base) else {
        return vec![];
    };

    let mut results: Vec<Project> = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let dir_name = entry.file_name().to_string_lossy().to_string();
        let proj_path = base.join(&dir_name);

        let sessions: Vec<PathBuf> = glob::glob(
            &format!("{}/*.jsonl", proj_path.display()),
        )
        .ok()
        .map(|g| g.flatten().collect())
        .unwrap_or_default();

        if sessions.is_empty() {
            continue;
        }

        let newest = sessions
            .iter()
            .max_by_key(|p| {
                p.metadata()
                    .map(|m| m.modified().unwrap_or(std::time::UNIX_EPOCH))
                    .unwrap_or(std::time::UNIX_EPOCH)
            })
            .unwrap();

        let mtime = newest
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .map(|t| {
                t.duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs_f64()
            })
            .unwrap_or(0.0);

        // Try to extract cwd from newest session
        let decoded = extract_cwd_from_file(newest)
            .unwrap_or_else(|| {
                let trimmed = dir_name.trim_start_matches('-');
                format!("/{}", trimmed.replace('-', "/"))
            });

        let short = decoded.replace(&home, "~");
        let short = if short == "~" {
            "~/".to_string()
        } else {
            short
        };

        results.push(Project {
            dir_name,
            display_path: short,
            session_count: sessions.len(),
            newest_mtime: mtime,
            time_ago: time_ago_from_epoch(mtime),
        });
    }

    results.sort_by(|a, b| {
        b.newest_mtime
            .partial_cmp(&a.newest_mtime)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results
}

fn extract_cwd_from_file(path: &Path) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    for line in content.lines() {
        if let Ok(obj) = serde_json::from_str::<Value>(line) {
            if obj.get("type").and_then(|t| t.as_str()) == Some("user") {
                if let Some(cwd) = obj.get("cwd").and_then(|c| c.as_str()) {
                    return Some(cwd.to_string());
                }
            }
        }
    }
    None
}

pub fn scan_session(path: &Path) -> SessionInfo {
    let home = home_dir_str();
    let mut first_msg = String::new();
    let mut first_ts = String::new();
    let mut last_ts = String::new();
    let mut cwd = String::new();
    let mut model = String::new();
    let mut user_count = 0usize;
    let mut asst_count = 0usize;
    let mut user_messages: Vec<String> = Vec::new();

    if let Ok(content) = fs::read_to_string(path) {
        for line in content.lines() {
            let obj: Value = match serde_json::from_str(line) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let t = obj
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            match t {
                "user" => {
                    user_count += 1;
                    if let Some(ts) = obj.get("timestamp").and_then(|v| v.as_str()) {
                        if first_ts.is_empty() {
                            first_ts = ts.to_string();
                        }
                        last_ts = ts.to_string();
                    }
                    if cwd.is_empty() {
                        if let Some(c) = obj.get("cwd").and_then(|v| v.as_str()) {
                            cwd = c.to_string();
                        }
                    }
                    if let Some(msg) = obj
                        .pointer("/message/content")
                        .and_then(|v| v.as_str())
                    {
                        let clean: String = msg
                            .replace('\n', " ")
                            .replace('\t', " ");
                        let truncated: String =
                            clean.chars().take(200).collect();
                        user_messages.push(truncated);
                        if first_msg.is_empty() {
                            first_msg = clean.chars().take(120).collect();
                        }
                    }
                }
                "assistant" => {
                    asst_count += 1;
                    if let Some(ts) = obj.get("timestamp").and_then(|v| v.as_str()) {
                        last_ts = ts.to_string();
                    }
                    if model.is_empty() {
                        if let Some(m) = obj
                            .pointer("/message/model")
                            .and_then(|v| v.as_str())
                        {
                            model = m.to_string();
                        }
                    }
                }
                _ => {}
            }
        }
    }

    let sid = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let total = user_count + asst_count;
    let fsize = path
        .metadata()
        .map(|m| fmt_size(m.len()))
        .unwrap_or_else(|_| "? B".to_string());

    let cwd_display = cwd.replace(&home, "~");

    SessionInfo {
        session_id: sid,
        first_msg: if first_msg.is_empty() {
            "(no message)".to_string()
        } else {
            first_msg
        },
        first_ts_ago: time_ago_from_iso(&first_ts),
        last_ts_ago: time_ago_from_iso(&last_ts),
        first_ts,
        last_ts,
        cwd: cwd_display,
        model: if model.is_empty() {
            "unknown".to_string()
        } else {
            model
        },
        user_msgs: user_count,
        assistant_msgs: asst_count,
        total_msgs: total,
        file_size: fsize,
        user_messages,
        proj_path: path.parent().unwrap_or(Path::new("")).to_path_buf(),
        jsonl_path: path.to_path_buf(),
    }
}

pub fn list_sessions(proj_path: &Path) -> Vec<SessionInfo> {
    let pattern = format!("{}/*.jsonl", proj_path.display());
    let mut files: Vec<PathBuf> = glob::glob(&pattern)
        .ok()
        .map(|g| g.flatten().collect())
        .unwrap_or_default();

    files.sort_by_key(|p| {
        Reverse(
            p.metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .unwrap_or(std::time::UNIX_EPOCH),
        )
    });

    files.iter().map(|p| scan_session(p)).collect()
}

pub fn list_all_sessions() -> Vec<SessionInfo> {
    let base = projects_dir();
    let Ok(entries) = fs::read_dir(&base) else {
        return vec![];
    };

    let mut all: Vec<(f64, PathBuf)> = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let pattern = format!("{}/*.jsonl", path.display());
        if let Ok(files) = glob::glob(&pattern) {
            for file in files.flatten() {
                let mtime = file
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .map(|t| {
                        t.duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs_f64()
                    })
                    .unwrap_or(0.0);
                all.push((mtime, file));
            }
        }
    }

    all.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    all.iter()
        .map(|(_, p)| scan_session(p))
        .filter(|s| s.total_msgs > 0)
        .collect()
}

pub fn read_conversation(path: &Path) -> Vec<ConversationMessage> {
    let mut messages = Vec::new();
    let Ok(content) = fs::read_to_string(path) else {
        return messages;
    };

    for line in content.lines() {
        let obj: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let t = obj
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let ts = obj
            .get("timestamp")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        match t {
            "user" => {
                if let Some(msg) = obj
                    .pointer("/message/content")
                    .and_then(|v| v.as_str())
                {
                    let trimmed = msg.trim();
                    if !trimmed.is_empty() {
                        messages.push(ConversationMessage {
                            role: "user".to_string(),
                            text: trimmed.to_string(),
                            timestamp: ts,
                        });
                    }
                }
            }
            "assistant" => {
                let content_parts = obj.pointer("/message/content");
                let mut text_parts = Vec::new();
                let mut tool_parts = Vec::new();

                if let Some(Value::Array(parts)) = content_parts {
                    for part in parts {
                        if let Some(ptype) = part.get("type").and_then(|v| v.as_str()) {
                            match ptype {
                                "text" => {
                                    if let Some(text) =
                                        part.get("text").and_then(|v| v.as_str())
                                    {
                                        let trimmed = text.trim();
                                        if !trimmed.is_empty() {
                                            text_parts.push(trimmed.to_string());
                                        }
                                    }
                                }
                                "tool_use" => {
                                    let name = part
                                        .get("name")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("?");
                                    tool_parts.push(format!("[tool: {}]", name));
                                }
                                _ => {}
                            }
                        }
                    }
                }

                let combined = if text_parts.is_empty() && !tool_parts.is_empty() {
                    tool_parts.join(" ")
                } else if !tool_parts.is_empty() {
                    format!("{}  {}", text_parts.join(" "), tool_parts.join(" "))
                } else {
                    text_parts.join(" ")
                };

                if !combined.is_empty() {
                    messages.push(ConversationMessage {
                        role: "assistant".to_string(),
                        text: combined,
                        timestamp: ts,
                    });
                }
            }
            _ => {}
        }
    }
    messages
}

pub fn cwd_to_project_dir(cwd: &str) -> Option<String> {
    let base = projects_dir();
    let dir_name = cwd.replace('/', "-");
    let proj_path = base.join(&dir_name);
    if proj_path.is_dir() {
        let pattern = format!("{}/*.jsonl", proj_path.display());
        if let Ok(mut files) = glob::glob(&pattern) {
            if files.next().is_some() {
                return Some(dir_name);
            }
        }
    }
    None
}

pub fn lookup_session(query: &str) -> Vec<(String, PathBuf)> {
    let base = projects_dir();
    let query_lower = query.to_lowercase();
    let mut matches = Vec::new();

    let Ok(entries) = fs::read_dir(&base) else {
        return matches;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let pattern = format!("{}/*.jsonl", path.display());
        if let Ok(files) = glob::glob(&pattern) {
            for file in files.flatten() {
                let basename = file
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_lowercase();
                if basename.contains(&query_lower) {
                    matches.push((
                        file.file_stem()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string(),
                        path.clone(),
                    ));
                }
            }
        }
    }
    matches
}
