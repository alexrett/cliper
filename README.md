# Cliper — macOS Clipboard Manager (Rust + Tauri)

Cliper is a privacy‑focused clipboard history app for macOS built with Rust and Tauri.
It captures text, images, RTF, and files (paths only), stores encrypted history, and shows a quick overlay window with search and keyboard navigation. The master key lives in macOS Keychain; sensitive fields never touch disk unencrypted.

## Highlights

- Status‑bar app (no Dock icon), overlay UI with vibrancy + native rounded corners
- Global hotkey to toggle overlay (default: `Cmd+Shift+Space`, configurable)
- Clipboard types: text (UTF‑8), images (PNG), RTF, file URLs (multiple) — file contents are never copied
- Encrypted SQLite storage (envelope) with AES‑256‑GCM and a unique 96‑bit nonce per item
- Master key (256‑bit) is created on first run and stored in Keychain; can be reset from Settings
- Search by substring/type, re‑copy back to system clipboard, pin, delete, clear
- Lazy image thumbnails; file cards show filename as title and full path as subtitle

## Stack

- Backend: Rust 1.75+, Tauri v1, Tokio, Serde, Anyhow, Thiserror
- macOS UI: `window_vibrancy` (NSVisualEffectView HUD material), native NSWindow corner radius
- Clipboard: `arboard` + direct `NSPasteboard` bridge (file URLs, RTF)
- Crypto: `ring` (AES‑256‑GCM), `zeroize`
- Keychain: `security-framework`
- DB: `rusqlite` (SQLite bundled), schema below
- UI: React + Vite (minimal, single window)

## App Icon

Place your app icon in WebP format at the repo root:

```
cliper_icon.webp
```

The build script will generate multiple PNG sizes and a macOS `.icns` at `src-tauri/icons/icon.icns` automatically. If no WebP is found, a placeholder icon is generated.

## Quick Start (Development)

Requirements: macOS 13+, Xcode Tools, Rust 1.75+, Node 18+, Tauri CLI.

```bash
# install UI deps once
npm --prefix ui install

# run dev (from repo root)
cargo tauri dev
```

The first time Cliper accesses Keychain, macOS will prompt for your login password. Approve (ideally “Always Allow”). If you ever need to reset the master key, use Settings → Reset Master Key (existing items become unreadable).

## Build (Release)

```bash
make mac
```

Artifacts:
- App: `src-tauri/target/release/bundle/macos/Cliper.app`
- DMG: `src-tauri/target/release/bundle/dmg/Cliper_0.1.0_aarch64.dmg`

### Code Signing & Notarization (optional)

Configure your Apple credentials (env vars or CI secrets) and re‑build:
- `APPLE_TEAM_ID`, `APPLE_SIGN_IDENTITY` (Developer ID Application)
- `APPLE_ID`, `APPLE_PASSWORD` (or App Store Connect API key)

Tauri’s bundler will sign and can submit for notarization if configured.

## Data Model (SQLite)

```
CREATE TABLE IF NOT EXISTS items (
  id INTEGER PRIMARY KEY,
  created_at INTEGER NOT NULL,
  kind TEXT NOT NULL,             -- "text" | "image" | "file"
  size INTEGER NOT NULL,
  sha256 BLOB NOT NULL,
  file_path TEXT,                 -- NULL for non-files
  is_pinned INTEGER NOT NULL DEFAULT 0,
  content_blob BLOB,              -- ciphertext (nonce || ciphertext)
  preview_blob BLOB,              -- reserved
  rtf_blob BLOB                   -- ciphertext (nonce || ciphertext)
);
CREATE INDEX IF NOT EXISTS idx_items_created ON items(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_items_kind ON items(kind);
```

Encrypted fields are stored as `nonce || ciphertext` (AES‑GCM, 96‑bit IV).

## Usage

- Status bar: Cliper lives in the menu bar (no Dock icon). Click to show/hide.
- Global hotkey: default `Cmd+Shift+Space` toggles the overlay (Settings → change).
- Overlay window: vibrancy + native rounded corners (HUD material); centered.
- Search at the top; list below with keyboard navigation:
  - Up/Down to select, Enter to copy back
  - `Cmd+P` to pin/unpin, `Delete` to delete, `Esc` to hide
- Filters: All | Text | Images | Files
- File cards: title = filename; subtitle = full path

## Settings

- Global Hotkey — update and apply immediately (persists to `settings.json` in the app data dir)
- Reset Master Key — regenerates 256‑bit key in Keychain; existing items become unreadable

## Permissions

- Keychain: macOS will ask for permission to read the master key (use your macOS login password)
- Accessibility: may be required by macOS to register global shortcuts; enable Cliper under System Settings → Privacy & Security → Accessibility

## Architecture

Backend (`src-tauri/`):
- `main.rs` — Tauri setup, vibrancy, native corner radius, status bar + global hotkeys
- `clipboard/` — NSPasteboard polling (changeCount every 250ms), type normalization, dedup by SHA‑256
- `crypto/` — envelope encryption, master key management, Keychain integration
- `db/` — migrations, CRUD, search
- `api.rs` — Tauri commands: `search`, `list_recent`, `copy_item`, `pin_item`, `delete_item`, `reveal_in_finder`, `get_settings`, `set_hotkey`, `reset_master_key`, `unlock`, `lock`

Frontend (`ui/`):
- React + Vite minimal overlay UI with search, list, filters, thumbnails, hotkeys, and Settings modal

## Troubleshooting

- Keychain prompt keeps appearing / wrong password
  - The prompt asks for your macOS login password to unlock the Keychain item. Approve “Always Allow” to avoid repeated prompts.
  - You can also remove the item in Keychain Access (service `com.example.cliper.masterkey`, account `default`) and restart.
  - Or use Settings → Reset Master Key and approve the new entry.

- Global hotkey doesn’t toggle the window
  - Check Accessibility permissions for Cliper under System Settings → Privacy & Security → Accessibility.
  - Adjust the hotkey via Settings if the default conflicts with another app.

- File shows as `/.file/id=...`
  - Fixed: Cliper reads file URLs via `NSPasteboard` `readObjectsForClasses:` and resolves to real POSIX paths.

## Development Notes

- Clipboard poller is a background thread on macOS to avoid `!Send` issues
- Items are deduplicated by `(kind, sha256, file_path)`
- Thumbnails are generated on demand when the UI asks for an image preview
