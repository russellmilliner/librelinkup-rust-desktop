use std::collections::VecDeque;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use chrono::Local;
use serde::{Deserialize, Serialize};

const MAX_LOG_ENTRIES: usize = 1000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EventLogEntry {
    pub timestamp: String,
    pub level: String,
    pub category: String,
    pub message: String,
}

static EVENT_LOG_BUFFER: OnceLock<Mutex<VecDeque<EventLogEntry>>> = OnceLock::new();
static EVENT_LOG_SEQ: AtomicU64 = AtomicU64::new(1);

fn log_buffer() -> &'static Mutex<VecDeque<EventLogEntry>> {
    EVENT_LOG_BUFFER.get_or_init(|| Mutex::new(VecDeque::with_capacity(MAX_LOG_ENTRIES)))
}

pub fn get_event_seq() -> u64 {
    EVENT_LOG_SEQ.load(Ordering::Relaxed)
}

pub fn record_event(level: &str, category: &str, message: impl Into<String>) {
    let entry = EventLogEntry {
        timestamp: Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
        level: level.trim().to_uppercase(),
        category: category.trim().to_uppercase(),
        message: message.into(),
    };
    if let Ok(mut guard) = log_buffer().lock() {
        if guard.len() >= MAX_LOG_ENTRIES {
            guard.pop_front();
        }
        guard.push_back(entry);
        EVENT_LOG_SEQ.fetch_add(1, Ordering::Relaxed);
    }
}

pub fn log_info(category: &str, message: impl Into<String>) {
    record_event("INFO", category, message);
}

pub fn log_warn(category: &str, message: impl Into<String>) {
    record_event("WARN", category, message);
}

pub fn log_error(category: &str, message: impl Into<String>) {
    record_event("ERROR", category, message);
}

pub fn get_all_events() -> Vec<EventLogEntry> {
    if let Ok(guard) = log_buffer().lock() {
        guard.iter().cloned().collect()
    } else {
        Vec::new()
    }
}

pub fn filter_events(
    entries: &[EventLogEntry],
    category_filter: &str,
    level_filter: &str,
    search_query: &str,
) -> Vec<EventLogEntry> {
    let cat = category_filter.trim().to_uppercase();
    let lvl = level_filter.trim().to_uppercase();
    let q = search_query.trim().to_lowercase();

    entries
        .iter()
        .filter(|e| {
            if !cat.is_empty() && cat != "ALL" && e.category.to_uppercase() != cat {
                return false;
            }
            if !lvl.is_empty() && lvl != "ALL" && e.level.to_uppercase() != lvl {
                return false;
            }
            if !q.is_empty() {
                let hay = format!("{} {} {} {}", e.timestamp, e.level, e.category, e.message)
                    .to_lowercase();
                if !hay.contains(&q) {
                    return false;
                }
            }
            true
        })
        .cloned()
        .collect()
}

pub fn get_events_json() -> String {
    let events = get_all_events();
    serde_json::to_string(&events).unwrap_or_else(|_| "[]".to_string())
}

pub fn get_event_count() -> usize {
    if let Ok(guard) = log_buffer().lock() {
        guard.len()
    } else {
        0
    }
}

pub fn clear_events() {
    if let Ok(mut guard) = log_buffer().lock() {
        guard.clear();
        EVENT_LOG_SEQ.fetch_add(1, Ordering::Relaxed);
    }
}

pub fn format_events_as_text(entries: &[EventLogEntry]) -> String {
    let mut out = String::new();
    for e in entries {
        use std::fmt::Write;
        let _ = writeln!(
            &mut out,
            "[{}] [{:<5}] [{:<8}] {}",
            e.timestamp, e.level, e.category, e.message
        );
    }
    out
}

pub fn export_events_to_path(dest_path: &Path) -> Result<usize, String> {
    let clean_str = dest_path
        .to_string_lossy()
        .trim_start_matches("file://")
        .to_string();
    let target = Path::new(&clean_str);
    let entries = get_all_events();
    let text = format_events_as_text(&entries);
    crate::paths::write_file_atomic_0600(target, text.as_bytes())
        .map_err(|e| format!("Failed to write log file: {e}"))?;
    Ok(entries.len())
}

/// Masks an email address or username for safe diagnostic logging (e.g., `"alice@example.com"` -> `"a***@example.com"`).
pub fn mask_email(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return "[empty]".to_string();
    }
    if let Some((local, domain)) = trimmed.split_once('@') {
        let first = local.chars().next().unwrap_or('*');
        let dom_clean = if domain.is_empty() { "***" } else { domain };
        format!("{first}***@{dom_clean}")
    } else {
        let first = trimmed.chars().next().unwrap_or('*');
        format!("{first}***")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_emails_and_usernames_without_leaking_pii() {
        assert_eq!(mask_email("patient.name@example.com"), "p***@example.com");
        assert_eq!(mask_email("a@b.org"), "a***@b.org");
        assert_eq!(mask_email(" plainuser "), "p***");
        assert_eq!(mask_email(""), "[empty]");
    }
}
