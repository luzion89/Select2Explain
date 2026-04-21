//! Platform-specific utilities: selected text retrieval, window text, app name.

/// Get the currently selected text in the frontmost application.
/// macOS: uses osascript to read system selection via Accessibility.
/// Returns None if nothing selected or no permission.
pub fn get_selected_text() -> Option<String> {
    #[cfg(target_os = "macos")]
    return macos_get_selected_text();
    #[cfg(not(target_os = "macos"))]
    None
}

/// Get all visible text from the frontmost window (best-effort).
pub fn get_window_text() -> Option<String> {
    #[cfg(target_os = "macos")]
    return macos_get_window_text();
    #[cfg(not(target_os = "macos"))]
    None
}

pub fn active_app_name() -> String {
    #[cfg(target_os = "macos")]
    return macos_active_app();
    #[cfg(not(target_os = "macos"))]
    "Unknown".to_string()
}

#[cfg(target_os = "macos")]
fn macos_active_app() -> String {
    run_osascript(
        "tell application \"System Events\" to get name of first application process whose frontmost is true"
    ).unwrap_or_else(|| "Unknown".to_string())
}

#[cfg(target_os = "macos")]
fn macos_get_selected_text() -> Option<String> {
    // Read AXSelectedText from the focused UI element (e.g. a text field or editor),
    // not from the window — most apps only expose selection on the focused element.
    let script = r#"
tell application "System Events"
    set frontApp to first application process whose frontmost is true
    -- Skip our own process to avoid self-triggering
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
    run_osascript(script).filter(|s| !s.trim().is_empty())
}

#[cfg(target_os = "macos")]
fn macos_get_window_text() -> Option<String> {
    // Get all text elements from the frontmost window
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
    run_osascript(script).filter(|s| !s.trim().is_empty())
}

#[cfg(target_os = "macos")]
fn run_osascript(script: &str) -> Option<String> {
    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() { None } else { Some(text) }
}
