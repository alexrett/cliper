use std::sync::Arc;

use parking_lot::Mutex;
use std::fs;
use std::path::PathBuf;

use crate::{crypto::KeyManager, db::Database};

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Settings {
    pub auto_lock_minutes: u64,
    pub hotkey: String,
}

pub fn settings_path(app_dir: PathBuf) -> PathBuf { app_dir.join("settings.json") }

pub fn load_settings(path: &PathBuf) -> Option<Settings> {
    if let Ok(data) = fs::read(path) {
        serde_json::from_slice(&data).ok()
    } else { None }
}

pub fn save_settings(path: &PathBuf, s: &Settings) {
    if let Some(dir) = path.parent() { let _ = fs::create_dir_all(dir); }
    if let Ok(bytes) = serde_json::to_vec_pretty(s) { let _ = fs::write(path, bytes); }
}

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Database>,
    pub crypto: Arc<KeyManager>,
    pub settings: Arc<Mutex<Settings>>,
}
