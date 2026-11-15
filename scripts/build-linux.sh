#!/usr/bin/env bash
set -euo pipefail
# shellcheck source=scripts/common.sh
source "$(dirname "$0")/common.sh"
cd "$task_repo_root"
if [[ ${1:-} == --help ]]; then echo 'Usage: bash scripts/build-linux.sh'; exit 0; fi
if (( $# != 0 )); then echo 'Usage: bash scripts/build-linux.sh' >&2; exit 2; fi
[[ $(uname -s) == Linux ]] || { echo 'Build the Ubuntu installer on Linux.' >&2; exit 1; }
[[ $(dpkg --print-architecture) == amd64 ]] || { echo 'The Ubuntu installer targets amd64. Build it on an amd64 host.' >&2; exit 1; }
python3 scripts/check-repo.py
cargo build --locked --release --manifest-path linux/Cargo.toml
task_root="$task_repo_root/build/linux-amd64"
mkdir -p "$task_root"
task_stage=$(mktemp -d "$task_root/stage.XXXXXX")
trap 'rm -r -- "$task_stage"' EXIT
task_package="$task_stage/package"
mkdir -p "$task_package/DEBIAN" "$task_package/usr/bin" "$task_package/usr/share/applications" "$task_package/usr/share/icons/hicolor/scalable/apps" "$task_package/usr/share/gnome-shell/extensions/featherclip@aeen.github" "$task_package/usr/share/doc/feather-clip"
install -m755 linux/target/release/featherclip "$task_package/usr/bin/featherclip"
cp assets/featherclip.svg "$task_package/usr/share/icons/hicolor/scalable/apps/featherclip.svg"
cp gnome/extension.js gnome/metadata.json "$task_package/usr/share/gnome-shell/extensions/featherclip@aeen.github/"
cp LICENSE "$task_package/usr/share/doc/feather-clip/copyright"
cat > "$task_package/usr/share/applications/io.github.AK6.FeatherClip.desktop" <<'DESKTOP'
[Desktop Entry]
Type=Application
Name=FeatherClip
Comment=A tiny native clipboard history
Exec=featherclip
Icon=featherclip
Terminal=false
Categories=Utility;
StartupWMClass=io.github.AK6.FeatherClip
DESKTOP
cat > "$task_package/DEBIAN/control" <<CONTROL
Package: feather-clip
Version: $task_version
Section: utils
Priority: optional
Architecture: amd64
Maintainer: Aeen Karimian <aeenakrimian@gmail.com>
Depends: libgtk-4-1 (>= 4.12), libadwaita-1-0 (>= 1.4), libglib2.0-0 (>= 2.78), libc6 (>= 2.39), libgcc-s1, libxcb1, libx11-6
Homepage: https://github.com/A-K-6/featherclip
Description: Tiny native clipboard history for Ubuntu
 Native GTK4 and libadwaita popup for text, images and local file references.
 History stays in memory. Includes a GNOME Shell Wayland integration.
CONTROL
task_links=$(ldd "$task_package/usr/bin/featherclip")
if grep -Eiq 'webkit|javascriptcore|not found' <<< "$task_links"; then
  echo 'Missing system library or unexpected web runtime.' >&2; exit 1
fi
dpkg-deb --build --root-owner-group "$task_package" "$task_root/FeatherClip_${task_version}_amd64.deb"
