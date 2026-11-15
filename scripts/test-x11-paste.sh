#!/usr/bin/env bash
set -euo pipefail
[[ ${CI:-} == true ]] || { echo 'Use only in isolated CI.' >&2; exit 1; }
unset WAYLAND_DISPLAY
export XDG_CURRENT_DESKTOP=Openbox
export FEATHERCLIP_ISOLATED_DISPLAY=1
task_logs=$(mktemp -d)
openbox --sm-disable > "$task_logs/openbox.log" 2>&1 &
task_wm=$!
python3 scripts/paste-fixture.py > "$task_logs/fixture.log" 2>&1 &
task_fixture=$!
trap 'kill "$task_fixture" "$task_wm" 2>/dev/null || true; rm -rf "$task_logs"' EXIT
gdbus wait --session --timeout 20 io.github.AK6.FeatherClip.PasteFixture
cargo test --locked --manifest-path src-tauri/Cargo.toml isolated_x11_paste_roundtrip -- --ignored --test-threads=1
