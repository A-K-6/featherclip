# Changelog

## [Unreleased]

## [2.0.1] — 2026-11-16

- Focus the Mac popup and search when opened, while remembering the original paste destination.
- Support Tab/Shift+Tab through clips and controls, with Space/Enter for buttons and options.
- Preserve the selected clip during history updates and cancel stale paste retries when reopened.
- Exercise native keyboard focus, navigation and empty-history controls in the Mac smoke check.

- Add a GitHub Pages download site, project blog, and setup/troubleshooting guide.

- Centralize installer versions and validate release metadata.
- Standardize native source formatting and document contributing and releases.
- Refresh CI, add dependency update proposals, and build releases from version tags.
- Build installers from fresh staging folders and verify the installed Ubuntu package in CI.

## [2.0.0] — 2026-11-15

- Replace the Tauri interface with Swift/AppKit on macOS and Rust/GTK4/libadwaita on Ubuntu.
- Use real macOS glass on macOS 26+, with native translucent material on macOS 13–15.
- Follow each desktop's native light/dark appearance.
- Keep text, images, local file references, search and previous-app paste in a small memory-only popup.
- Ship separate Apple Silicon, Intel and Ubuntu installers with SHA-256 checksums.

Mac builds are ad-hoc signed and are not Apple-notarized. Upgrading may require refreshing FeatherClip's Accessibility entry. Ubuntu/GNOME Wayland requires the bundled GNOME bridge; first installation may require a logout/login. See the [installation guide](https://github.com/A-K-6/featherclip#install).

## [1.1.0] — 2026-11-15

- Add images, file references and paste into the previous app.
- Keep the Mac popup separate from window tabs and support full-screen Spaces.
- Add separate Apple Silicon and Intel installers and improve GNOME Wayland support.

## [1.0.0] — 2026-11-15

- Initial open-source, memory-only text clipboard for macOS and Ubuntu.

[Unreleased]: https://github.com/A-K-6/featherclip/compare/v2.0.1...main
[2.0.1]: https://github.com/A-K-6/featherclip/releases/tag/v2.0.1
[2.0.0]: https://github.com/A-K-6/featherclip/releases/tag/v2.0.0
[1.1.0]: https://github.com/A-K-6/featherclip/releases/tag/v1.1.0
[1.0.0]: https://github.com/A-K-6/featherclip/releases/tag/v1.0.0
