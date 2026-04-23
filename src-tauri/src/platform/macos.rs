use std::process::Command;
use std::sync::{Mutex, OnceLock};

use serde::Deserialize;

use super::{CursorPoint, SelectionContext, SelectionProbe, WindowRect};

static LAST_EMPTY_PROBE_SIGNATURE: OnceLock<Mutex<String>> = OnceLock::new();

pub fn poll_selection() -> anyhow::Result<Option<SelectionProbe>> {
    let source_app = active_app_name();
    let window_title = active_window_title().unwrap_or_default();
    let selected_text = get_selected_text().unwrap_or_default().trim().to_string();

    if selected_text.is_empty() {
        log_empty_probe_once(&source_app, &window_title);
        return Ok(None);
    }

    let cursor = current_cursor_position().unwrap_or(CursorPoint { x: 0, y: 0 });

    crate::services::debug_log::log(
        "macos-probe",
        format!(
            "probe success: app={}, window={}, text={}",
            source_app,
            window_title,
            crate::services::debug_log::truncate_for_log(&selected_text, 160),
        ),
    );

    Ok(Some(SelectionProbe {
        selection_signature: format!("{}|{}|{}", source_app, window_title, selected_text),
        selected_text,
        source_app,
        window_title,
        cursor,
        selection_method: "accessibility".into(),
    }))
}

pub fn capture_context() -> anyhow::Result<Option<SelectionContext>> {
    let source_app = active_app_name();
    let window_title = active_window_title().unwrap_or_default();
    let selected_text = get_selected_text().unwrap_or_default().trim().to_string();

    if selected_text.is_empty() {
        crate::services::debug_log::log(
            "macos-capture",
            format!("full capture returned empty selection: app={}, window={}", source_app, window_title),
        );
        return Ok(None);
    }

    let cursor = current_cursor_position().unwrap_or(CursorPoint { x: 0, y: 0 });
    let window_text = get_window_text().unwrap_or_default();
    let (screenshot_base64, screenshot_mime) = crate::services::screenshot::capture_frontmost_window()?;

    crate::services::debug_log::log(
        "macos-capture",
        format!(
            "full capture success: app={}, window={}, screenshot_ok={}, screenshot_bytes={}, window_text_len={}",
            source_app,
            window_title,
            !screenshot_base64.trim().is_empty(),
            screenshot_base64.len(),
            window_text.len(),
        ),
    );

    Ok(Some(SelectionContext {
        selection_signature: format!("{}|{}|{}", source_app, window_title, selected_text),
        selected_text,
        source_app,
        window_title,
        cursor,
        window_rect: WindowRect {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        },
        selection_method: "accessibility".into(),
        window_text,
        screenshot_base64,
        screenshot_mime,
    }))
}

pub fn get_selected_text() -> Option<String> {
    let script = r#"
tell application "System Events"
    set frontApp to first application process whose frontmost is true
    if name of frontApp is "select2explain" then return ""
    try
        set focusedElem to value of attribute "AXFocusedUIElement" of frontApp
        if focusedElem is missing value then return ""
        set selText to value of attribute "AXSelectedText" of focusedElem
        if selText is missing value then return ""
        return selText
    on error
        return ""
    end try
end tell
"#;
    run_osascript(script).filter(|value| !value.trim().is_empty())
}

pub fn get_window_text() -> Option<String> {
    let script = r#"
tell application "System Events"
    set frontApp to first application process whose frontmost is true
    set frontWindow to missing value
    try
        set frontWindow to front window of frontApp
    end try
    if frontWindow is missing value then return ""
    set allText to {}
    try
        set uiElements to entire contents of frontWindow
        repeat with elem in uiElements
            try
                set elemVal to value of elem
                if elemVal is not missing value and class of elemVal is string and elemVal is not "" then
                    set end of allText to elemVal
                end if
            end try
        end repeat
    end try
    return allText as string
end tell
"#;
    run_osascript(script).filter(|value| !value.trim().is_empty())
}

pub fn active_app_name() -> String {
    run_osascript(
        "tell application \"System Events\" to get name of first application process whose frontmost is true"
    ).unwrap_or_else(|| "Unknown".to_string())
}

fn active_window_title() -> Option<String> {
    let script = r#"
tell application "System Events"
    try
        set frontApp to first application process whose frontmost is true
        set frontWindow to front window of frontApp
        return name of frontWindow
    on error
        return ""
    end try
end tell
"#;
    run_osascript(script).filter(|value| !value.trim().is_empty())
}

fn current_cursor_position() -> Option<CursorPoint> {
    #[derive(Deserialize)]
    struct CursorJson {
        x: i32,
        y: i32,
    }

    let script = r#"
ObjC.import('AppKit');
const point = $.NSEvent.mouseLocation;
const screen = $.NSScreen.mainScreen.frame;
JSON.stringify({
  x: Math.round(point.x),
  y: Math.round(screen.size.height - point.y)
});
"#;

    let output = Command::new("osascript")
        .args(["-l", "JavaScript", "-e", script])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    serde_json::from_str::<CursorJson>(&text)
        .ok()
        .map(|cursor| CursorPoint { x: cursor.x, y: cursor.y })
}

fn run_osascript(script: &str) -> Option<String> {
    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() { None } else { Some(text) }
}

fn log_empty_probe_once(source_app: &str, window_title: &str) {
    let signature = format!("{}|{}", source_app, window_title);
    let cache = LAST_EMPTY_PROBE_SIGNATURE.get_or_init(|| Mutex::new(String::new()));
    let Ok(mut previous) = cache.lock() else {
        return;
    };

    if *previous == signature {
        return;
    }

    *previous = signature;
    crate::services::debug_log::log(
        "macos-probe",
        format!("probe found no accessible selection: app={}, window={}", source_app, window_title),
    );
}