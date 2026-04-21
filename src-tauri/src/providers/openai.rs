use std::time::Instant;
use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use super::{ProviderResponse, TextRequest, VisionRequest};

pub struct OpenAiProvider {
    pub base_url: String,
    pub api_key: String,
    client: Client,
}

impl OpenAiProvider {
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            api_key: api_key.into(),
            client: Client::new(),
        }
    }

    pub async fn call_text(&self, req: TextRequest) -> Result<ProviderResponse> {
        #[derive(Serialize)]
        struct Msg { role: String, content: String }
        #[derive(Serialize)]
        struct Body { model: String, messages: Vec<Msg>, max_tokens: u32, temperature: f32 }
        #[derive(Deserialize)]
        struct Choice { message: MsgContent }
        #[derive(Deserialize)]
        struct MsgContent { content: String }
        #[derive(Deserialize)]
        struct Resp { choices: Vec<Choice> }

        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let start = Instant::now();
        let body = Body {
            model: req.model,
            messages: vec![
                Msg { role: "system".into(), content: req.system },
                Msg { role: "user".into(), content: req.user },
            ],
            max_tokens: 1024,
            temperature: 0.3,
        };
        let http_resp = self.client.post(&url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send().await.context("request failed")?;
        let status = http_resp.status();
        let raw = http_resp.text().await.context("read response body failed")?;
        if !status.is_success() {
            // Extract message from OpenAI-style error body if possible
            let msg = serde_json::from_str::<serde_json::Value>(&raw)
                .ok()
                .and_then(|v| v["error"]["message"].as_str().map(String::from))
                .unwrap_or_else(|| format!("HTTP {}: {}", status.as_u16(), raw));
            return Err(anyhow::anyhow!(msg));
        }
        let resp: Resp = serde_json::from_str(&raw).context("parse response failed")?;
        Ok(ProviderResponse {
            content: resp.choices.into_iter().next()
                .map(|c| c.message.content)
                .unwrap_or_default(),
            latency_ms: start.elapsed().as_millis() as u64,
        })
    }

    pub async fn call_vision(&self, req: VisionRequest) -> Result<ProviderResponse> {
        use serde_json::{json, Value};

        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let start = Instant::now();

        let image_url = format!("data:{};base64,{}", req.image_mime, req.image_base64);
        let user_content: Value = json!([
            { "type": "text", "text": req.user_text },
            { "type": "image_url", "image_url": { "url": image_url, "detail": "low" } }
        ]);

        let body: Value = json!({
            "model": req.model,
            "messages": [
                { "role": "system", "content": req.system },
                { "role": "user", "content": user_content }
            ],
            "max_tokens": 512,
            "temperature": 0.2
        });

        #[derive(Deserialize)]
        struct Choice { message: MsgContent }
        #[derive(Deserialize)]
        struct MsgContent { content: String }
        #[derive(Deserialize)]
        struct Resp { choices: Vec<Choice> }

        let http_resp = self.client.post(&url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send().await.context("vision request failed")?;
        let status = http_resp.status();
        let raw = http_resp.text().await.context("read vision response body failed")?;
        if !status.is_success() {
            let msg = serde_json::from_str::<serde_json::Value>(&raw)
                .ok()
                .and_then(|v| v["error"]["message"].as_str().map(String::from))
                .unwrap_or_else(|| format!("HTTP {}: {}", status.as_u16(), raw));
            return Err(anyhow::anyhow!(msg));
        }
        let resp: Resp = serde_json::from_str(&raw).context("vision parse response failed")?;

        Ok(ProviderResponse {
            content: resp.choices.into_iter().next()
                .map(|c| c.message.content)
                .unwrap_or_default(),
            latency_ms: start.elapsed().as_millis() as u64,
        })
    }
}
