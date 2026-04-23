use std::fs;

use crate::state::{normalize_ai_wait_ms, ProviderSettings};

const FAST_DEFAULT_MODEL: &str = "x-ai/grok-4.1-fast";
const PREVIOUS_FAST_DEFAULT_MODEL: &str = "google/gemini-2.5-flash-lite";
const LEGACY_DEFAULT_MODEL: &str = "qwen/qwen2.5-vl-72b-instruct";

pub fn load_settings() -> ProviderSettings {
    let Ok(path) = crate::services::app_paths::settings_path() else {
        return ProviderSettings::default();
    };

    let Ok(content) = fs::read_to_string(path) else {
        return ProviderSettings::default();
    };

    let mut settings: ProviderSettings = serde_json::from_str(&content).unwrap_or_default();
    let original_model = settings.model.clone();
    migrate_legacy_defaults(&mut settings);

    if settings.model != original_model {
        let _ = save_settings(&settings);
    }

    settings
}

pub fn save_settings(settings: &ProviderSettings) -> Result<(), String> {
    let path = crate::services::app_paths::settings_path().map_err(|e| e.to_string())?;
    let content = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(path, content).map_err(|e| e.to_string())
}

fn migrate_legacy_defaults(settings: &mut ProviderSettings) {
    if settings.model.trim().is_empty()
        || settings.model == LEGACY_DEFAULT_MODEL
        || settings.model == PREVIOUS_FAST_DEFAULT_MODEL
    {
        settings.model = FAST_DEFAULT_MODEL.to_string();
    }

    settings.max_ai_wait_ms = normalize_ai_wait_ms(settings.max_ai_wait_ms);
}