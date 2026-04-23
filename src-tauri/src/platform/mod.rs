use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "macos")]
mod macos;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorPoint {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionProbe {
    pub selected_text: String,
    pub selection_signature: String,
    pub source_app: String,
    pub window_title: String,
    pub cursor: CursorPoint,
    pub selection_method: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionContext {
    pub selected_text: String,
    pub selection_signature: String,
    pub source_app: String,
    pub window_title: String,
    pub cursor: CursorPoint,
    pub window_rect: WindowRect,
    pub selection_method: String,
    pub window_text: String,
    pub screenshot_base64: String,
    pub screenshot_mime: String,
}

pub fn poll_selection() -> Result<Option<SelectionProbe>> {
    #[cfg(target_os = "windows")]
    {
        return windows::poll_selection();
    }

    #[cfg(target_os = "macos")]
    {
        return macos::poll_selection();
    }

    #[allow(unreachable_code)]
    Err(anyhow!("selection polling is not supported on this platform"))
}

pub fn capture_context() -> Result<Option<SelectionContext>> {
    #[cfg(target_os = "windows")]
    {
        return windows::capture_context();
    }

    #[cfg(target_os = "macos")]
    {
        return macos::capture_context();
    }

    #[allow(unreachable_code)]
    Err(anyhow!("context capture is not supported on this platform"))
}

pub fn primary_mouse_button_pressed() -> Result<bool> {
    #[cfg(target_os = "windows")]
    {
        return windows::primary_mouse_button_pressed();
    }

    #[allow(unreachable_code)]
    Ok(false)
}

pub fn is_our_window(source_app: &str, window_title: &str) -> bool {
    let source = source_app.trim().to_ascii_lowercase();
    let title = window_title.trim().to_ascii_lowercase();

    source == "select2explain"
        || title == "select2explain"
        || title == "select2explain popup"
}

/// Get the currently selected text in the frontmost application.
/// macOS: uses osascript to read system selection via Accessibility.
/// Returns None if nothing selected or no permission.
pub fn get_selected_text() -> Option<String> {
    #[cfg(target_os = "macos")]
    return macos::get_selected_text();
    #[cfg(not(target_os = "macos"))]
    None
}

/// Get all visible text from the frontmost window (best-effort).
pub fn get_window_text() -> Option<String> {
    #[cfg(target_os = "macos")]
    return macos::get_window_text();
    #[cfg(not(target_os = "macos"))]
    None
}

pub fn active_app_name() -> String {
    #[cfg(target_os = "macos")]
    return macos::active_app_name();
    #[cfg(not(target_os = "macos"))]
    "Unknown".to_string()
}
