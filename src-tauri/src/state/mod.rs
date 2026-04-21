use std::sync::Mutex;
use serde::{Deserialize, Serialize};

pub const KEYRING_SERVICE: &str = "com.tuntun.select2explain";
pub const KEYRING_USER: &str = "api_key";

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSettings {
    pub base_url: String,
    pub model: String,
    pub vision_model: String,
}

impl Default for ProviderSettings {
    fn default() -> Self {
        Self {
            base_url: "https://openrouter.ai/api/v1".into(),
            model: "qwen/qwen2.5-vl-72b-instruct".into(),
            vision_model: "qwen/qwen2.5-vl-72b-instruct".into(),
        }
    }
}

pub struct AppState {
    pub settings: Mutex<ProviderSettings>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            settings: Mutex::new(ProviderSettings::default()),
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
