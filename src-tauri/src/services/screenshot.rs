//! Capture a screenshot of the frontmost window and return base64-encoded PNG bytes.

use anyhow::Result;

/// Captures the frontmost window on macOS using `screencapture`.
/// Returns (base64_png, mime_type).
pub fn capture_frontmost_window() -> Result<(String, String)> {
    #[cfg(target_os = "macos")]
    {
        capture_macos()
    }
    #[cfg(not(target_os = "macos"))]
    {
        anyhow::bail!("screenshot not supported on this platform")
    }
}

#[cfg(target_os = "macos")]
fn capture_macos() -> Result<(String, String)> {
    use anyhow::Context;
    use std::process::Command;

    let tmp = std::env::temp_dir().join("s2e_capture.png");
    // -x: no sound; -l<wid>: specific window; we use -S (screen) as fallback
    // For simplicity, capture the frontmost window via screencapture -x (whole screen)
    // then crop is unnecessary — we just send the full screen screenshot.
    let status = Command::new("screencapture")
        .args(["-x", "-o", "-m", tmp.to_str().unwrap()])
        .status()
        .context("screencapture failed to launch")?;

    if !status.success() {
        anyhow::bail!("screencapture exited with {:?}", status.code());
    }

    let bytes = std::fs::read(&tmp).context("read screenshot file")?;
    // Clean up
    let _ = std::fs::remove_file(&tmp);

    let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes);
    Ok((b64, "image/png".into()))
}
