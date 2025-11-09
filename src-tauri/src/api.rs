use crate::{clipboard, db};
use anyhow::Result;
use tauri::{Manager, State, GlobalShortcutManager};
use image::GenericImageView;
use base64::Engine;
use std::path::Path;

#[derive(Debug, Clone, serde::Serialize)]
pub struct UiItemDto {
  pub id: i64,
  pub created_at: i64,
  pub kind: String,
  pub size: i64,
  pub sha256_hex: String,
  pub file_path: Option<String>,
  pub is_pinned: bool,
  pub preview: Option<String>,
}

use crate::state::AppState;

#[tauri::command]
pub fn unlock(state: State<AppState>) -> Result<(), String> {
    state.crypto.unlock().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn lock(state: State<AppState>) -> Result<(), String> {
    state.crypto.lock();
    Ok(())
}

#[tauri::command]
pub fn list_recent(state: State<AppState>, limit: u32) -> Result<Vec<UiItemDto>, String> {
    let items = state.db.list_recent(limit).map_err(|e| e.to_string())?;
    let mut out = Vec::with_capacity(items.len());
    for it in items {
        let mut preview = None;
        let mut size = it.size;
        if it.kind == "text" {
            if let Ok((_, Some(ct), _, _, _)) = state.db.get_item_raw(it.id) {
                if let Ok(pt) = state.crypto.decrypt(&ct) {
                    let s = String::from_utf8_lossy(&pt);
                    let p: String = s.chars().take(100).collect();
                    preview = Some(p);
                }
            }
        } else if it.kind == "file" {
            if let Some(ref fp) = it.file_path {
                if let Some(name) = Path::new(fp).file_name().and_then(|n| n.to_str()) {
                    preview = Some(name.to_string());
                }
                if size <= 0 {
                    if let Ok(meta) = std::fs::metadata(fp) { size = meta.len() as i64; }
                }
            }
        }
        out.push(UiItemDto {
            id: it.id,
            created_at: it.created_at,
            kind: it.kind,
            size,
            sha256_hex: it.sha256_hex,
            file_path: it.file_path,
            is_pinned: it.is_pinned,
            preview,
        });
    }
    Ok(out)
}

#[tauri::command]
pub fn search(state: State<AppState>, query: String, kind: Option<String>, limit: u32) -> Result<Vec<UiItemDto>, String> {
    // Since payloads are encrypted, we retrieve recent items and filter after (if unlocked).
    let mut items = state.db.list_recent(200).map_err(|e| e.to_string())?;
    if let Some(k) = kind {
        items.retain(|i| i.kind == k);
    }
    if query.trim().is_empty() {
        items.truncate(limit as usize);
        // hydrate previews
        let mapped = items.into_iter().map(|it| {
            let mut preview = None;
            let mut size = it.size;
            if it.kind == "text" {
                if let Ok((_, Some(ct), _, _, _)) = state.db.get_item_raw(it.id) {
                    if let Ok(pt) = state.crypto.decrypt(&ct) {
                        let s = String::from_utf8_lossy(&pt);
                        preview = Some(s.chars().take(100).collect());
                    }
                }
            } else if it.kind == "file" {
                if let Some(ref fp) = it.file_path {
                    if let Some(name) = Path::new(fp).file_name().and_then(|n| n.to_str()) {
                        preview = Some(name.to_string());
                    }
                    if size <= 0 { if let Ok(meta) = std::fs::metadata(fp) { size = meta.len() as i64; } }
                }
            }
            UiItemDto {
                id: it.id,
                created_at: it.created_at,
                kind: it.kind,
                size,
                sha256_hex: it.sha256_hex,
                file_path: it.file_path,
                is_pinned: it.is_pinned,
                preview,
            }
        }).collect();
        return Ok(mapped);
    }
    let q = query.to_lowercase();
    let mut out = Vec::new();
    for it in items {
        if out.len() >= limit as usize { break; }
        match it.kind.as_str() {
            "text" => {
                if let Ok((_, Some(ct), _, _, _)) = state.db.get_item_raw(it.id) {
                    if let Ok(pt) = state.crypto.decrypt(&ct) {
                        let s_lower = String::from_utf8_lossy(&pt).to_lowercase();
                        if s_lower.contains(&q) {
                            let preview = Some(String::from_utf8_lossy(&pt).chars().take(100).collect());
                            out.push(UiItemDto { id: it.id, created_at: it.created_at, kind: it.kind, size: it.size, sha256_hex: it.sha256_hex, file_path: it.file_path, is_pinned: it.is_pinned, preview });
                        }
                    }
                }
            }
            "file" => {
                if let Some(fp) = &it.file_path {
                    if fp.to_lowercase().contains(&q) {
                        let name = Path::new(fp).file_name().and_then(|n| n.to_str()).map(|s| s.to_string());
                        out.push(UiItemDto { id: it.id, created_at: it.created_at, kind: it.kind, size: it.size, sha256_hex: it.sha256_hex, file_path: it.file_path, is_pinned: it.is_pinned, preview: name });
                    }
                }
            }
            "image" => {
                // For images, no text — include on empty query or by kind only; if query present, skip.
            }
            _ => {}
        }
    }
    Ok(out)
}

#[tauri::command]
pub fn copy_item(state: State<AppState>, id: i64) -> Result<(), String> {
    clipboard::copy_back(&state.db, &state.crypto, id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn pin_item(window: tauri::Window, state: State<AppState>, id: i64, pin: bool) -> Result<(), String> {
    state.db.pin_item(id, pin).map_err(|e| e.to_string())?;
    let _ = window.app_handle().emit_all("items_updated", ());
    Ok(())
}

#[tauri::command]
pub fn delete_item(window: tauri::Window, state: State<AppState>, id: i64) -> Result<(), String> {
    state.db.delete_item(id).map_err(|e| e.to_string())?;
    let _ = window.app_handle().emit_all("items_updated", ());
    Ok(())
}

#[tauri::command]
pub fn get_settings(state: State<AppState>) -> Result<crate::state::Settings, String> {
    Ok(state.settings.lock().clone())
}

#[tauri::command]
pub fn set_hotkey(window: tauri::Window, state: State<AppState>, hotkey: String) -> Result<(), String> {
    // Re-register global shortcut
    let app = window.app_handle();
    let app_for_cb = app.clone();
    let mut gsm = app.global_shortcut_manager();
    gsm.unregister_all().map_err(|e| e.to_string())?;
    gsm.register(hotkey.as_str(), move || {
        if let Some(win) = app_for_cb.get_window("main") {
            let _ = win.show();
            let _ = win.unminimize();
            let _ = win.set_focus();
        }
    }).map_err(|e| e.to_string())?;

    // Update and persist settings
    {
        let mut s = state.settings.lock();
        s.hotkey = hotkey.clone();
        let app_dir = app.path_resolver().app_data_dir().ok_or("no app dir")?;
        let path = crate::state::settings_path(app_dir);
        crate::state::save_settings(&path, &s);
    }
    Ok(())
}

#[tauri::command]
pub fn get_image_preview(state: State<AppState>, id: i64, max: Option<u32>) -> Result<String, String> {
    let (kind, content_blob, _, _, _) = state.db.get_item_raw(id).map_err(|e| e.to_string())?;
    if kind != "image" { return Err("not an image".into()); }
    let ct = content_blob.ok_or("no content")?;
    let pt = state.crypto.decrypt(&ct).map_err(|e| e.to_string())?; // PNG
    let img = image::load_from_memory(&pt).map_err(|e| e.to_string())?;
    let max_side = max.unwrap_or(128);
    let (w, h) = img.dimensions();
    let scale = (max_side as f32 / w.max(h) as f32).min(1.0);
    let new_w = (w as f32 * scale).round() as u32;
    let new_h = (h as f32 * scale).round() as u32;
    let resized = if scale < 1.0 { img.thumbnail(new_w, new_h) } else { img };
    let mut out = Vec::new();
    resized.write_to(&mut std::io::Cursor::new(&mut out), image::ImageOutputFormat::Png).map_err(|e| e.to_string())?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(out);
    Ok(format!("data:image/png;base64,{}", b64))
}

#[tauri::command]
pub fn reset_master_key(state: State<AppState>) -> Result<(), String> {
    state.crypto.reset_master_key().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn reveal_in_finder(path: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("-R")
            .arg(path)
            .status()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    #[allow(unreachable_code)]
    Err("not supported".into())
}
