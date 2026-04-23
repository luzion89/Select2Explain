use std::mem::size_of;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use base64::Engine;
use image::{codecs::png::PngEncoder, ColorType, ImageEncoder};
use uiautomation::core::{UIAutomation, UIElement};
use uiautomation::patterns::{UILegacyIAccessiblePattern, UITextPattern, UIValuePattern};
use uiautomation::types::{Handle, TreeScope};
use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, HGLOBAL, HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits,
    ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP, HDC,
    HGDIOBJ, SRCCOPY,
};
use windows::Win32::System::DataExchange::{
    CloseClipboard, GetClipboardData, GetClipboardSequenceNumber, OpenClipboard,
};
use windows::Win32::System::Memory::{GlobalLock, GlobalUnlock};
use windows::Win32::System::Ole::CF_UNICODETEXT;
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
    VIRTUAL_KEY, VK_CONTROL, VK_LBUTTON,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, GetForegroundWindow, GetWindowRect, GetWindowTextLengthW, GetWindowTextW,
    GetWindowThreadProcessId,
};

use super::{CursorPoint, SelectionContext, SelectionProbe, WindowRect};

static LAST_EMPTY_PROBE_SIGNATURE: OnceLock<Mutex<String>> = OnceLock::new();

#[derive(Clone, Copy)]
enum CaptureMode {
    Probe,
    Full,
}

impl CaptureMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Probe => "probe",
            Self::Full => "full",
        }
    }

    fn include_descendants(self) -> bool {
        matches!(self, Self::Probe | Self::Full)
    }

    fn allow_clipboard_fallback(self) -> bool {
        matches!(self, Self::Full)
    }

    fn include_window_text(self) -> bool {
        matches!(self, Self::Full)
    }

    fn include_screenshot(self) -> bool {
        false
    }
}

#[derive(Debug, Clone)]
struct CaptureEnvelope {
    source_app: String,
    window_title: String,
    cursor: CursorPoint,
    window_rect: WindowRect,
    selected_text: String,
    selection_method: String,
    window_text: String,
    screenshot_mime: String,
    screenshot_base64: String,
    selection_signature: String,
}

#[derive(Debug, Clone)]
struct WindowMeta {
    hwnd: HWND,
    source_app: String,
    window_title: String,
    window_rect: WindowRect,
}

#[derive(Debug, Clone)]
struct SelectionData {
    text: String,
    method: String,
}

pub fn poll_selection() -> Result<Option<SelectionProbe>> {
    let capture = capture(CaptureMode::Probe)?;
    let selected_text = capture.selected_text.trim().to_string();
    if selected_text.is_empty() {
        log_empty_probe_once(&capture);
        return Ok(None);
    }

    crate::services::debug_log::log(
        "windows-probe",
        format!(
            "probe success: app={}, window={}, method={}, text={}",
            capture.source_app,
            capture.window_title,
            capture.selection_method,
            crate::services::debug_log::truncate_for_log(&selected_text, 160),
        ),
    );
    crate::services::debug_log::trace(
        "windows-probe",
        format!(
            "probe success: app={}, window={}, method={}, cursor=({}, {}), signature={}, text={}",
            capture.source_app,
            capture.window_title,
            capture.selection_method,
            capture.cursor.x,
            capture.cursor.y,
            crate::services::debug_log::truncate_for_log(&capture.selection_signature, 220),
            crate::services::debug_log::truncate_for_log(&selected_text, 320),
        ),
    );

    Ok(Some(SelectionProbe {
        selected_text,
        selection_signature: capture.selection_signature,
        source_app: capture.source_app,
        window_title: capture.window_title,
        cursor: capture.cursor,
        selection_method: capture.selection_method,
    }))
}

pub fn capture_context() -> Result<Option<SelectionContext>> {
    let capture = capture(CaptureMode::Full)?;
    let selected_text = capture.selected_text.trim().to_string();
    if selected_text.is_empty() {
        crate::services::debug_log::log(
            "windows-capture",
            format!(
                "full capture returned empty selection: app={}, window={}, window_text_len={}, screenshot_bytes={}",
                capture.source_app,
                capture.window_title,
                capture.window_text.len(),
                capture.screenshot_base64.len(),
            ),
        );
    }

    crate::services::debug_log::log(
        "windows-capture",
        format!(
            "full capture success: app={}, window={}, method={}, screenshot_ok={}, screenshot_bytes={}, window_text_len={}",
            capture.source_app,
            capture.window_title,
            capture.selection_method,
            !capture.screenshot_base64.trim().is_empty(),
            capture.screenshot_base64.len(),
            capture.window_text.len(),
        ),
    );

    Ok(Some(SelectionContext {
        selected_text,
        selection_signature: capture.selection_signature,
        source_app: capture.source_app,
        window_title: capture.window_title,
        cursor: capture.cursor,
        window_rect: capture.window_rect,
        selection_method: capture.selection_method,
        window_text: capture.window_text,
        screenshot_base64: capture.screenshot_base64,
        screenshot_mime: capture.screenshot_mime,
    }))
}

pub fn primary_mouse_button_pressed() -> Result<bool> {
    let state = unsafe { GetAsyncKeyState(i32::from(VK_LBUTTON.0)) };
    Ok((u16::from_ne_bytes(state.to_ne_bytes()) & 0x8000) != 0)
}

fn capture(mode: CaptureMode) -> Result<CaptureEnvelope> {
    let started_at = Instant::now();
    crate::services::debug_log::trace(
        "windows-capture",
        format!("native capture start: mode={}", mode.as_str()),
    );

    let window = read_foreground_window()?;
    crate::services::debug_log::trace(
        "windows-capture",
        format!(
            "window meta: app={}, title={}, rect=({},{},{},{})",
            window.source_app,
            window.window_title,
            window.window_rect.x,
            window.window_rect.y,
            window.window_rect.width,
            window.window_rect.height,
        ),
    );

    let cursor = read_cursor_position().unwrap_or(CursorPoint {
        x: window.window_rect.x,
        y: window.window_rect.y,
    });
    crate::services::debug_log::trace(
        "windows-capture",
        format!("cursor captured: x={}, y={}", cursor.x, cursor.y),
    );

    let automation = UIAutomation::new().context("failed to initialize UI Automation")?;
    let root = automation
        .element_from_handle(Handle::from(window.hwnd.0 as isize))
        .ok();
    let selection = read_selection(&automation, root.as_ref(), mode)?;

    let window_text = if mode.include_window_text() {
        read_window_text(root.as_ref())?
    } else {
        String::new()
    };

    let (screenshot_mime, screenshot_base64) = if mode.include_screenshot() {
        (
            "image/png".to_string(),
            capture_window_png_base64(&window.window_rect)?,
        )
    } else {
        (String::new(), String::new())
    };

    let capture = CaptureEnvelope {
        selection_signature: format!(
            "{}|{}|{}",
            window.source_app, window.window_title, selection.text
        ),
        source_app: window.source_app,
        window_title: window.window_title,
        cursor,
        window_rect: window.window_rect,
        selected_text: selection.text,
        selection_method: selection.method,
        window_text,
        screenshot_mime,
        screenshot_base64,
    };

    crate::services::debug_log::trace(
        "windows-capture",
        format!(
            "native capture finish: mode={}, method={}, selected_len={}, window_text_len={}, screenshot_bytes={}, elapsed_ms={}",
            mode.as_str(),
            capture.selection_method,
            capture.selected_text.len(),
            capture.window_text.len(),
            capture.screenshot_base64.len(),
            started_at.elapsed().as_millis(),
        ),
    );

    Ok(capture)
}

fn read_foreground_window() -> Result<WindowMeta> {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.0.is_null() {
        return Err(anyhow!("failed to get foreground window"));
    }

    let mut rect = RECT::default();
    unsafe {
        GetWindowRect(hwnd, &mut rect).context("failed to read foreground window rect")?;
    }

    let mut process_id = 0u32;
    unsafe {
        let _ = GetWindowThreadProcessId(hwnd, Some(&mut process_id));
    }

    Ok(WindowMeta {
        hwnd,
        source_app: process_name_from_id(process_id).unwrap_or_else(|| format!("pid-{}", process_id)),
        window_title: get_window_title(hwnd)?,
        window_rect: WindowRect {
            x: rect.left,
            y: rect.top,
            width: (rect.right - rect.left).max(1),
            height: (rect.bottom - rect.top).max(1),
        },
    })
}

fn get_window_title(hwnd: HWND) -> Result<String> {
    let length = unsafe { GetWindowTextLengthW(hwnd) };
    if length == 0 {
        return Ok(String::new());
    }

    let mut buffer = vec![0u16; length as usize + 1];
    let written = unsafe { GetWindowTextW(hwnd, &mut buffer) };
    if written == 0 {
        return Ok(String::new());
    }

    Ok(String::from_utf16_lossy(&buffer[..written as usize]).trim().to_string())
}

fn process_name_from_id(process_id: u32) -> Option<String> {
    if process_id == 0 {
        return None;
    }

    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id).ok()?;
        let mut buffer = vec![0u16; 260];
        let mut length = buffer.len() as u32;
        let result = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_FORMAT(0),
            PWSTR(buffer.as_mut_ptr()),
            &mut length,
        );
        let _ = CloseHandle(handle);
        result.ok()?;

        let path = String::from_utf16_lossy(&buffer[..length as usize]);
        Path::new(&path)
            .file_stem()
            .and_then(|value| value.to_str())
            .map(|value| value.to_string())
    }
}

fn read_cursor_position() -> Result<CursorPoint> {
    let mut point = POINT::default();
    unsafe {
        GetCursorPos(&mut point).context("failed to read cursor position")?;
    }

    Ok(CursorPoint { x: point.x, y: point.y })
}

fn read_selection(
    automation: &UIAutomation,
    root: Option<&UIElement>,
    mode: CaptureMode,
) -> Result<SelectionData> {
    let focused = automation.get_focused_element().ok();
    if let Some(element) = focused.as_ref() {
        if let Some(text) = read_text_pattern_selection(element) {
            return Ok(SelectionData {
                text,
                method: "uia-focused".into(),
            });
        }
    }

    if mode.include_descendants() {
        if let Some(root) = root {
            crate::services::debug_log::trace("windows-capture", "descendant selection scan start");
            if let Some(text) = find_descendant_selection(automation, root) {
                crate::services::debug_log::trace("windows-capture", "descendant selection scan matched");
                return Ok(SelectionData {
                    text,
                    method: "uia-descendant".into(),
                });
            }
        }
    }

    if !mode.allow_clipboard_fallback() {
        return Ok(SelectionData {
            text: String::new(),
            method: String::new(),
        });
    }

    crate::services::debug_log::trace("windows-capture", "clipboard fallback start");
    let clipboard_text = read_clipboard_selection_fallback(focused.as_ref())?;
    if !clipboard_text.is_empty() {
        return Ok(SelectionData {
            text: clipboard_text,
            method: "clipboard-copy".into(),
        });
    }

    Ok(SelectionData {
        text: String::new(),
        method: String::new(),
    })
}

fn read_text_pattern_selection(element: &UIElement) -> Option<String> {
    let pattern = element.get_pattern::<UITextPattern>().ok()?;
    let ranges = pattern.get_selection().ok()?;
    let mut collected = Vec::new();
    for range in ranges {
        if let Ok(text) = range.get_text(-1) {
            let text = normalize_text(&text, 4000);
            if !text.is_empty() {
                collected.push(text);
            }
        }
    }

    let joined = collected.join("\n");
    if joined.is_empty() {
        None
    } else {
        Some(joined)
    }
}

fn read_value_text(element: &UIElement) -> Option<String> {
    element
        .get_pattern::<UIValuePattern>()
        .ok()
        .and_then(|pattern| pattern.get_value().ok())
        .map(|value| normalize_text(&value, 512))
        .filter(|value| !value.is_empty())
}

fn read_legacy_value(element: &UIElement) -> Option<String> {
    element
        .get_pattern::<UILegacyIAccessiblePattern>()
        .ok()
        .and_then(|pattern| pattern.get_value().ok())
        .map(|value| normalize_text(&value, 512))
        .filter(|value| !value.is_empty())
}

fn find_descendant_selection(automation: &UIAutomation, root: &UIElement) -> Option<String> {
    let condition = automation.create_true_condition().ok()?;
    let descendants = root.find_all(TreeScope::Descendants, &condition).ok()?;
    for element in descendants {
        if let Some(text) = read_text_pattern_selection(&element) {
            return Some(text);
        }
    }
    None
}

fn read_window_text(root: Option<&UIElement>) -> Result<String> {
    let Some(root) = root else {
        return Ok(String::new());
    };

    let automation = UIAutomation::new().context("failed to initialize UI Automation for window text")?;
    let condition = automation.create_true_condition().context("failed to create UIA true condition")?;
    let descendants = root
        .find_all(TreeScope::Descendants, &condition)
        .context("failed to enumerate window descendants")?;

    let mut values = Vec::new();
    push_unique_text(&mut values, root.get_name().ok(), 512);
    if let Some(text) = read_document_text(root, 3000) {
        push_unique_text(&mut values, Some(text), 3000);
    }
    push_unique_text(&mut values, read_value_text(root), 512);
    push_unique_text(&mut values, read_legacy_value(root), 512);

    for element in descendants {
        if joined_len(&values) >= 12_000 {
            break;
        }
        push_unique_text(&mut values, element.get_name().ok(), 512);
        if joined_len(&values) >= 12_000 {
            break;
        }
        push_unique_text(&mut values, read_value_text(&element), 512);
        if joined_len(&values) >= 12_000 {
            break;
        }
        push_unique_text(&mut values, read_legacy_value(&element), 512);
    }

    Ok(normalize_text(&values.join("\n"), 12_000))
}

fn read_document_text(element: &UIElement, limit: usize) -> Option<String> {
    let pattern = element.get_pattern::<UITextPattern>().ok()?;
    let range = pattern.get_document_range().ok()?;
    let text = range.get_text(limit as i32).ok()?;
    let text = normalize_text(&text, limit);
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn push_unique_text(values: &mut Vec<String>, maybe_text: Option<String>, limit: usize) {
    let Some(text) = maybe_text else {
        return;
    };
    let text = normalize_text(&text, limit);
    if text.len() < 2 || values.iter().any(|value| value == &text) {
        return;
    }
    values.push(text);
}

fn joined_len(values: &[String]) -> usize {
    values.iter().map(String::len).sum::<usize>() + values.len().saturating_sub(1)
}

fn read_clipboard_selection_fallback(focused: Option<&UIElement>) -> Result<String> {
    let before_sequence = unsafe { GetClipboardSequenceNumber() };

    if let Some(element) = focused {
        let _ = element.send_keys("{ctrl}(c)", 0);
    } else {
        send_ctrl_c()?;
    }

    let deadline = Instant::now() + Duration::from_millis(280);
    while Instant::now() < deadline {
        thread::sleep(Duration::from_millis(25));
        let current_sequence = unsafe { GetClipboardSequenceNumber() };
        if current_sequence != 0 && current_sequence != before_sequence {
            crate::services::debug_log::trace(
                "windows-capture",
                format!("clipboard fallback success: sequence={current_sequence}"),
            );
            return read_clipboard_unicode_text();
        }
    }

    crate::services::debug_log::trace(
        "windows-capture",
        "clipboard fallback returned empty selection",
    );
    Ok(String::new())
}

fn send_ctrl_c() -> Result<()> {
    let inputs = [
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(VK_CONTROL.0),
                    wScan: 0,
                    dwFlags: Default::default(),
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY('C' as u16),
                    wScan: 0,
                    dwFlags: Default::default(),
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY('C' as u16),
                    wScan: 0,
                    dwFlags: KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(VK_CONTROL.0),
                    wScan: 0,
                    dwFlags: KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
    ];

    let sent = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
    if sent == 0 {
        return Err(anyhow!("failed to send Ctrl+C fallback"));
    }
    Ok(())
}

fn read_clipboard_unicode_text() -> Result<String> {
    unsafe {
        OpenClipboard(None).context("failed to open clipboard")?;
    }

    let result = (|| {
        let handle = unsafe { GetClipboardData(CF_UNICODETEXT.0 as u32) }
            .context("failed to get clipboard data")?;
        let global = HGLOBAL(handle.0);
        let raw = unsafe { GlobalLock(global) };
        if raw.is_null() {
            return Err(anyhow!("failed to lock clipboard buffer"));
        }

        let text = unsafe {
            let mut length = 0usize;
            let ptr = raw as *const u16;
            while *ptr.add(length) != 0 {
                length += 1;
            }
            let slice = std::slice::from_raw_parts(ptr, length);
            String::from_utf16_lossy(slice)
        };

        unsafe {
            let _ = GlobalUnlock(global);
        }

        Ok(normalize_text(&text, 4000))
    })();

    unsafe {
        let _ = CloseClipboard();
    }

    result
}

fn capture_window_png_base64(rect: &WindowRect) -> Result<String> {
    unsafe {
        let screen_dc = GetDC(None);
        if screen_dc.0.is_null() {
            return Err(anyhow!("failed to get screen DC"));
        }

        let memory_dc = CreateCompatibleDC(Some(screen_dc));
        if memory_dc.0.is_null() {
            let _ = ReleaseDC(None, screen_dc);
            return Err(anyhow!("failed to create memory DC"));
        }

        let bitmap = CreateCompatibleBitmap(screen_dc, rect.width, rect.height);
        if bitmap.0.is_null() {
            let _ = DeleteDC(memory_dc);
            let _ = ReleaseDC(None, screen_dc);
            return Err(anyhow!("failed to create compatible bitmap"));
        }

        let previous = SelectObject(memory_dc, HGDIOBJ(bitmap.0));
        let result = BitBlt(
            memory_dc,
            0,
            0,
            rect.width,
            rect.height,
            Some(screen_dc),
            rect.x,
            rect.y,
            SRCCOPY,
        )
        .context("failed to capture window pixels")
        .and_then(|_| bitmap_to_png_base64(bitmap, memory_dc, rect.width, rect.height));

        let _ = SelectObject(memory_dc, previous);
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let _ = DeleteDC(memory_dc);
        let _ = ReleaseDC(None, screen_dc);

        result
    }
}

unsafe fn bitmap_to_png_base64(bitmap: HBITMAP, dc: HDC, width: i32, height: i32) -> Result<String> {
    let mut info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };

    let mut pixels = vec![0u8; (width * height * 4) as usize];
    let rows = GetDIBits(
        dc,
        bitmap,
        0,
        height as u32,
        Some(pixels.as_mut_ptr() as *mut _),
        &mut info,
        DIB_RGB_COLORS,
    );
    if rows == 0 {
        return Err(anyhow!("failed to read bitmap pixels"));
    }

    for pixel in pixels.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }

    let mut encoded = Vec::new();
    PngEncoder::new(&mut encoded)
        .write_image(&pixels, width as u32, height as u32, ColorType::Rgba8.into())
        .context("failed to encode screenshot png")?;

    Ok(base64::engine::general_purpose::STANDARD.encode(encoded))
}

fn normalize_text(value: &str, limit: usize) -> String {
    let normalized = value
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    if limit == 0 {
        return normalized;
    }

    normalized.chars().take(limit).collect()
}

fn log_empty_probe_once(capture: &CaptureEnvelope) {
    let signature = format!("{}|{}", capture.source_app, capture.window_title);
    let cache = LAST_EMPTY_PROBE_SIGNATURE.get_or_init(|| Mutex::new(String::new()));
    let Ok(mut previous) = cache.lock() else {
        return;
    };

    if *previous == signature {
        return;
    }

    *previous = signature;
    crate::services::debug_log::log(
        "windows-probe",
        format!(
            "probe found no accessible selection: app={}, window={}, focused_text={}",
            capture.source_app,
            capture.window_title,
            crate::services::debug_log::truncate_for_log(&capture.window_text, 160),
        ),
    );
    crate::services::debug_log::trace(
        "windows-probe",
        format!(
            "probe empty selection: app={}, window={}, method={}, cursor=({}, {}), focused_text={}",
            capture.source_app,
            capture.window_title,
            capture.selection_method,
            capture.cursor.x,
            capture.cursor.y,
            crate::services::debug_log::truncate_for_log(&capture.window_text, 220),
        ),
    );
}