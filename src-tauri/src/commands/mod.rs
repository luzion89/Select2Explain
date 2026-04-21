use tauri::State;
use crate::state::{AppState, ExplainResponse, ProviderSettings, KEYRING_SERVICE, KEYRING_USER};
use crate::services::{screenshot, ai_pipeline};
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
    let mut lock = state.settings.lock().map_err(|_| "lock error".to_string())?;
    *lock = settings;
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
        "visionModel": s.vision_model,
        "apiKeyConfigured": configured,
    }))
}

/// Core command: take screenshot, run two-stage AI, return explanation.
/// Frontend calls this when it detects selection has changed (via polling event).
#[tauri::command]
pub async fn explain_selection(
    selected_text: String,
    state: State<'_, AppState>,
) -> Result<ExplainResponse, String> {
    let settings = state.settings.lock()
        .map_err(|_| "lock error".to_string())?
        .clone();

    let api_key = load_api_key().ok_or("API key not configured")?;
    let source_app = platform::active_app_name();

    // Capture screenshot
    let (screenshot_b64, screenshot_mime) = screenshot::capture_frontmost_window()
        .map_err(|e| format!("screenshot failed: {e}"))?;

    // Get window text (best-effort, may be empty)
    let window_text = platform::get_window_text().unwrap_or_default();

    let result = ai_pipeline::run(
        ai_pipeline::PipelineInput {
            selected_text: selected_text.clone(),
            window_text,
            screenshot_b64,
            screenshot_mime,
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
