#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
task_arch=${1:-$(uname -m)}
[[ $task_arch == arm64 || $task_arch == x86_64 ]] || { echo 'Use arm64 or x86_64' >&2; exit 1; }
swift build --package-path macos -c release --arch "$task_arch"
task_bin=$(swift build --package-path macos -c release --arch "$task_arch" --show-bin-path)
task_root="build/macos-$task_arch"
task_app="$task_root/FeatherClip.app"
mkdir -p "$task_app/Contents/MacOS" "$task_app/Contents/Resources"
cp "$task_bin/FeatherClip" "$task_app/Contents/MacOS/FeatherClip"
cp assets/icon.icns assets/sample.png "$task_app/Contents/Resources/"
cat > "$task_app/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>io.github.AK6.FeatherClip</string>
<key>CFBundleName</key><string>FeatherClip</string>
<key>CFBundleDisplayName</key><string>FeatherClip</string>
<key>CFBundleExecutable</key><string>FeatherClip</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>2.0.0</string>
<key>CFBundleVersion</key><string>2.0.0</string>
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
if otool -L "$task_app/Contents/MacOS/FeatherClip" | grep -E 'WebKit|JavaScriptCore'; then
  echo 'Unexpected web runtime linked' >&2; exit 1
fi
if [[ ${2:-} == --dmg ]]; then
  mkdir -p "$task_root/disk"
  ditto "$task_app" "$task_root/disk/FeatherClip.app"
  [[ -L "$task_root/disk/Applications" ]] || ln -s /Applications "$task_root/disk/Applications"
  hdiutil create -quiet -ov -format UDZO -volname FeatherClip -srcfolder "$task_root/disk" "$task_root/FeatherClip_2.0.0_$task_arch.dmg"
fi
echo "Native app: $task_app"
