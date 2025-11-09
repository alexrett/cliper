#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]
#![allow(unexpected_cfgs)]

#[cfg(target_os = "macos")]

use std::sync::Arc;

use parking_lot::Mutex;
use tauri::{GlobalShortcutManager, Manager, ActivationPolicy, SystemTray, SystemTrayEvent};

mod api;
mod clipboard;
mod crypto;
mod db;
mod state;

#[cfg(target_os = "macos")]
use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial, NSVisualEffectState};
#[cfg(target_os = "macos")]
use cocoa::base::id as cocoa_id;
#[cfg(target_os = "macos")]
use objc::{msg_send, sel, sel_impl};

use state::{AppState, Settings};

#[cfg(target_os = "macos")]
fn setup_vibrancy(win: &tauri::Window) {
    apply_vibrancy(
        win,
        NSVisualEffectMaterial::HudWindow,
        Some(NSVisualEffectState::Active),
        None,
    )
    .expect("vibrancy failed");
}

#[cfg(target_os = "macos")]
fn round_corners(win: &tauri::Window, radius: f64) {
    if let Ok(ns_win) = win.ns_window() {
        let ns_win: cocoa_id = ns_win as _;
        unsafe {
            let content_view: cocoa_id = msg_send![ns_win, contentView];
            let _: () = msg_send![content_view, setWantsLayer: true];
            let layer: cocoa_id = msg_send![content_view, layer];
            let _: () = msg_send![layer, setCornerRadius: radius];
            let _: () = msg_send![layer, setMasksToBounds: true];
        }
    }
}


fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let app_handle = app.handle();

            // Init DB
            let app_dir = app_handle.path_resolver().app_data_dir().expect("app data dir");
            let db = db::Database::new(app_dir).expect("db init");
            db.migrate().expect("db migrate");

            // Init crypto manager (lazy-unlock from Keychain on demand)
            let bundle_id = app.config().tauri.bundle.identifier.clone();
            let crypto = crypto::KeyManager::new(bundle_id);

            // Load settings from app data dir
            let settings_path = state::settings_path(app_handle.path_resolver().app_data_dir().expect("app data dir"));
            let settings = state::load_settings(&settings_path).unwrap_or(Settings { auto_lock_minutes: 5, hotkey: "CmdOrCtrl+Shift+Space".into() });

            let state = AppState {
                db: Arc::new(db),
                crypto: Arc::new(crypto),
                settings: Arc::new(Mutex::new(settings)),
            };

            app.manage(state.clone());

            // Apply vibrancy and basic window tweaks
            #[cfg(target_os = "macos")]
            if let Some(win) = app.get_window("main") {
                setup_vibrancy(&win);
                let _ = win.center();
                round_corners(&win, 14.0);
            }

            // Global hotkey to toggle/show window
            {
                let app_for_cb = app.handle();
                let mut gsm = app.handle().global_shortcut_manager();
                let _ = gsm.unregister_all();
                let accel = state.settings.lock().hotkey.clone();
                if let Err(e) = gsm.register(accel.as_str(), move || {
                    if let Some(win) = app_for_cb.get_window("main") {
                        let _ = win.show();
                        let _ = win.unminimize();
                        let _ = win.set_focus();
                    }
                }) {
                    eprintln!("failed to register global shortcut: {e}");
                }
            }

            // Start clipboard poller (macOS)
            #[cfg(target_os = "macos")]
            {
                let state_clone = state.clone();
                let app_for_poller = app.handle();
                std::thread::spawn(move || {
                    let _ = clipboard::poll_pasteboard_sync(app_for_poller, state_clone);
                });
            }

            // Hide Dock icon, keep menu bar (Accessory app)
            #[cfg(target_os = "macos")]
            app.set_activation_policy(ActivationPolicy::Accessory);

            Ok(())
        })
        // Add system tray (top bar) to toggle window
        .system_tray(SystemTray::new())
        .on_system_tray_event(|app, event| {
            if let SystemTrayEvent::LeftClick { .. } = event {
                if let Some(win) = app.get_window("main") {
                    let visible = win.is_visible().unwrap_or(false);
                    if visible { let _ = win.hide(); } else { let _ = win.show(); let _ = win.set_focus(); }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            api::search,
            api::list_recent,
            api::copy_item,
            api::pin_item,
            api::delete_item,
            api::reveal_in_finder,
            api::get_settings,
            api::set_hotkey,
            api::get_image_preview,
            api::reset_master_key,
            api::unlock,
            api::lock
        ])
        .on_window_event(|event| {
            match event.event() {
                // Hide instead of quit
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    let _ = event.window().hide();
                }
                // Hide when focus lost
                tauri::WindowEvent::Focused(false) => {
                    let _ = event.window().hide();
                }
                _ => {}
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
