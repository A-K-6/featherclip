# FeatherClip

A tiny **native** clipboard popup for macOS and Ubuntu. Copy text, pictures or files, open your history, and select a clip to paste into the previous app.

- **macOS:** Swift + AppKit. Real Apple glass on macOS 26+, system translucent material on macOS 13–15. Native search, menus, buttons and keyboard navigation.
- **Ubuntu:** Rust + GTK4 + libadwaita. Native GNOME controls and automatic light/dark appearance.
- No Tauri, webview, Electron, Node runtime, accounts, network requests or cloud sync. Appearance follows each desktop; clipboard history is local.

[Website & blog](https://a-k-6.github.io/featherclip/) · [Download](https://github.com/A-K-6/featherclip/releases/latest) · [Report a bug](https://github.com/A-K-6/featherclip/issues)

## Install

**Mac · macOS 13+:** download the **arm64** `.dmg` for Apple Silicon (M-series), or the separate **x86_64** `.dmg` for Intel. Open it and drag FeatherClip into Applications. Quit and replace any older version first. The clipboard icon appears in the menu bar.

Community builds are ad-hoc signed and aren't Apple-notarized. If macOS blocks the first launch, follow [Apple's Open Anyway instructions](https://support.apple.com/en-us/102445). The app does not disable system security settings.

**Ubuntu · 24.04+ · Intel/AMD 64-bit:** download the `.deb` and install:

```sh
sudo apt install ./FeatherClip_2.0.1_amd64.deb
```

Open FeatherClip from Applications. It uses system GTK4 and libadwaita libraries.

**Default GNOME/Wayland:** the installer includes the small FeatherClip GNOME Bridge. Choose **••• → Enable GNOME integration**. A first installation may require logging out and back in before enabling it. The bridge provides clipboard access and previous-window paste through local session D-Bus, stores no history and makes no network connections. It declares GNOME 45–51 compatibility; CI exercises GNOME 46. X11 needs no extension.

## Use

Copy something, focus the app you want to paste into, then open FeatherClip:

- **⌘⇧V** on Mac; **Ctrl+Alt+V** on Linux/X11 when available.
- On Wayland, create a custom keyboard shortcut for `featherclip --toggle`.
- Search, use arrow keys, then **Enter**, or click a clip to paste. **Shift+Enter** copies without pasting; Mac also supports **Shift-click**.
- On Mac, typing starts in search immediately. **Tab / Shift+Tab** moves between clips and controls; **Space / Enter** opens **•••** when focused.
- **Escape**, the close button or clicking outside dismisses the popup. Clipboard capture continues in the background.
- **Pause**, **Clear**, **Start at login**, and **Quit** are native controls. Login startup is off by default.

On Mac, choose **••• → Enable direct paste**, then enable FeatherClip in **System Settings → Privacy & Security → Accessibility** (called **Device Control and Data Access** on newer macOS). Restart FeatherClip after granting access. Capture and copy work without this permission. Replacing an ad-hoc signed build can require refreshing its entry.

Direct paste supports macOS, X11 and GNOME Wayland. Other Wayland desktops currently offer manual paste. If the previous app closes or cannot regain focus, use your normal Paste command. Terminal apps on Linux use Ctrl+Shift+V. Pictures and files require a destination that accepts them.

## Small by design

History stays **only in memory**, limited to **30 unique clips** and **24 MiB** of retained payloads. Text is limited to 100 KiB, PNG/TIFF images to 8 MiB, and file references to 100 local files / 100 KiB. Files remain in their original locations; selecting them copies rather than cuts. Moving/deleting a source invalidates its reference. X11 images use PNG. App-specific formats and rich-text formatting aren't retained.

The worker checks every 750 ms, skips unchanged clipboard revisions where supported, and stops reading while paused. There is one small native popup; no embedded browser. Thumbnails are bounded. History begins with new copies after launch; existing clipboard contents and paused copies aren't backfilled. Clear removes history without changing the system clipboard. Quit discards history.

Clipboard text can contain secrets. Pause before copying sensitive information. Known concealed markers are skipped on Mac and GNOME, but not every clipboard backend exposes them. OS swap/crash handling and other clipboard managers are outside the app's control.

## Build

### macOS

Install Xcode with the macOS 26 SDK or newer (for the glass API), plus Python 3 for the build metadata check. No external Swift packages are required.

```sh
swift test --package-path macos
bash scripts/build-macos.sh arm64 --dmg
bash scripts/build-macos.sh x86_64 --dmg
build/macos-arm64/FeatherClip.app/Contents/MacOS/FeatherClip --smoke-test
```

Deployment targets macOS 13; availability checks select the older system material when glass isn't available. Each installer contains one architecture.

### Ubuntu

Install Rust 1.90+ and native dependencies:

```sh
sudo apt install build-essential pkg-config python3 libgtk-4-dev libadwaita-1-dev
cargo test --locked --manifest-path linux/Cargo.toml
cargo clippy --locked --manifest-path linux/Cargo.toml --all-targets -- -D warnings
bash scripts/build-linux.sh
```

`--demo` uses only synthetic clips and a private in-memory clipboard. `--smoke-test` exercises the actual native window, Unicode search, copy, pause, clear and reopening, then exits. Neither mode accesses your real clipboard or changes login items.

CI builds ARM64 and Intel Mac installers and an Ubuntu `.deb`. It also checks actual text/image/file clipboard round trips, native UI smoke tests and previous-window paste under isolated X11 and GNOME Wayland desktops. GNOME bridge source is in [`gnome/`](gnome); it is a native desktop extension, not a web UI.

## Uninstall

Turn Start at login off, then Quit. On Mac, move FeatherClip from Applications to Trash. On Ubuntu:

```sh
gnome-extensions disable featherclip@aeen.github
sudo apt remove feather-clip
```

There is no saved clipboard history. The optional Linux autostart file is `~/.config/autostart/featherclip.desktop`.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for the source layout, checks and release process, and [CHANGELOG.md](CHANGELOG.md) for release history. Small improvements are welcome. Include OS and desktop/session details with bug reports; omit private clipboard contents. Preserve the simple UI, bounded memory and local-only defaults.

[MIT licensed](LICENSE). Aeen Karimian · `aeenakrimian@gmail.com`.
