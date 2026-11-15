#!/usr/bin/env bash
# Run with dbus-run-session on an isolated CI desktop, never in a user's GNOME session.
set -euo pipefail
if [[ ${CI:-} != true ]]; then
  echo 'This headless desktop check is restricted to CI.' >&2
  exit 1
fi

task_runtime=$(mktemp -d)
export XDG_RUNTIME_DIR="$task_runtime"
export XDG_SESSION_TYPE=wayland
export XDG_CURRENT_DESKTOP=GNOME
export WAYLAND_DISPLAY=wayland-0
export LIBGL_ALWAYS_SOFTWARE=1
export MUTTER_DEBUG_DISABLE_HW_CURSORS=1
export FEATHERCLIP_ISOLATED_DISPLAY=1

task_extension="$HOME/.local/share/gnome-shell/extensions/featherclip@aeen.github"
mkdir -p "$task_extension"
cp gnome/extension.js gnome/metadata.json "$task_extension/"
gsettings set org.gnome.shell disable-user-extensions false
gsettings set org.gnome.shell enabled-extensions "['featherclip@aeen.github']"

gnome-shell --wayland --headless --virtual-monitor 1280x720 > "$task_runtime/shell.log" 2>&1 &
task_shell_pid=$!
cleanup() {
  kill "$task_shell_pid" 2>/dev/null || true
  if [[ ${task_failed:-0} == 1 ]]; then tail -80 "$task_runtime/shell.log"; fi
  rm -rf "$task_runtime"
}
trap cleanup EXIT
task_failed=1
gdbus wait --session --timeout 45 io.github.AK6.FeatherClip.Gnome
cargo test --locked --manifest-path src-tauri/Cargo.toml isolated_gnome_bridge_roundtrip -- --ignored --test-threads=1
task_failed=0
