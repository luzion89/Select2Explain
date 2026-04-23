use tauri::{AppHandle, State};
use crate::state::{normalize_ai_wait_ms, AppState, ExplainResponse, ProviderSettings, KEYRING_SERVICE, KEYRING_USER};
use crate::services::ai_pipeline;
use crate::platform;

fn load_api_key() -> Option<String> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .ok()
        .and_then(|e| e.get_password().ok())
        .filter(|k| !k.trim().is_empty())
}

/// Called by frontend to save provider settings.
#[tauri::command]
pub fn save_settings(settings: ProviderSettings, state: State<'_, AppState>) -> Result<(), String> {
    let normalized = ProviderSettings {
        max_ai_wait_ms: normalize_ai_wait_ms(settings.max_ai_wait_ms),
        ..settings
    };
    let mut lock = state.settings.lock().map_err(|_| "lock error".to_string())?;
    *lock = normalized.clone();
    crate::services::settings_store::save_settings(&normalized)?;
    crate::services::debug_log::set_enabled(normalized.debug_logging_enabled);
    crate::services::debug_log::log(
        "settings",
        format!(
            "settings saved: translation_enabled={}, debug_logging_enabled={}, max_ai_wait_ms={}, base_url={}, model={}",
            normalized.translation_enabled,
            normalized.debug_logging_enabled,
            normalized.max_ai_wait_ms,
            normalized.base_url,
            normalized.model,
        ),
    );
    Ok(())
}

/// Called by frontend to save API key to system keychain.
#[tauri::command]
pub fn save_api_key(key: String) -> Result<(), String> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .map_err(|e| e.to_string())?
        .set_password(&key)
        .map_err(|e| e.to_string())
}

/// Called by frontend to delete API key.
#[tauri::command]
pub fn delete_api_key() -> Result<(), String> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .map_err(|e| e.to_string())?
        .set_password("")
        .map_err(|e| e.to_string())
}

/// Returns current settings and api_key_configured flag to frontend.
#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let s = state.settings.lock().map_err(|_| "lock error".to_string())?;
    let configured = load_api_key().is_some();
    Ok(serde_json::json!({
        "baseUrl": s.base_url,
        "model": s.model,
        "translationEnabled": s.translation_enabled,
        "debugLoggingEnabled": s.debug_logging_enabled,
        "apiKeyConfigured": configured,
        "maxAiWaitMs": s.max_ai_wait_ms,
        "debugLogPath": crate::services::debug_log::current_log_path(),
        "interactionLogPath": crate::services::debug_log::current_interaction_log_path(),
        "httpLogPath": crate::services::debug_log::current_http_log_path(),
    }))
}

#[tauri::command]
pub fn cancel_popup_request(app: AppHandle, reason: Option<String>) -> Result<(), String> {
    crate::cancel_active_request(
        &app,
        reason.as_deref().unwrap_or("popup cancel requested by frontend"),
        true,
    );
    Ok(())
}

#[tauri::command]
pub fn resize_popup_window(app: AppHandle, width: u32, height: u32) -> Result<(), String> {
    crate::resize_popup_window(&app, width, height)
}

/// Core command: capture current window text context, run text-only AI explanation, return result.
#[tauri::command]
pub async fn explain_selection(
    selected_text: String,
    state: State<'_, AppState>,
) -> Result<ExplainResponse, String> {
    let settings = state.settings.lock()
        .map_err(|_| "lock error".to_string())?
        .clone();

    let api_key = load_api_key().ok_or("API key not configured")?;
    let snapshot = tokio::task::spawn_blocking(platform::capture_context)
        .await
        .map_err(|e| format!("capture join failed: {e}"))?
        .map_err(|e| format!("capture failed: {e}"))?;

    let source_app = snapshot.as_ref()
        .map(|item| item.source_app.clone())
        .unwrap_or_else(platform::active_app_name);

    let selected_text = snapshot.as_ref()
        .map(|item| item.selected_text.clone())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(selected_text);

    let window_text = snapshot
        .map(|item| item.window_text)
        .unwrap_or_default();

    let result = ai_pipeline::run(
        ai_pipeline::PipelineInput {
            selected_text: selected_text.clone(),
            window_text,
        },
        &settings,
        &api_key,
    ).await.map_err(|e| format!("AI pipeline error: {e}"))?;

    Ok(ExplainResponse {
        explanation: result.explanation,
        source_app,
        latency_ms: result.latency_ms,
    })
}

/// Test API connectivity: send a trivial message to the configured provider.
/// Returns latency in ms on success, or an error string.
#[tauri::command]
pub async fn test_connection(state: State<'_, AppState>) -> Result<u64, String> {
    let settings = state.settings.lock()
        .map_err(|_| "lock error".to_string())?
        .clone();
    let api_key = load_api_key().ok_or("API key not configured")?;

    use crate::providers::{OpenAiClient, TextRequest};
    use std::time::Instant;
    let client = OpenAiClient::new(&settings.base_url, &api_key);
    let start = Instant::now();
    client.text(TextRequest {
        model: settings.model.clone(),
        system: "You are a test assistant.".into(),
        user: "Reply with just: ok".into(),
    }).await.map_err(|e| format!("Connection failed: {e}"))?;
    Ok(start.elapsed().as_millis() as u64)
}
