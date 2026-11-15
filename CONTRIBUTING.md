# Contributing

Keep FeatherClip small, native and local. Preserve memory-only history, bounded payloads and a simple popup. Bug reports should include the app/OS version, desktop/session type (GNOME Wayland or X11 on Linux), expected behavior and reproduction steps. Use synthetic clips when testing or sharing a report.

## Project layout

| Path | Responsibility |
| --- | --- |
| `macos/` | Swift/AppKit app, clipboard core and native pasteboard tests |
| `linux/` | Rust/GTK4/libadwaita app, clipboard worker and paste integrations |
| `gnome/` | Small GNOME Shell bridge for Wayland clipboard access and paste |
| `site/` | Static GitHub Pages website, blog and setup guide |
| `assets/` | App icons and the synthetic demo image |
| `scripts/` | Packaging, metadata checks and isolated desktop fixtures |
| `VERSION` | Shared installer version, checked against Cargo.toml and Cargo.lock |

## Local checks

Follow [README](README.md#build) for platform prerequisites. Python 3 is used for metadata checks; the app has no Python runtime dependency.

On either platform:

```sh
python3 scripts/check-repo.py
git diff --check
```

On Mac, use Xcode with the macOS 26 SDK or newer:

```sh
swift format format --in-place --recursive macos
swift format lint --strict --recursive macos
swift test --package-path macos
bash scripts/build-macos.sh arm64 --dmg  # x86_64 for an Intel build
build/macos-arm64/FeatherClip.app/Contents/MacOS/FeatherClip --smoke-test
```

On Ubuntu 24.04+ amd64:

```sh
cargo fmt --manifest-path linux/Cargo.toml
cargo test --locked --manifest-path linux/Cargo.toml
cargo clippy --locked --manifest-path linux/Cargo.toml --all-targets -- -D warnings
bash scripts/build-linux.sh
linux/target/release/featherclip --smoke-test
```

`--demo` and `--smoke-test` use synthetic clips and an in-memory clipboard. Real paste acceptance requires a foreground destination: copy a test sentence in TextEdit or a text editor, delete it, open FeatherClip with its shortcut, select it, and verify it arrives. A successful Paste event alone does not prove the destination received it.

CI exercises real text/image/file round trips and previous-window paste in disposable X11 and GNOME Wayland sessions. `scripts/test-gnome.sh` starts an unsafe headless compositor and changes test-session settings; both desktop fixture scripts deliberately require `CI=true`. Run them only in disposable CI environments. The normal app does not enable unsafe GNOME mode.

## Releases

1. Update `VERSION`, the package version in `linux/Cargo.toml`, and its matching entry in `linux/Cargo.lock` (use Cargo to refresh the lockfile). Update the Ubuntu install example in README. Add a dated `CHANGELOG.md` section and update its comparison links. The GNOME extension's integer version changes only when the bridge changes.
2. Run the relevant local checks, push the reviewed commit to `main`, and wait for all [build checks](https://github.com/A-K-6/featherclip/actions) to pass.
3. Tag that exact commit and push the tag:

```sh
git tag -a vX.Y.Z -m 'FeatherClip X.Y.Z'
git push origin vX.Y.Z
```

A version tag runs the full Mac ARM64, Mac Intel and Ubuntu checks again. After they all pass, CI publishes those three installers, a SHA256SUMS file, and the authored changelog section as the GitHub release. A tag that disagrees with `VERSION` fails. Existing releases are not overwritten; inspect a partial release before retrying a failed publishing job.

The scripts generate fresh staging folders under ignored `build/` directories and remove those temporary folders when done. Swift and Cargo caches stay in their usual ignored directories. Mac signing is currently ad-hoc; a different binary can require the owner to refresh Accessibility permission. Local builds do not replace an installed app automatically.

## Website and blog

The GitHub Pages site uses static HTML/CSS and Python's standard library to assemble its shared layout. It has no browser JavaScript or frontend package dependencies. Edit `site/pages/`, `site/layout.html` and `site/assets/style.css`; add new page metadata to `PAGES` in `scripts/build-site.py`.

```sh
gh api repos/A-K-6/featherclip/releases/latest > /tmp/featherclip-site-release.json
python3 scripts/build-site.py --release-json /tmp/featherclip-site-release.json
python3 -m http.server 8765 --directory build/site --bind 127.0.0.1
```

Open `http://127.0.0.1:8765`. The build checks local links, assets and fragments, and verifies download links against the latest published release. Unreleased source versions cannot create nonexistent download links. Blog posts keep their original dates and release references.

Website changes on `main` deploy automatically, and pull requests build without deploying. Release publishing dispatches the website workflow after the installers are published. To refresh manually, run `gh workflow run pages.yml --ref main`. Pages settings use GitHub Actions; no custom domain is required. The website is public, like the source repository.
