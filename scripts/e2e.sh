#!/usr/bin/env bash
set -euo pipefail

# Simple E2E smoke script for macOS.
# NOTE: Requires the app to be running (dev or built) so the clipboard poller is active.
# This script copies sample content into the system clipboard and then prompts you to
# trigger the global hotkey (Cmd+Shift+Space) to verify items appear in UI.

echo "[E2E] Copying sample text..."
printf "Hello from E2E $(date)" | pbcopy
sleep 1

if command -v osascript >/dev/null 2>&1; then
  echo "[E2E] Attempting to copy a Finder-selected file (if any) via AppleScript..."
  osascript <<'OSA'
  tell application "Finder"
    try
      set theItems to selection as alias list
      if (count of theItems) is greater than 0 then
        set thePath to POSIX path of (item 1 of theItems)
        do shell script "osascript -e 'set the clipboard to POSIX file \"" & thePath & "\"'"
      end if
    end try
  end tell
OSA
fi

echo "[E2E] Now press Cmd+Shift+Space to open the app and verify the new entries."

