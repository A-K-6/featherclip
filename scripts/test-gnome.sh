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
export GDK_BACKEND=wayland
export LIBGL_ALWAYS_SOFTWARE=1
export MUTTER_DEBUG_DISABLE_HW_CURSORS=1
export FEATHERCLIP_ISOLATED_DISPLAY=1

task_extension="$HOME/.local/share/gnome-shell/extensions/featherclip@aeen.github"
mkdir -p "$task_extension"
cp gnome/extension.js gnome/metadata.json "$task_extension/"
gsettings set org.gnome.shell disable-user-extensions false
gsettings set org.gnome.shell enabled-extensions "['featherclip@aeen.github']"
gsettings set org.gnome.shell welcome-dialog-last-shown-version '999.0'

gnome-shell --wayland --headless --unsafe-mode --virtual-monitor 1280x720 > "$task_runtime/shell.log" 2>&1 &
task_shell_pid=$!
cleanup() {
  if [[ -n ${task_fixture_pid:-} ]]; then kill "$task_fixture_pid" 2>/dev/null || true; fi
  kill "$task_shell_pid" 2>/dev/null || true
  if [[ ${task_failed:-0} == 1 ]]; then tail -80 "$task_runtime/shell.log"; cat "$task_runtime/fixture.log" 2>/dev/null || true; fi
  rm -rf "$task_runtime"
}
trap cleanup EXIT
task_failed=1
gdbus wait --session --timeout 45 io.github.AK6.FeatherClip.Gnome
# The extension can export D-Bus before Shell finishes starting its desktop.
task_ready=''
for task_attempt in {1..100}; do
  task_ready=$(gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell --method org.gnome.Shell.Eval 'Main.layoutManager._startingUp === false')
  if [[ $task_ready == "(true, 'true')" ]]; then break; fi
  sleep 0.1
done
if [[ $task_ready != "(true, 'true')" ]]; then echo 'GNOME desktop did not finish starting' >&2; exit 1; fi
# The headless session starts in Overview; mimic returning to a normal desktop.
# Unsafe Eval exists only inside this disposable test compositor.
gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell --method org.gnome.Shell.Eval 'global._clipTestKeyboard = imports.gi.Clutter.get_default_backend().get_default_seat().create_virtual_device(imports.gi.Clutter.InputDeviceType.KEYBOARD_DEVICE); Main.overview.hide();'
python3 scripts/paste-fixture.py > "$task_runtime/fixture.log" 2>&1 &
task_fixture_pid=$!
gdbus wait --session --timeout 20 io.github.AK6.FeatherClip.PasteFixture
cargo test --locked --manifest-path src-tauri/Cargo.toml isolated_gnome -- --ignored --test-threads=1
task_failed=0
