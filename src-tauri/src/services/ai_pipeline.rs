//! Two-stage AI pipeline:
//! Stage 1: Send screenshot + selected text to vision model. Ask if screenshot
//!          provides sufficient context to explain the selection.
//! Stage 2: If not sufficient, also send full window text as context.

use anyhow::Result;
use crate::providers::{OpenAiClient, TextRequest, VisionRequest};
use crate::state::ProviderSettings;

pub struct PipelineInput {
    pub selected_text: String,
    /// Full text from the active window (may be empty if unavailable)
    pub window_text: String,
    /// Base64-encoded screenshot PNG
    pub screenshot_b64: String,
    pub screenshot_mime: String,
}

pub struct PipelineResult {
    pub explanation: String,
    pub used_screenshot_only: bool,
    pub latency_ms: u64,
}

pub async fn run(input: PipelineInput, settings: &ProviderSettings, api_key: &str) -> Result<PipelineResult> {
    let client = OpenAiClient::new(&settings.base_url, api_key);
    let start = std::time::Instant::now();

    // --- Stage 1: Vision check ---
    let vision_system = "You are a context assessment assistant. \
        The user has selected some text in an application. \
        You will be given a screenshot of the active window and the selected text. \
        Your task: decide whether the screenshot alone provides SUFFICIENT context to explain \
        what the selected text means in context. \
        Reply with ONLY a JSON object: {\"sufficient\": true/false, \"reason\": \"...\"}. \
        Be conservative: if in doubt, say false.";

    let vision_user = format!(
        "Selected text: \"{}\"\n\nDoes the screenshot provide sufficient context to explain this text?",
        input.selected_text
    );

    let stage1 = client.vision(VisionRequest {
        model: settings.vision_model.clone(),
        system: vision_system.into(),
        user_text: vision_user,
        image_base64: input.screenshot_b64.clone(),
        image_mime: input.screenshot_mime.clone(),
    }).await?;

    let sufficient = parse_sufficient(&stage1.content);

    // --- Stage 2 or direct explanation ---
    let explain_system = "You are a context-aware reading assistant. \
        When given a selected text and its surrounding context, explain what the selected text \
        means IN THIS SPECIFIC CONTEXT. Be concise (2-4 sentences). \
        Always reply in the same language as the selected text. \
        Do NOT give a dictionary definition — explain the contextual meaning.";

    let explanation;
    let used_screenshot_only;

    if sufficient && !input.window_text.is_empty() {
        // Still use text context for better accuracy when we have it
        // (screenshot was just the sufficiency check)
        let user_msg = format!(
            "Selected text: \"{}\"\n\nContext from the window:\n{}",
            input.selected_text, truncate(&input.window_text, 4000)
        );
        let resp = client.text(TextRequest {
            model: settings.model.clone(),
            system: explain_system.into(),
            user: user_msg,
        }).await?;
        explanation = resp.content;
        used_screenshot_only = false;
    } else if sufficient {
        // Screenshot sufficient, no window text available — use vision model to explain
        let explain_user = format!(
            "Selected text: \"{}\"\n\nPlease explain what this text means in the context shown in the screenshot.",
            input.selected_text
        );
        let resp = client.vision(VisionRequest {
            model: settings.vision_model.clone(),
            system: explain_system.into(),
            user_text: explain_user,
            image_base64: input.screenshot_b64,
            image_mime: input.screenshot_mime,
        }).await?;
        explanation = resp.content;
        used_screenshot_only = true;
    } else {
        // Screenshot not sufficient — send full window text
        let user_msg = if input.window_text.is_empty() {
            format!("Selected text: \"{}\"\n\nNo additional context is available.", input.selected_text)
        } else {
            format!(
                "Selected text: \"{}\"\n\nFull context from the window:\n{}",
                input.selected_text, truncate(&input.window_text, 6000)
            )
        };
        let resp = client.text(TextRequest {
            model: settings.model.clone(),
            system: explain_system.into(),
            user: user_msg,
        }).await?;
        explanation = resp.content;
        used_screenshot_only = false;
    }

    Ok(PipelineResult {
        explanation,
        used_screenshot_only,
        latency_ms: start.elapsed().as_millis() as u64,
    })
}

fn parse_sufficient(content: &str) -> bool {
    // Try to parse {"sufficient": true/false, ...}
    let trimmed = content.trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
        return v.get("sufficient").and_then(|b| b.as_bool()).unwrap_or(false);
    }
    // Fallback: look for literal "true"
    content.to_lowercase().contains("\"sufficient\": true") || content.to_lowercase().contains("\"sufficient\":true")
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.len() <= max_chars {
        s.to_string()
    } else {
        format!("{}...[truncated]", &s[..max_chars])
    }
}
