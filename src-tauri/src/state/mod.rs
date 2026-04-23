use std::{sync::Mutex, time::Instant};
use serde::{Deserialize, Serialize};
use tokio::task::AbortHandle;

pub const KEYRING_SERVICE: &str = "com.tuntun.select2explain";
pub const KEYRING_USER: &str = "api_key";
pub const DEFAULT_MAX_AI_WAIT_MS: u64 = 15_000;
pub const MIN_AI_WAIT_MS: u64 = 5_000;
pub const MAX_AI_WAIT_MS: u64 = 120_000;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSettings {
    pub base_url: String,
    pub model: String,
    pub translation_enabled: bool,
    pub debug_logging_enabled: bool,
    pub max_ai_wait_ms: u64,
}

impl Default for ProviderSettings {
    fn default() -> Self {
        Self {
            base_url: "https://openrouter.ai/api/v1".into(),
            model: "x-ai/grok-4.1-fast".into(),
            translation_enabled: false,
            debug_logging_enabled: false,
            max_ai_wait_ms: DEFAULT_MAX_AI_WAIT_MS,
        }
    }
}

pub fn normalize_ai_wait_ms(value: u64) -> u64 {
    value.clamp(MIN_AI_WAIT_MS, MAX_AI_WAIT_MS)
}

#[derive(Default)]
pub struct RuntimeState {
    pub active_request_id: u64,
    pub last_selection_signature: String,
    pub in_flight: bool,
    pub monitor_status: String,
    pub popup_interactive: bool,
    pub popup_last_revealed_at: Option<Instant>,
    pub active_request_abort: Option<AbortHandle>,
}

pub struct AppState {
    pub settings: Mutex<ProviderSettings>,
    pub runtime: Mutex<RuntimeState>,
}

impl AppState {
    pub fn new(settings: ProviderSettings) -> Self {
        Self {
            settings: Mutex::new(settings),
            runtime: Mutex::new(RuntimeState::default()),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplainResponse {
    pub explanation: String,
    pub source_app: String,
    pub latency_ms: u64,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PopupLoadingPayload {
    pub selected_text: String,
    pub source_app: String,
    pub window_title: String,
    pub cursor_x: i32,
    pub cursor_y: i32,
    pub max_wait_ms: u64,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PopupPayload {
    pub selected_text: String,
    pub explanation: String,
    pub source_app: String,
    pub window_title: String,
    pub latency_ms: u64,
    pub error: String,
    pub cursor_x: i32,
    pub cursor_y: i32,
}
