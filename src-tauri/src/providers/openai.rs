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
        let request_body = serde_json::to_string(&body).unwrap_or_else(|_| "{\"error\":\"failed to serialize request body\"}".to_string());
        crate::services::debug_log::trace(
            "provider-text",
            format!(
                "request: url={}, model={}, system={}, user={}",
                url,
                body.model,
                crate::services::debug_log::truncate_for_log(&body.messages[0].content, 180),
                crate::services::debug_log::truncate_for_log(&body.messages[1].content, 420),
            ),
        );
        crate::services::debug_log::http(
            "provider-text",
            format!("request: method=POST, url={}, body={}", url, request_body),
        );
        let http_resp = self.client.post(&url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send().await;
        let http_resp = match http_resp {
            Ok(response) => response,
            Err(error) => {
                crate::services::debug_log::http(
                    "provider-text",
                    format!("response: error=request failed, latency_ms={}, detail={error}", start.elapsed().as_millis()),
                );
                return Err(error).context("request failed");
            }
        };
        let status = http_resp.status();
        let raw = match http_resp.text().await {
            Ok(raw) => raw,
            Err(error) => {
                crate::services::debug_log::http(
                    "provider-text",
                    format!("response: error=read response body failed, status={}, latency_ms={}, detail={error}", status.as_u16(), start.elapsed().as_millis()),
                );
                return Err(error).context("read response body failed");
            }
        };
        crate::services::debug_log::http(
            "provider-text",
            format!("response: status={}, latency_ms={}, body={}", status.as_u16(), start.elapsed().as_millis(), raw),
        );
        if !status.is_success() {
            crate::services::debug_log::trace(
                "provider-text",
                format!(
                    "response error: status={}, body={}",
                    status.as_u16(),
                    crate::services::debug_log::truncate_for_log(&raw, 420),
                ),
            );
            // Extract message from OpenAI-style error body if possible
            let msg = serde_json::from_str::<serde_json::Value>(&raw)
                .ok()
                .and_then(|v| v["error"]["message"].as_str().map(String::from))
                .unwrap_or_else(|| format!("HTTP {}: {}", status.as_u16(), raw));
            return Err(anyhow::anyhow!(msg));
        }
        let resp: Resp = serde_json::from_str(&raw).context("parse response failed")?;
        let content = resp.choices.into_iter().next()
            .map(|c| c.message.content)
            .unwrap_or_default();
        crate::services::debug_log::trace(
            "provider-text",
            format!(
                "response ok: status={}, latency_ms={}, content={}",
                status.as_u16(),
                start.elapsed().as_millis(),
                crate::services::debug_log::truncate_for_log(&content, 420),
            ),
        );
        Ok(ProviderResponse {
            content,
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
        let request_body = serde_json::to_string(&body).unwrap_or_else(|_| "{\"error\":\"failed to serialize vision request body\"}".to_string());
        crate::services::debug_log::trace(
            "provider-vision",
            format!(
                "request: url={}, model={}, user_text={}, image_mime={}, image_bytes={}",
                url,
                req.model,
                crate::services::debug_log::truncate_for_log(&req.user_text, 320),
                req.image_mime,
                req.image_base64.len(),
            ),
        );
        crate::services::debug_log::http(
            "provider-vision",
            format!("request: method=POST, url={}, body={}", url, request_body),
        );

        #[derive(Deserialize)]
        struct Choice { message: MsgContent }
        #[derive(Deserialize)]
        struct MsgContent { content: String }
        #[derive(Deserialize)]
        struct Resp { choices: Vec<Choice> }

        let http_resp = self.client.post(&url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send().await;
        let http_resp = match http_resp {
            Ok(response) => response,
            Err(error) => {
                crate::services::debug_log::http(
                    "provider-vision",
                    format!("response: error=vision request failed, latency_ms={}, detail={error}", start.elapsed().as_millis()),
                );
                return Err(error).context("vision request failed");
            }
        };
        let status = http_resp.status();
        let raw = match http_resp.text().await {
            Ok(raw) => raw,
            Err(error) => {
                crate::services::debug_log::http(
                    "provider-vision",
                    format!("response: error=read vision response body failed, status={}, latency_ms={}, detail={error}", status.as_u16(), start.elapsed().as_millis()),
                );
                return Err(error).context("read vision response body failed");
            }
        };
        crate::services::debug_log::http(
            "provider-vision",
            format!("response: status={}, latency_ms={}, body={}", status.as_u16(), start.elapsed().as_millis(), raw),
        );
        if !status.is_success() {
            crate::services::debug_log::trace(
                "provider-vision",
                format!(
                    "response error: status={}, body={}",
                    status.as_u16(),
                    crate::services::debug_log::truncate_for_log(&raw, 420),
                ),
            );
            let msg = serde_json::from_str::<serde_json::Value>(&raw)
                .ok()
                .and_then(|v| v["error"]["message"].as_str().map(String::from))
                .unwrap_or_else(|| format!("HTTP {}: {}", status.as_u16(), raw));
            return Err(anyhow::anyhow!(msg));
        }
        let resp: Resp = serde_json::from_str(&raw).context("vision parse response failed")?;
        let content = resp.choices.into_iter().next()
            .map(|c| c.message.content)
            .unwrap_or_default();
        crate::services::debug_log::trace(
            "provider-vision",
            format!(
                "response ok: status={}, latency_ms={}, content={}",
                status.as_u16(),
                start.elapsed().as_millis(),
                crate::services::debug_log::truncate_for_log(&content, 420),
            ),
        );

        Ok(ProviderResponse {
            content,
            latency_ms: start.elapsed().as_millis() as u64,
        })
    }
}
