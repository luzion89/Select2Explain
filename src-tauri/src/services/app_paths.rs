use std::fs;
use std::path::PathBuf;

use anyhow::{anyhow, Result};

const APP_DIR_NAME: &str = "Select2Explain";

pub fn app_storage_dir() -> Result<PathBuf> {
    let base_dir = dirs::data_local_dir()
        .or_else(dirs::data_dir)
        .or_else(|| std::env::current_dir().ok())
        .ok_or_else(|| anyhow!("failed to resolve application data directory"))?;

    let dir = base_dir.join(APP_DIR_NAME);
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn settings_path() -> Result<PathBuf> {
    Ok(app_storage_dir()?.join("settings.json"))
}

pub fn debug_log_path() -> Result<PathBuf> {
    Ok(app_storage_dir()?.join("runtime.log"))
}

pub fn interaction_log_path() -> Result<PathBuf> {
    Ok(app_storage_dir()?.join("interaction.log"))
}

pub fn http_log_path() -> Result<PathBuf> {
    Ok(app_storage_dir()?.join("http.log"))
}