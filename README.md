# Cliper (Rust + Tauri)

macOS‑приложение «история буфера обмена» на Rust + Tauri.

Возможности:
- История буфера: текст, изображения, файлы (пути), RTF.
- Шифрованное хранение (envelope encryption, AES‑256‑GCM с уникальным 96‑бит IV).
- Мастер‑ключ 256 бит хранится в macOS Keychain, не пишется на диск.
- Глобальный хоткей `Cmd+Shift+Space` открывает overlay‑окно с «liquid glass» вибрацией.
- Поиск, повторное копирование, закрепление, удаление, очистка.
- Пакетирование через `tauri build`, Hardened Runtime, entitlements, нотаpизация.

Стек:
- Backend: Rust 1.75+, crates: `tauri` (v1), `tokio`, `serde`, `serde_json`, `anyhow`, `thiserror`,
  `window_vibrancy`, `arboard`, `rusqlite`, `ring`, `security-framework`, `zeroize`, `sha2`.
- Clipboard: `arboard` + нативный мост к `NSPasteboard` для `public.file-url`, RTF.
- DB: `rusqlite` (встроенный SQLite), шифруется полезная нагрузка (envelope encryption).
- UI: Tauri WebView + React (Vite). Вибрация через `NSVisualEffectView` (`window_vibrancy`).

Структура:
- `src-tauri/` — Rust/Tauri backend, конфиги, сборка.
- `ui/` — React + Vite фронтенд.
- `scripts/` — вспомогательные скрипты.

## Быстрый старт (dev)

Требования: macOS 14+, Xcode tools, Rust 1.75+, Node 18+, Tauri CLI.

1) Установка зависимостей UI

```
npm --prefix ui install
```

2) Запуск UI в dev (опционально)

```
npm --prefix ui run dev
```

3) Запуск Tauri dev

```
cargo tauri dev
```

Приложение спросит доступ к Keychain при первом запуске (создаст мастер‑ключ).

## Сборка, подпись, упаковка

```
make mac
```

По умолчанию включён Hardened Runtime. Подготовьте Developer ID сертификаты и настройте подпись/нотаризацию (см. ниже).

### Подпись и нотаризация (кратко)

- Установите переменные окружения или задайте в `tauri.conf.json`:
  - `APPLE_ID`, `APPLE_PASSWORD` (или ключ API) — для нотаризации.
  - `APPLE_TEAM_ID`, `APPLE_SIGN_IDENTITY` — для подписи.
- Убедитесь, что `entitlements.plist` в комплекте, JIT выключен.
- Запускайте `cargo tauri build` — Tauri Bundler подпишет и отправит на нотаризацию при конфигурации.

## Модель данных (SQLite)

```
CREATE TABLE IF NOT EXISTS items (
  id INTEGER PRIMARY KEY,
  created_at INTEGER NOT NULL,
  kind TEXT NOT NULL,             -- "text" | "image" | "file"
  size INTEGER NOT NULL,
  sha256 BLOB NOT NULL,
  file_path TEXT,                 -- NULL для не-файлов
  is_pinned INTEGER NOT NULL DEFAULT 0,
  content_blob BLOB,              -- шифротекст (nonce||ciphertext)
  preview_blob BLOB,              -- шифротекст миниатюры (nonce||ciphertext)
  rtf_blob BLOB                   -- шифротекст RTF, если было (nonce||ciphertext)
);
CREATE INDEX IF NOT EXISTS idx_items_created ON items(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_items_kind ON items(kind);
```

Примечание: IV/nonce хранится префиксом перед шифротекстом в соответствующем поле (`nonce || ciphertext`).

## Управление ключом

- 256‑битный ключ создаётся при первом запуске.
- Хранится в Keychain под сервисом `<bundle_id>.masterkey`, аккаунт `default`.
- В памяти ключ обёрнут в `zeroize`, при блокировке/сворачивании нулируется.
- Автоблокировка по таймеру бездействия (настраивается в UI): приложение забывает ключ и запрашивает снова из Keychain.

## Глобальный хоткей

`Cmd+Shift+Space` — показывает окно, ставит фокус, всегда поверх.

## UI

- Прозрачное окно без декора, центр экрана, вибрация `HudWindow`.
- Поле поиска, фильтры (All | Text | Images | Files), список карточек.
- Навигация стрелками, Enter — вставить, Cmd+P — pin, Delete — удалить.

## Скрипты

- `make mac` — `npm run --prefix ui build` и `cargo tauri build`.
- `scripts/e2e.sh` — пример E2E сценария (см. ограничения в комментариях).

## Тесты

- Юнит‑тесты: `src-tauri/tests/crypto.rs` — шифрование/дешифрование, порча тега.
- Интеграция: `src-tauri/tests/db_integration.rs` — миграции, вставка, поиск, пин.

## Ограничения и заметки

- Для файлов хранится только путь/метаданные, содержимое не копируется.
- Поиск по содержимому выполняется в приложении после расшифровки (данные в БД зашифрованы).
- Миниатюры изображений генерируются лениво при показе.

