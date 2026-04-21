pub mod openai;

use anyhow::Result;

pub struct TextRequest {
    pub model: String,
    pub system: String,
    pub user: String,
}

pub struct VisionRequest {
    pub model: String,
    pub system: String,
    pub user_text: String,
    /// PNG/JPEG bytes, base64 encoded
    pub image_base64: String,
    pub image_mime: String,
}

pub struct ProviderResponse {
    pub content: String,
    pub latency_ms: u64,
}

pub struct OpenAiClient {
    inner: openai::OpenAiProvider,
}

impl OpenAiClient {
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            inner: openai::OpenAiProvider::new(base_url, api_key),
        }
    }

    pub async fn text(&self, req: TextRequest) -> Result<ProviderResponse> {
        self.inner.call_text(req).await
    }

    pub async fn vision(&self, req: VisionRequest) -> Result<ProviderResponse> {
        self.inner.call_vision(req).await
    }
}
