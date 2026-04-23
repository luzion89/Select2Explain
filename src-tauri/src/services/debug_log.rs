use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use anyhow::Result;
use chrono::Local;

struct DebugLogger {
    runtime_path: PathBuf,
    interaction_path: PathBuf,
    http_path: PathBuf,
    enabled: AtomicBool,
    write_lock: Mutex<()>,
}

impl DebugLogger {
    fn new(runtime_path: PathBuf, interaction_path: PathBuf, http_path: PathBuf) -> Self {
        Self {
            runtime_path,
            interaction_path,
            http_path,
            enabled: AtomicBool::new(false),
            write_lock: Mutex::new(()),
        }
    }

    fn write_line(&self, path: &PathBuf, line: &str) {
        let Ok(_guard) = self.write_lock.lock() else {
            return;
        };

        let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        else {
            return;
        };

        let _ = writeln!(file, "{line}");
    }
}

static LOGGER: OnceLock<DebugLogger> = OnceLock::new();

pub fn init() -> Result<PathBuf> {
    let path = crate::services::app_paths::debug_log_path()?;
    let interaction_path = crate::services::app_paths::interaction_log_path()?;
    let http_path = crate::services::app_paths::http_log_path()?;
    if LOGGER.get().is_none() {
        let _ = LOGGER.set(DebugLogger::new(path.clone(), interaction_path, http_path));
    }
    Ok(path)
}

pub fn set_enabled(enabled: bool) {
    if let Some(logger) = LOGGER.get() {
        logger.enabled.store(enabled, Ordering::Relaxed);
        let line = format_line("debug", if enabled { "debug logging enabled" } else { "debug logging disabled" });
        logger.write_line(&logger.runtime_path, &line);
        logger.write_line(&logger.interaction_path, &line);
        logger.write_line(&logger.http_path, &line);
    }
}

pub fn log(scope: &str, message: impl AsRef<str>) {
    let Some(logger) = LOGGER.get() else {
        return;
    };

    if !logger.enabled.load(Ordering::Relaxed) {
        return;
    }

    logger.write_line(&logger.runtime_path, &format_line(scope, message.as_ref()));
}

pub fn trace(scope: &str, message: impl AsRef<str>) {
    let Some(logger) = LOGGER.get() else {
        return;
    };

    if !logger.enabled.load(Ordering::Relaxed) {
        return;
    }

    logger.write_line(&logger.interaction_path, &format_line(scope, message.as_ref()));
}

pub fn http(scope: &str, message: impl AsRef<str>) {
    let Some(logger) = LOGGER.get() else {
        return;
    };

    if !logger.enabled.load(Ordering::Relaxed) {
        return;
    }

    logger.write_line(&logger.http_path, &format_line(scope, &sanitize_multiline(message.as_ref())));
}

pub fn current_log_path() -> Option<String> {
    LOGGER.get().map(|logger| logger.runtime_path.display().to_string())
}

pub fn current_interaction_log_path() -> Option<String> {
    LOGGER.get().map(|logger| logger.interaction_path.display().to_string())
}

pub fn current_http_log_path() -> Option<String> {
    LOGGER.get().map(|logger| logger.http_path.display().to_string())
}

pub fn truncate_for_log(value: &str, max_chars: usize) -> String {
    let flattened = redact_sensitive_tokens(&value
        .replace("\r\n", " ")
        .replace('\n', " ")
        .replace('\r', " ")
        .trim()
        .to_string());

    let char_count = flattened.chars().count();
    if char_count <= max_chars {
        return flattened;
    }

    let end = flattened
        .char_indices()
        .nth(max_chars)
        .map(|(index, _)| index)
        .unwrap_or(flattened.len());

    format!("{}...[truncated]", &flattened[..end])
}

fn redact_sensitive_tokens(value: &str) -> String {
    redact_prefixed_token(&redact_bearer_token(value), "sk-")
}

fn sanitize_multiline(value: &str) -> String {
    redact_sensitive_tokens(&value.replace("\r\n", "\\n").replace('\n', "\\n").replace('\r', "\\r"))
}

fn redact_bearer_token(value: &str) -> String {
    let mut output = String::new();
    let mut rest = value;

    loop {
        let Some(index) = rest.find("Bearer ") else {
            output.push_str(rest);
            break;
        };

        let prefix_end = index + "Bearer ".len();
        output.push_str(&rest[..prefix_end]);
        let token_len = rest[prefix_end..]
            .chars()
            .take_while(|ch| !ch.is_whitespace() && *ch != '"' && *ch != '\'' && *ch != ',' && *ch != ')')
            .count();

        if token_len == 0 {
            rest = &rest[prefix_end..];
            continue;
        }

        output.push_str("[redacted]");
        let consumed = rest[prefix_end..]
            .char_indices()
            .nth(token_len)
            .map(|(offset, _)| prefix_end + offset)
            .unwrap_or(rest.len());
        rest = &rest[consumed..];
    }

    output
}

fn redact_prefixed_token(value: &str, prefix: &str) -> String {
    let mut output = String::new();
    let mut rest = value;

    loop {
        let Some(index) = rest.find(prefix) else {
            output.push_str(rest);
            break;
        };

        output.push_str(&rest[..index]);
        output.push_str(prefix);
        output.push_str("[redacted]");

        let token_start = index + prefix.len();
        let token_len = rest[token_start..]
            .chars()
            .take_while(|ch| !ch.is_whitespace() && *ch != '"' && *ch != '\'' && *ch != ',' && *ch != ')')
            .count();
        let consumed = rest[token_start..]
            .char_indices()
            .nth(token_len)
            .map(|(offset, _)| token_start + offset)
            .unwrap_or(rest.len());

        rest = &rest[consumed..];
    }

    output
}

fn format_line(scope: &str, message: &str) -> String {
    format!("{} [{}] {}", Local::now().format("%Y-%m-%d %H:%M:%S%.3f"), scope, message)
}