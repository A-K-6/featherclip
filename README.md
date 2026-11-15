# FeatherClip

A little room for your clipboard. A small, open-source clipboard history for **macOS and Ubuntu**, built with **Tauri 2, Rust, and plain TypeScript**.

[Download the latest release](https://github.com/A-K-6/featherclip/releases/latest) · [Report a bug](https://github.com/A-K-6/featherclip/issues)

## Install

### macOS · Apple Silicon and Intel · macOS 12+

1. Download the `.dmg` from [Releases](https://github.com/A-K-6/featherclip/releases/latest).
2. Open it and drag **FeatherClip** into **Applications**.
3. Open FeatherClip. Its clipboard icon lives in your menu bar.

This community release is **ad-hoc signed, not Apple-notarized**. On first launch, macOS may block it. After trying to open it, go to **System Settings → Privacy & Security → Open Anyway**. See [Apple's instructions](https://support.apple.com/en-us/102445). No security settings are disabled by the app. If macOS requests clipboard access, allow it for history capture.

### Ubuntu · 24.04+ · 64-bit Intel/AMD

Download the `.deb`, open a terminal in the download folder, and install it:

```sh
sudo apt install ./FeatherClip_1.0.0_amd64.deb
```

Open **FeatherClip** from your applications menu. `apt` installs the required system libraries. The package uses your system WebKit instead of bundling a browser.

**Default Ubuntu/GNOME with Wayland:** the package includes the small **FeatherClip GNOME Bridge** extension. Click **Enable GNOME clipboard support** inside FeatherClip. On a fresh install, GNOME may require **one logout/login** before discovering the extension; then click Enable again. It reads/writes the local clipboard only when the running app requests it. It stores no history and makes no network connections. The bridge declares compatibility with GNOME 45–51; no extension is needed in an X11 session.

Other Linux desktops are best effort: X11 and Wayland compositors supporting the data-control protocol use the native clipboard library. GNOME Wayland requires the bundled bridge; an XWayland-only workaround is not used. The `.deb` is the supported Linux installer for this release.

## Use

- Copy text as usual. Open FeatherClip from the tray/menu bar or your applications launcher.
- Search, then click a clip to copy it. Switch back and paste with **⌘V** / **Ctrl+V**.
- **⌘⇧V** opens history on macOS; **Ctrl+Alt+V** on Linux/X11 when available. In Wayland, add a custom desktop keyboard shortcut for the command `featherclip`; opening it again focuses the running instance. Escape closes the history window.
- **Pause** and **Clear** sit below the clip list. **Start at login** and **Quit** are under the **•••** menu. Startup is off by default.
- Closing the window keeps the tray and clipboard worker running. **Quit** ends the app and discards its history. Reopen from your launcher if your desktop hides tray icons.

## Small by design

- **30 unique text clips**, newest first; up to **100 KiB of UTF-8 per clip**.
- **Memory-only history**: no history database, cloud sync, accounts, analytics, or clipboard logs. History starts with new copies after launch. Paused copies aren't backfilled on resume.
- **No automatic paste** or keyboard monitoring. No Accessibility permission needed.
- Images, files, and rich formatting are outside this first release. Text is pasted exactly, including Unicode and line breaks.
- The history window is created on demand and destroyed when closed. The background worker checks every 750 ms and skips reads while paused.
- Plain TypeScript and CSS, with the system webview. No React, Electron, bundled Chromium, or runtime web requests. "Lightweight" describes the design; system-webview memory varies by platform.

**Clipboard privacy:** ordinary copied text can contain secrets. Pause before copying sensitive text and clear history when appropriate. Known concealed clipboard markers are skipped on macOS and by the GNOME bridge; markers aren't available consistently across every backend. Clear removes FeatherClip's history; it leaves the current system clipboard intact. OS swap/crash handling and third-party clipboard managers are outside the app's control.

## Build from source

Install Node.js 22+, Rust 1.90+ (current stable recommended), and the [Tauri platform prerequisites](https://v2.tauri.app/start/prerequisites/). On Ubuntu:

```sh
sudo apt install build-essential curl file libwebkit2gtk-4.1-dev \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

```sh
git clone https://github.com/A-K-6/featherclip.git
cd featherclip
npm ci
npm run tauri -- dev
```

Validation and packaging:

```sh
npm run build
npm test
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npm run tauri -- build --bundles deb       # Ubuntu
npm run tauri -- build --bundles app,dmg   # macOS host architecture
```

For a universal macOS release, install both Rust targets and build:

```sh
rustup target add aarch64-apple-darwin x86_64-apple-darwin
npm run tauri -- build --target universal-apple-darwin --bundles app,dmg
```

`--demo` runs with synthetic clips and an in-memory clipboard. `--smoke-test` checks the real webview, IPC, Unicode search, copy selection, pause, and clear against that synthetic clipboard, then exits. Neither mode reads your real clipboard or enables launch at login. CI builds and exercises the app separately on macOS and Ubuntu.

GNOME source lives in [`gnome/`](gnome). It is packaged system-wide by the `.deb`; it does not need to be downloaded separately. Clipboard access is through local session D-Bus, never a listening TCP port. Extensions run inside GNOME Shell, so keep changes there small and review them carefully.

## Uninstall

Turn **Start at login** off and quit first. On macOS, delete FeatherClip from Applications. On Ubuntu:

```sh
gnome-extensions disable featherclip@aeen.github  # if enabled
sudo apt remove feather-clip
```

There is no saved clipboard history to remove. The optional autostart entry is the only setting the app writes.

## Contributing

Small bug fixes and improvements are welcome. Open an issue with your OS version, desktop/session type (X11 or Wayland), and steps to reproduce. Please don't include private clipboard contents. Keep contributions focused, preserve the memory-only default, and run the checks above. Release artifacts include SHA-256 checksums; versioned source and lockfiles are committed.

## License

[MIT](LICENSE). Made by **Aeen Karimian** · `aeenakrimian@gmail.com`.
