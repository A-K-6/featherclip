#!/usr/bin/env bash
set -euo pipefail
# shellcheck source=scripts/common.sh
source "$(dirname "$0")/common.sh"
cd "$task_repo_root"
usage() { echo 'Usage: bash scripts/build-macos.sh [arm64|x86_64] [--dmg]'; }
if [[ ${1:-} == --help ]]; then usage; exit 0; fi
if (( $# > 2 )) || [[ -n ${2:-} && $2 != --dmg ]]; then usage >&2; exit 2; fi
task_arch=${1:-$(uname -m)}
[[ $task_arch == arm64 || $task_arch == x86_64 ]] || { usage >&2; exit 2; }
[[ $(uname -s) == Darwin ]] || { echo 'Build the Mac app on macOS with Xcode.' >&2; exit 1; }
python3 scripts/check-repo.py
swift build --package-path macos -c release --arch "$task_arch"
task_bin=$(swift build --package-path macos -c release --arch "$task_arch" --show-bin-path)
task_root="$task_repo_root/build/macos-$task_arch"
mkdir -p "$task_root"
task_stage=$(mktemp -d "$task_root/stage.XXXXXX")
trap 'rm -r -- "$task_stage"' EXIT
task_app="$task_stage/FeatherClip.app"
mkdir -p "$task_app/Contents/MacOS" "$task_app/Contents/Resources"
cp "$task_bin/FeatherClip" "$task_app/Contents/MacOS/FeatherClip"
cp assets/icon.icns assets/sample.png LICENSE "$task_app/Contents/Resources/"
cat > "$task_app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>io.github.AK6.FeatherClip</string>
<key>CFBundleName</key><string>FeatherClip</string>
<key>CFBundleDisplayName</key><string>FeatherClip</string>
<key>CFBundleExecutable</key><string>FeatherClip</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>$task_version</string>
<key>CFBundleVersion</key><string>$task_version</string>
<key>CFBundleIconFile</key><string>icon.icns</string>
<key>LSMinimumSystemVersion</key><string>13.0</string>
<key>LSUIElement</key><true/>
<key>NSHighResolutionCapable</key><true/>
<key>NSHumanReadableCopyright</key><string>Copyright © 2026 Aeen Karimian. MIT licensed.</string>
</dict></plist>
PLIST
codesign --force --sign - --identifier io.github.AK6.FeatherClip "$task_app"
codesign --verify --deep --strict "$task_app"
lipo -archs "$task_app/Contents/MacOS/FeatherClip" | grep -x "$task_arch"
task_links=$(otool -L "$task_app/Contents/MacOS/FeatherClip")
if grep -Eq 'WebKit|JavaScriptCore' <<< "$task_links"; then
  echo 'Unexpected web runtime linked' >&2; exit 1
fi
# Replace only the generated build bundle, so old resources cannot leak into a new build.
if [[ -e "$task_root/FeatherClip.app" ]]; then mv "$task_root/FeatherClip.app" "$task_stage/previous.app"; fi
mv "$task_app" "$task_root/FeatherClip.app"
if [[ ${2:-} == --dmg ]]; then
  mkdir -p "$task_stage/disk"
  ditto "$task_root/FeatherClip.app" "$task_stage/disk/FeatherClip.app"
  ln -s /Applications "$task_stage/disk/Applications"
  hdiutil create -quiet -ov -format UDZO -volname FeatherClip -srcfolder "$task_stage/disk" "$task_root/FeatherClip_${task_version}_$task_arch.dmg"
fi
echo "Native app: $task_root/FeatherClip.app"
