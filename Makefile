mac:
	@echo "Building UI and Tauri app for macOS..."
	npm --prefix ui ci || npm --prefix ui install
	npm --prefix ui run build
	cargo tauri build

dev:
	@echo "Starting Tauri dev (UI dev server recommended in separate shell)"
	cargo tauri dev

