#![cfg_attr(windows, windows_subsystem = "windows")]

use std::time::{Duration, Instant};

mod commands;
mod platform;
mod providers;
mod services;
mod state;

use tauri::{
    image::Image,
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, Position, Size, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent,
};
use tauri_plugin_global_shortcut::{Builder as GlobalShortcutBuilder, ShortcutState};
use state::{AppState, PopupLoadingPayload, PopupPayload, KEYRING_SERVICE, KEYRING_USER};

#[cfg(windows)]
use windows::Win32::{
    Graphics::Gdi::{CreateRoundRectRgn, DeleteObject, SetWindowRgn},
    UI::WindowsAndMessaging::GetWindowRect,
};

const TRANSLATION_TOGGLE_SHORTCUT: &str = "Ctrl+Alt+Q";
#[cfg(windows)]
const POPUP_CORNER_RADIUS: i32 = 28;

#[cfg(target_os = "windows")]
#[derive(Default)]
struct MouseReleaseGate {
    primary_button_down: bool,
    pending_release_at: Option<Instant>,
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let settings = crate::services::settings_store::load_settings();
            let log_path = crate::services::debug_log::init()?;
            let interaction_log_path = crate::services::debug_log::current_interaction_log_path()
                .unwrap_or_else(|| "unavailable".to_string());
            let app_icon = crate::services::app_icon::build_app_icon();
            crate::services::debug_log::set_enabled(settings.debug_logging_enabled);
            crate::services::debug_log::log(
                "startup",
                format!(
                    "application setup complete: runtime_log_path={}, interaction_log_path={}, translation_enabled={}, debug_logging_enabled={}",
                    log_path.display(),
                    interaction_log_path,
                    settings.translation_enabled,
                    settings.debug_logging_enabled,
                ),
            );
            crate::services::debug_log::trace(
                "startup",
                format!(
                    "setup complete: runtime_log_path={}, interaction_log_path={}, translation_enabled={}, debug_logging_enabled={}",
                    log_path.display(),
                    crate::services::debug_log::current_interaction_log_path().unwrap_or_default(),
                    settings.translation_enabled,
                    settings.debug_logging_enabled,
                ),
            );

            app.manage(AppState::new(settings));
            app.handle().plugin(
                GlobalShortcutBuilder::new()
                    .with_shortcut("ctrl+alt+q")?
                    .with_handler(|app, _shortcut, event| {
                        if event.state == ShortcutState::Pressed {
                            if let Err(error) = toggle_translation_enabled_from_hotkey(app) {
                                crate::services::debug_log::log(
                                    "settings",
                                    format!("failed to toggle translation via hotkey: {error}"),
                                );
                            }
                        }
                    })
                    .build(),
            )?;
            create_popup_window(&app.handle(), app_icon.clone())?;
            create_tray(&app.handle(), app_icon.clone())?;
            apply_window_icons(&app.handle(), app_icon);
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(selection_monitor(handle));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::save_api_key,
            commands::delete_api_key,
            commands::cancel_popup_request,
            commands::resize_popup_window,
            commands::explain_selection,
            commands::test_connection,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Select2Explain");
}

fn toggle_translation_enabled_from_hotkey(app: &AppHandle) -> Result<(), String> {
    let next_settings = {
        let state = app.state::<AppState>();
        let mut settings = state.settings.lock().map_err(|_| "settings lock error".to_string())?;
        settings.translation_enabled = !settings.translation_enabled;
        crate::services::settings_store::save_settings(&settings)?;
        settings.clone()
    };

    if !next_settings.translation_enabled {
        cancel_active_request(app, "translation disabled via hotkey", true);
    }

    crate::services::debug_log::log(
        "settings",
        format!(
            "translation toggled via hotkey {}: translation_enabled={}",
            TRANSLATION_TOGGLE_SHORTCUT,
            next_settings.translation_enabled,
        ),
    );

    let _ = app.emit(
        "settings:changed",
        serde_json::json!({
            "baseUrl": next_settings.base_url,
            "model": next_settings.model,
            "translationEnabled": next_settings.translation_enabled,
            "debugLoggingEnabled": next_settings.debug_logging_enabled,
            "maxAiWaitMs": next_settings.max_ai_wait_ms,
            "apiKeyConfigured": load_api_key().is_some(),
            "debugLogPath": crate::services::debug_log::current_log_path(),
            "interactionLogPath": crate::services::debug_log::current_interaction_log_path(),
            "httpLogPath": crate::services::debug_log::current_http_log_path(),
        }),
    );

    Ok(())
}

async fn selection_monitor(app: AppHandle) {
    #[cfg(target_os = "windows")]
    let mut mouse_release_gate = MouseReleaseGate::default();

    loop {
        #[cfg(target_os = "windows")]
        tokio::time::sleep(Duration::from_millis(25)).await;

        #[cfg(not(target_os = "windows"))]
        tokio::time::sleep(Duration::from_millis(650)).await;

        maybe_hide_unfocused_popup(&app);

        let main_window = app.get_webview_window("main");
        let main_window_visible = main_window
            .as_ref()
            .and_then(|window| window.is_visible().ok())
            .unwrap_or(false);
        let main_window_focused = main_window
            .as_ref()
            .and_then(|window| window.is_focused().ok())
            .unwrap_or(false);

        if main_window_focused {
            reset_selection_signature(&app);
            #[cfg(target_os = "windows")]
            reset_mouse_release_gate(&mut mouse_release_gate);
            update_monitor_status(
                &app,
                "main-window-focused",
                format!(
                    "monitor paused because main window is focused: main_visible={}, main_focused={}",
                    main_window_visible,
                    main_window_focused,
                ),
            );
            continue;
        }

        let settings = match app.state::<AppState>().settings.lock() {
            Ok(guard) => guard.clone(),
            Err(_) => continue,
        };

        if !settings.translation_enabled {
            reset_selection_signature(&app);
            #[cfg(target_os = "windows")]
            reset_mouse_release_gate(&mut mouse_release_gate);
            update_monitor_status(&app, "translation-disabled", "monitor paused because translation is disabled");
            continue;
        }

        #[cfg(target_os = "windows")]
        update_monitor_status(
            &app,
            "waiting-mouse-release",
            format!(
                "monitor armed for mouse release: main_visible={}, main_focused={}, translation_enabled={}",
                main_window_visible,
                main_window_focused,
                settings.translation_enabled,
            ),
        );

        #[cfg(not(target_os = "windows"))]
        update_monitor_status(
            &app,
            "polling-active",
            format!(
                "monitor active: main_visible={}, main_focused={}, translation_enabled={}",
                main_window_visible,
                main_window_focused,
                settings.translation_enabled,
            ),
        );

        #[cfg(target_os = "windows")]
        let should_probe = match should_probe_after_mouse_release(&mut mouse_release_gate) {
            Ok(should_probe) => should_probe,
            Err(error) => {
                crate::services::debug_log::log("selection-monitor", format!("mouse trigger failed: {error}"));
                crate::services::debug_log::trace("selection-monitor", format!("mouse trigger failed: {error}"));
                continue;
            }
        };

        #[cfg(not(target_os = "windows"))]
        let should_probe = true;

        if !should_probe {
            continue;
        }

        #[cfg(target_os = "windows")]
        maybe_cancel_loading_popup_on_mouse_release(&app);

        #[cfg(target_os = "windows")]
        update_monitor_status(
            &app,
            "probing-after-mouse-release",
            "mouse primary button released; probing selection",
        );

        let probe = match tokio::task::spawn_blocking(platform::poll_selection).await {
            Ok(Ok(probe)) => probe,
            Ok(Err(error)) => {
                eprintln!("selection probe failed: {error}");
                crate::services::debug_log::log("selection-monitor", format!("selection probe failed: {error}"));
                crate::services::debug_log::trace("selection-monitor", format!("selection probe failed: {error}"));
                continue;
            }
            Err(error) => {
                eprintln!("selection probe join failed: {error}");
                crate::services::debug_log::log("selection-monitor", format!("selection probe join failed: {error}"));
                crate::services::debug_log::trace("selection-monitor", format!("selection probe join failed: {error}"));
                continue;
            }
        };

        let Some(probe) = probe else {
            reset_selection_signature(&app);
            continue;
        };

        crate::services::debug_log::log(
            "selection-monitor",
            format!(
                "new selection probe: app={}, window={}, method={}, cursor=({}, {}), text={}",
                probe.source_app,
                probe.window_title,
                probe.selection_method,
                probe.cursor.x,
                probe.cursor.y,
                crate::services::debug_log::truncate_for_log(&probe.selected_text, 160),
            ),
        );
        crate::services::debug_log::trace(
            "selection-monitor",
            format!(
                "probe accepted: app={}, window={}, method={}, cursor=({}, {}), signature={}, text={}",
                probe.source_app,
                probe.window_title,
                probe.selection_method,
                probe.cursor.x,
                probe.cursor.y,
                crate::services::debug_log::truncate_for_log(&probe.selection_signature, 220),
                crate::services::debug_log::truncate_for_log(&probe.selected_text, 320),
            ),
        );

        if platform::is_our_window(&probe.source_app, &probe.window_title) {
            crate::services::debug_log::log("selection-monitor", "ignored self window selection");
            crate::services::debug_log::trace(
                "selection-monitor",
                format!(
                    "ignored self window selection: app={}, window={}",
                    probe.source_app,
                    probe.window_title,
                ),
            );
            continue;
        }

        let request_id = {
            let state = app.state::<AppState>();
            let Ok(mut runtime) = state.runtime.lock() else {
                continue;
            };

            if runtime.in_flight || runtime.last_selection_signature == probe.selection_signature {
                crate::services::debug_log::log("selection-monitor", "ignored duplicate or in-flight selection");
                None
            } else {
                runtime.in_flight = true;
                runtime.active_request_id = runtime.active_request_id.wrapping_add(1);
                runtime.last_selection_signature = probe.selection_signature.clone();
                Some(runtime.active_request_id)
            }
        };

        if let Some(request_id) = request_id {
            let handle = app.clone();
            let selection_task = tokio::spawn(async move {
                process_selection(handle, probe, request_id).await;
            });
            let abort_handle = selection_task.abort_handle();
            if let Ok(mut runtime) = app.state::<AppState>().runtime.lock() {
                if runtime.active_request_id == request_id {
                    runtime.active_request_abort = Some(abort_handle);
                }
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn reset_mouse_release_gate(gate: &mut MouseReleaseGate) {
    gate.primary_button_down = false;
    gate.pending_release_at = None;
}

#[cfg(target_os = "windows")]
fn should_probe_after_mouse_release(gate: &mut MouseReleaseGate) -> Result<bool, String> {
    let primary_button_down = platform::primary_mouse_button_pressed()
        .map_err(|error| format!("failed to read primary mouse button state: {error}"))?;

    if primary_button_down {
        if !gate.primary_button_down {
            gate.pending_release_at = None;
            crate::services::debug_log::trace("selection-monitor", "mouse primary down detected");
        }
        gate.primary_button_down = true;
        return Ok(false);
    }

    if gate.primary_button_down {
        gate.primary_button_down = false;
        gate.pending_release_at = Some(Instant::now());
        crate::services::debug_log::log("selection-monitor", "mouse primary up detected; scheduling selection probe");
        crate::services::debug_log::trace("selection-monitor", "mouse primary up detected; scheduling selection probe");
        return Ok(false);
    }

    if let Some(released_at) = gate.pending_release_at {
        if released_at.elapsed() < Duration::from_millis(120) {
            return Ok(false);
        }

        gate.pending_release_at = None;
        crate::services::debug_log::trace("selection-monitor", "mouse release debounce elapsed; probing selection");
        return Ok(true);
    }

    Ok(false)
}

async fn process_selection(app: AppHandle, probe: platform::SelectionProbe, request_id: u64) {
    crate::services::debug_log::trace(
        "selection-monitor",
        format!(
            "process_selection start: app={}, window={}, method={}, cursor=({}, {}), text={}",
            probe.source_app,
            probe.window_title,
            probe.selection_method,
            probe.cursor.x,
            probe.cursor.y,
            crate::services::debug_log::truncate_for_log(&probe.selected_text, 320),
        ),
    );

    let snapshot = match capture_popup_context(&probe).await {
        Ok(snapshot) => snapshot,
        Err(error) => {
            if request_is_current(&app, request_id) {
                let payload = PopupPayload {
                    selected_text: probe.selected_text.clone(),
                    explanation: String::new(),
                    source_app: probe.source_app.clone(),
                    window_title: probe.window_title.clone(),
                    latency_ms: 0,
                    error,
                    cursor_x: probe.cursor.x,
                    cursor_y: probe.cursor.y,
                };

                if let Some(window) = app.get_webview_window("popup") {
                    reveal_popup_window(&app, &window, payload.cursor_x, payload.cursor_y, "show");
                    let _ = window.emit("popup:show", payload);
                }
            }
            finish_active_request(&app, request_id);
            return;
        }
    };

    if !request_is_current(&app, request_id) {
        return;
    }

    let loading = PopupLoadingPayload {
        selected_text: snapshot.selected_text.clone(),
        source_app: snapshot.source_app.clone(),
        window_title: snapshot.window_title.clone(),
        cursor_x: snapshot.cursor.x,
        cursor_y: snapshot.cursor.y,
        max_wait_ms: app.state::<AppState>().settings.lock().ok().map(|settings| settings.max_ai_wait_ms).unwrap_or(crate::state::DEFAULT_MAX_AI_WAIT_MS),
    };

    if let Some(window) = app.get_webview_window("popup") {
        reveal_popup_window(&app, &window, loading.cursor_x, loading.cursor_y, "loading");
        crate::services::debug_log::log(
            "popup",
            format!(
                "popup:loading emitted: app={}, window={}, cursor=({}, {}), selected_text={}",
                loading.source_app,
                loading.window_title,
                loading.cursor_x,
                loading.cursor_y,
                crate::services::debug_log::truncate_for_log(&loading.selected_text, 180),
            ),
        );
        crate::services::debug_log::trace(
            "popup",
            format!(
                "loading emitted: app={}, window={}, cursor=({}, {}), selected_text={}",
                loading.source_app,
                loading.window_title,
                loading.cursor_x,
                loading.cursor_y,
                crate::services::debug_log::truncate_for_log(&loading.selected_text, 320),
            ),
        );
        let emit_result = window.emit("popup:loading", loading);
        if let Err(error) = emit_result {
            crate::services::debug_log::log("popup", format!("popup:loading emit failed: {error}"));
        }
    }

    let fallback = PopupPayload {
        selected_text: snapshot.selected_text.clone(),
        explanation: String::new(),
        source_app: snapshot.source_app.clone(),
        window_title: snapshot.window_title.clone(),
        latency_ms: 0,
        error: String::new(),
        cursor_x: snapshot.cursor.x,
        cursor_y: snapshot.cursor.y,
    };

    let payload = match build_popup_payload(&app, snapshot).await {
        Ok(payload) => payload,
        Err(error) => PopupPayload { error, ..fallback },
    };

    if !request_is_current(&app, request_id) {
        return;
    }

    if let Some(window) = app.get_webview_window("popup") {
        reveal_popup_window(&app, &window, payload.cursor_x, payload.cursor_y, "show");
        crate::services::debug_log::log(
            "popup",
            format!(
                "popup:show emitted: app={}, window={}, cursor=({}, {}), has_error={}, selected_text={}",
                payload.source_app,
                payload.window_title,
                payload.cursor_x,
                payload.cursor_y,
                !payload.error.trim().is_empty(),
                crate::services::debug_log::truncate_for_log(&payload.selected_text, 180),
            ),
        );
        crate::services::debug_log::trace(
            "popup",
            format!(
                "payload emitted: app={}, window={}, cursor=({}, {}), error={}, explanation={}",
                payload.source_app,
                payload.window_title,
                payload.cursor_x,
                payload.cursor_y,
                crate::services::debug_log::truncate_for_log(&payload.error, 240),
                crate::services::debug_log::truncate_for_log(&payload.explanation, 420),
            ),
        );
        let emit_result = window.emit("popup:show", payload);
        if let Err(error) = emit_result {
            crate::services::debug_log::log("popup", format!("popup:show emit failed: {error}"));
        }
    }

    finish_active_request(&app, request_id);
}

fn reveal_popup_window(app: &AppHandle, window: &WebviewWindow, cursor_x: i32, cursor_y: i32, phase: &str) {
    let visible_before = window.is_visible().ok().unwrap_or(false);
    let focused_before = window.is_focused().ok().unwrap_or(false);
    let fallback_x = cursor_x.saturating_add(18);
    let fallback_y = cursor_y.saturating_sub(18).max(8);
    let should_focus = phase == "show";

    let _ = window.set_position(Position::Physical(PhysicalPosition::new(fallback_x, fallback_y)));
    let _ = window.show();
    if should_focus {
        let _ = window.set_focus();
    }

    if let Ok(mut runtime) = app.state::<AppState>().runtime.lock() {
        runtime.popup_interactive = should_focus;
        runtime.popup_last_revealed_at = Some(Instant::now());
    }

    let visible_after = window.is_visible().ok().unwrap_or(false);
    let focused_after = window.is_focused().ok().unwrap_or(false);
    crate::services::debug_log::log(
        "popup",
        format!(
            "popup native reveal: phase={}, cursor=({}, {}), fallback_position=({}, {}), visible_before={}, visible_after={}, focused_before={}, focused_after={}",
            phase,
            cursor_x,
            cursor_y,
            fallback_x,
            fallback_y,
            visible_before,
            visible_after,
            focused_before,
            focused_after,
        ),
    );
}

fn maybe_hide_unfocused_popup(app: &AppHandle) {
    let Some(window) = app.get_webview_window("popup") else {
        return;
    };

    let visible = window.is_visible().ok().unwrap_or(false);
    if !visible {
        if let Ok(mut runtime) = app.state::<AppState>().runtime.lock() {
            runtime.popup_interactive = false;
            runtime.popup_last_revealed_at = None;
        }
        return;
    }

    let focused = window.is_focused().ok().unwrap_or(false);
    let should_hide = {
        let state = app.state::<AppState>();
        let Ok(runtime) = state.runtime.lock() else {
            return;
        };

        runtime.popup_interactive
            && runtime
                .popup_last_revealed_at
                .map(|revealed_at| !focused && revealed_at.elapsed() > Duration::from_millis(220))
                .unwrap_or(false)
    };

    if should_hide {
        cancel_active_request(app, "popup hidden by backend blur fallback", true);
    }
}

#[cfg(target_os = "windows")]
fn maybe_cancel_loading_popup_on_mouse_release(app: &AppHandle) {
    let should_cancel = {
        let state = app.state::<AppState>();
        let Ok(runtime) = state.runtime.lock() else {
            return;
        };

        runtime.in_flight
    };

    if !should_cancel {
        return;
    }

    let Some(window) = app.get_webview_window("popup") else {
        return;
    };

    let visible = window.is_visible().ok().unwrap_or(false);
    if !visible {
        return;
    }

    crate::services::debug_log::log(
        "popup",
        "mouse release detected while AI request is loading; cancelling popup request",
    );
    cancel_active_request(app, "popup dismissed by external mouse release during loading", true);
}

async fn capture_popup_context(probe: &platform::SelectionProbe) -> Result<platform::SelectionContext, String> {
    let mut snapshot = tokio::task::spawn_blocking(platform::capture_context)
        .await
        .map_err(|e| format!("capture join failed: {e}"))?
        .map_err(|e| format!("capture failed: {e}"))?
        .unwrap_or_else(|| platform::SelectionContext {
            selected_text: probe.selected_text.clone(),
            selection_signature: probe.selection_signature.clone(),
            source_app: probe.source_app.clone(),
            window_title: probe.window_title.clone(),
            cursor: probe.cursor.clone(),
            window_rect: platform::WindowRect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            selection_method: probe.selection_method.clone(),
            window_text: String::new(),
            screenshot_base64: String::new(),
            screenshot_mime: String::new(),
        });

    if snapshot.selected_text.trim().is_empty() {
        snapshot.selected_text = probe.selected_text.clone();
        snapshot.selection_signature = probe.selection_signature.clone();
        if snapshot.selection_method.trim().is_empty() {
            snapshot.selection_method = "probe-fallback".into();
        } else {
            snapshot.selection_method = format!("{}/probe-fallback", snapshot.selection_method);
        }
    }

    crate::services::debug_log::log(
        "capture",
        format!(
            "context captured: app={}, window={}, method={}, screenshot_ok={}, screenshot_bytes={}, window_text_len={}, window_text={}",
            snapshot.source_app,
            snapshot.window_title,
            snapshot.selection_method,
            !snapshot.screenshot_base64.trim().is_empty(),
            snapshot.screenshot_base64.len(),
            snapshot.window_text.len(),
            crate::services::debug_log::truncate_for_log(&snapshot.window_text, 300),
        ),
    );
    crate::services::debug_log::trace(
        "capture",
        format!(
            "snapshot: app={}, window={}, method={}, cursor=({}, {}), window_rect=({}, {}, {}, {}), screenshot_mime={}, screenshot_bytes={}, selected_text={}, window_text={}",
            snapshot.source_app,
            snapshot.window_title,
            snapshot.selection_method,
            snapshot.cursor.x,
            snapshot.cursor.y,
            snapshot.window_rect.x,
            snapshot.window_rect.y,
            snapshot.window_rect.width,
            snapshot.window_rect.height,
            snapshot.screenshot_mime,
            snapshot.screenshot_base64.len(),
            crate::services::debug_log::truncate_for_log(&snapshot.selected_text, 320),
            crate::services::debug_log::truncate_for_log(&snapshot.window_text, 520),
        ),
    );

    if snapshot.screenshot_base64.trim().is_empty() {
        crate::services::debug_log::log(
            "capture",
            format!(
                "text-only pipeline continuing without screenshot: app={}, window={}, method={}, window_text_len={}",
                snapshot.source_app,
                snapshot.window_title,
                snapshot.selection_method,
                snapshot.window_text.len(),
            ),
        );
    }

    Ok(snapshot)
}

async fn build_popup_payload(app: &AppHandle, snapshot: platform::SelectionContext) -> Result<PopupPayload, String> {
    let settings = app.state::<AppState>().settings.lock()
        .map_err(|_| "settings lock error".to_string())?
        .clone();
    let api_key = load_api_key().ok_or("API key 未配置，请先在设置页保存 Key".to_string())?;
    let max_wait_ms = settings.max_ai_wait_ms;

    let result = match tokio::time::timeout(
        Duration::from_millis(max_wait_ms),
        crate::services::ai_pipeline::run(
            crate::services::ai_pipeline::PipelineInput {
                selected_text: snapshot.selected_text.clone(),
                window_text: snapshot.window_text.clone(),
            },
            &settings,
            &api_key,
        ),
    )
    .await
    {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => return Err(format!("AI pipeline error: {error}")),
        Err(_) => {
            crate::services::debug_log::log(
                "ai-pipeline",
                format!("pipeline timeout after {} ms", max_wait_ms),
            );
            return Err(format!("AI 请求超时（{} 秒），已自动取消。", max_wait_ms / 1000));
        }
    };

    Ok(PopupPayload {
        selected_text: snapshot.selected_text,
        explanation: result.explanation,
        source_app: snapshot.source_app,
        window_title: snapshot.window_title,
        latency_ms: result.latency_ms,
        error: String::new(),
        cursor_x: snapshot.cursor.x,
        cursor_y: snapshot.cursor.y,
    })
}

fn request_is_current(app: &AppHandle, request_id: u64) -> bool {
    let state = app.state::<AppState>();
    let Ok(runtime) = state.runtime.lock() else {
        return false;
    };

    runtime.active_request_id == request_id
}

fn finish_active_request(app: &AppHandle, request_id: u64) {
    if let Ok(mut runtime) = app.state::<AppState>().runtime.lock() {
        if runtime.active_request_id == request_id {
            runtime.in_flight = false;
            runtime.active_request_abort = None;
        }
    }
}

pub(crate) fn cancel_active_request(app: &AppHandle, reason: &str, hide_popup: bool) {
    let abort_handle = {
        let state = app.state::<AppState>();
        let Ok(mut runtime) = state.runtime.lock() else {
            return;
        };

        runtime.active_request_id = runtime.active_request_id.wrapping_add(1);
        runtime.in_flight = false;
        runtime.popup_interactive = false;
        runtime.popup_last_revealed_at = None;
        runtime.active_request_abort.take()
    };

    if let Some(abort_handle) = abort_handle {
        abort_handle.abort();
        crate::services::debug_log::log("popup", format!("active request cancelled: {reason}"));
    }

    reset_selection_signature(app);

    if hide_popup {
        if let Some(window) = app.get_webview_window("popup") {
            let _ = window.hide();
        }
        let _ = app.emit("popup:clear", serde_json::json!({}));
    }
}

pub(crate) fn resize_popup_window(app: &AppHandle, width: u32, height: u32) -> Result<(), String> {
    let Some(window) = app.get_webview_window("popup") else {
        return Err("popup window not found".to_string());
    };

    let target_width = width.max(320);
    let target_height = height.max(168);
    window
        .set_size(Size::Physical(PhysicalSize::new(target_width, target_height)))
        .map_err(|error| format!("failed to resize popup window: {error}"))?;

    let applied = window
        .outer_size()
        .map(|size| format!("{}x{}", size.width, size.height))
        .unwrap_or_else(|_| "unavailable".to_string());

    crate::services::debug_log::log(
        "popup",
        format!(
            "popup resized by backend: requested={}x{}, applied={}",
            target_width,
            target_height,
            applied,
        ),
    );

    Ok(())
}

fn reset_selection_signature(app: &AppHandle) {
    if let Ok(mut runtime) = app.state::<AppState>().runtime.lock() {
        runtime.last_selection_signature.clear();
    }
}

fn update_monitor_status(app: &AppHandle, status: &str, message: impl Into<String>) {
    let message = message.into();
    let should_log = {
        let state = app.state::<AppState>();
        let Ok(mut runtime) = state.runtime.lock() else {
            return;
        };

        if runtime.monitor_status == status {
            false
        } else {
            runtime.monitor_status = status.to_string();
            true
        }
    };

    if should_log {
        crate::services::debug_log::log("selection-monitor", &message);
        crate::services::debug_log::trace("selection-monitor", message);
    }
}

fn create_popup_window(app: &AppHandle, app_icon: Image<'static>) -> tauri::Result<()> {
    if app.get_webview_window("popup").is_some() {
        return Ok(());
    }

    let popup = WebviewWindowBuilder::new(app, "popup", WebviewUrl::default())
        .title("Select2Explain Popup")
        .icon(app_icon)?
        .inner_size(408.0, 248.0)
        .visible(false)
        .transparent(true)
        .decorations(false)
        .shadow(false)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .build()?;

    #[cfg(windows)]
    {
        let _ = popup.set_shadow(false);
        apply_popup_window_region(&popup);

        let popup_clone = popup.clone();
        popup.on_window_event(move |event| {
            if matches!(event, WindowEvent::Resized(_)) {
                apply_popup_window_region(&popup_clone);
            }
        });
    }

    Ok(())
}

#[cfg(windows)]
fn apply_popup_window_region(window: &WebviewWindow) {
    let Ok(hwnd) = window.hwnd() else {
        crate::services::debug_log::log("popup", "failed to resolve popup hwnd for native region");
        return;
    };

    let mut rect = Default::default();
    if let Err(error) = unsafe { GetWindowRect(hwnd, &mut rect) } {
        crate::services::debug_log::log("popup", format!("failed to read popup bounds for native region: {error}"));
        return;
    }

    let width = (rect.right - rect.left).max(1);
    let height = (rect.bottom - rect.top).max(1);
    let diameter = POPUP_CORNER_RADIUS * 2;
    let region = unsafe { CreateRoundRectRgn(0, 0, width, height, diameter, diameter) };

    if region.0.is_null() {
        crate::services::debug_log::log("popup", "failed to create rounded popup region");
        return;
    }

    let applied = unsafe { SetWindowRgn(hwnd, Some(region), true) } != 0;
    if !applied {
        let _ = unsafe { DeleteObject(region.into()) };
        crate::services::debug_log::log("popup", "failed to apply rounded popup region");
    }
}

fn create_tray(app: &AppHandle, app_icon: Image<'static>) -> tauri::Result<()> {
    let show_item = MenuItemBuilder::new("打开控制面板")
        .id("show")
        .build(app)?;
    let quit_item = MenuItemBuilder::new("退出 Select2Explain")
        .id("quit")
        .build(app)?;

    let menu = MenuBuilder::new(app)
        .items(&[&show_item, &quit_item])
        .build()?;

    TrayIconBuilder::new()
        .icon(app_icon)
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main_window(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;

    Ok(())
}

fn apply_window_icons(app: &AppHandle, app_icon: Image<'static>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_icon(app_icon.clone());
    }

    if let Some(window) = app.get_webview_window("popup") {
        let _ = window.set_icon(app_icon);
    }
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_always_on_top(true);
        let _ = window.set_focus();
        let _ = window.set_always_on_top(false);
        crate::services::debug_log::log("tray", "main window restored from tray menu");
    }
}

fn load_api_key() -> Option<String> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .ok()
        .and_then(|entry| entry.get_password().ok())
        .filter(|value| !value.trim().is_empty())
}
