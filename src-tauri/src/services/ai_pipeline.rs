//! Single-stage AI pipeline:
//! Send selected text plus full window text directly to a fast text model.

use anyhow::Result;
use crate::providers::{OpenAiClient, TextRequest};
use crate::state::ProviderSettings;

const MAX_PROMPT_CONTEXT_CHARS: usize = 3600;

pub struct PipelineInput {
    pub selected_text: String,
    /// Full text from the active window (may be empty if unavailable)
    pub window_text: String,
}

pub struct PipelineResult {
    pub explanation: String,
    pub latency_ms: u64,
}

pub async fn run(input: PipelineInput, settings: &ProviderSettings, api_key: &str) -> Result<PipelineResult> {
    let client = OpenAiClient::new(&settings.base_url, api_key);
    let start = std::time::Instant::now();
    let (prompt_context, context_strategy) = build_prompt_context(&input.selected_text, &input.window_text);

    crate::services::debug_log::log(
        "ai-pipeline",
        format!(
            "pipeline start: selected_text={}, window_text_len={}, model={}",
            crate::services::debug_log::truncate_for_log(&input.selected_text, 160),
            input.window_text.len(),
            settings.model,
        ),
    );
    crate::services::debug_log::trace(
        "ai-pipeline",
        format!(
            "pipeline input: selected_text={}, model={}, window_text={}",
            crate::services::debug_log::truncate_for_log(&input.selected_text, 240),
            settings.model,
            crate::services::debug_log::truncate_for_log(&input.window_text, 480),
        ),
    );

    let explain_system = "You are a context-aware reading assistant. \
        When given a selected text and its surrounding context, explain what the selected text \
        means IN THIS SPECIFIC CONTEXT. Prefer the surrounding window text over generic knowledge. \
        Be concise (2-4 sentences). \
        Always reply in the same language as the selected text. \
        Do NOT give a dictionary definition — explain the contextual meaning.";

    crate::services::debug_log::log(
        "ai-pipeline",
        format!(
            "using prompt context: strategy={}, original_window_text_len={}, prompt_context_len={}, prompt_context={}",
            context_strategy,
            input.window_text.len(),
            prompt_context.len(),
            crate::services::debug_log::truncate_for_log(&prompt_context, 300),
        ),
    );

    let user_msg = if prompt_context.is_empty() {
        format!("Selected text: \"{}\"\n\nNo additional context is available.", input.selected_text)
    } else {
        format!(
            "Selected text: \"{}\"\n\nFull context from the window:\n{}",
            input.selected_text,
            prompt_context
        )
    };

    let explanation = client.text(TextRequest {
        model: settings.model.clone(),
        system: explain_system.into(),
        user: user_msg,
    }).await?.content;

    crate::services::debug_log::log(
        "ai-pipeline",
        format!(
            "pipeline finished: latency_ms={}",
            start.elapsed().as_millis(),
        ),
    );
    crate::services::debug_log::trace(
        "ai-pipeline",
        format!(
            "final explanation: text={}",
            crate::services::debug_log::truncate_for_log(&explanation, 480),
        ),
    );

    Ok(PipelineResult {
        explanation,
        latency_ms: start.elapsed().as_millis() as u64,
    })
}

fn build_prompt_context(selected_text: &str, window_text: &str) -> (String, &'static str) {
    if window_text.trim().is_empty() {
        return (String::new(), "empty");
    }

    let chars: Vec<char> = window_text.chars().collect();
    if chars.len() <= MAX_PROMPT_CONTEXT_CHARS {
        return (window_text.to_string(), "full-window");
    }

    let selected_text = selected_text.trim();
    if !selected_text.is_empty() {
        if let Some(byte_index) = window_text.find(selected_text) {
            let selected_char_len = selected_text.chars().count().max(1);
            let match_char_index = window_text[..byte_index].chars().count();
            let context_budget = MAX_PROMPT_CONTEXT_CHARS.saturating_sub(selected_char_len);
            let before_budget = context_budget / 2;
            let after_budget = context_budget - before_budget;
            let excerpt_start = match_char_index.saturating_sub(before_budget);
            let excerpt_end = (match_char_index + selected_char_len + after_budget).min(chars.len());

            let mut excerpt: String = chars[excerpt_start..excerpt_end].iter().collect();
            if excerpt_start > 0 {
                excerpt = format!("...[context omitted]\n{}", excerpt);
            }
            if excerpt_end < chars.len() {
                excerpt.push_str("\n...[context omitted]");
            }

            return (excerpt, "selected-window");
        }
    }

    let head: String = chars.iter().take(MAX_PROMPT_CONTEXT_CHARS).collect();
    (format!("{}\n...[context omitted]", head), "leading-window")
}
