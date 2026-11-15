#!/usr/bin/env python3
"""Build the static GitHub Pages site from a verified, published GitHub release."""
import argparse
import html
import json
import re
import shutil
import tempfile
from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import unquote, urlparse

ROOT = Path(__file__).resolve().parent.parent
PAGES = {
    "index.html": (
        "FeatherClip — A native clipboard for Mac and Ubuntu",
        "A tiny, native clipboard history for macOS and Ubuntu. Text, pictures, files, and previous-app paste. Free, open source, and memory-only.",
    ),
    "guide.html": (
        "Setup & troubleshooting — FeatherClip",
        "Install FeatherClip on Mac or Ubuntu, enable direct paste, refresh macOS Accessibility permissions, and set up the GNOME Wayland bridge.",
    ),
    "blog/index.html": (
        "The FeatherClip blog — Native apps, small changes",
        "Release stories and decisions behind FeatherClip, a small native clipboard for Mac and Ubuntu.",
    ),
    "blog/going-native.html": (
        "FeatherClip 2.0: a native home for your clipboard",
        "Why FeatherClip 2.0 uses real macOS glass, AppKit on Mac, and GTK4 with libadwaita on Ubuntu.",
    ),
}


class Links(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.targets = []
        self.ids = set()

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if "id" in attrs:
            if attrs["id"] in self.ids:
                raise ValueError(f"Duplicate HTML id: {attrs['id']}")
            self.ids.add(attrs["id"])
        for key in ("href", "src"):
            if attrs.get(key):
                self.targets.append(attrs[key])


def release_values(path):
    release = json.loads(path.read_text())
    match = re.fullmatch(r"v(\d+\.\d+\.\d+)", release.get("tag_name", ""))
    if not match or release.get("draft") or release.get("prerelease"):
        raise ValueError("Use a published stable release for the download site")
    version = match[1]
    base = f"https://github.com/A-K-6/featherclip/releases/download/v{version}/"
    assets = {asset["name"]: asset["browser_download_url"] for asset in release["assets"]}
    values = {"VERSION": version, "RELEASE_URL": f"https://github.com/A-K-6/featherclip/releases/tag/v{version}"}
    for key, filename in {
        "MAC_ARM_URL": f"FeatherClip_{version}_arm64.dmg",
        "MAC_INTEL_URL": f"FeatherClip_{version}_x86_64.dmg",
        "LINUX_URL": f"FeatherClip_{version}_amd64.deb",
        "CHECKSUMS_URL": "SHA256SUMS",
    }.items():
        expected = base + filename
        if assets.get(filename) != expected:
            raise ValueError(f"Release is missing the expected asset: {filename}")
        values[key] = expected
    return values


def render(template, values):
    def replace(match):
        key = match[1]
        if key not in values:
            raise ValueError(f"Unknown site template value: {key}")
        return values[key]
    return re.sub(r"\{\{([A-Z_]+)\}\}", replace, template)


def validate(folder):
    parsed = {}
    for path in folder.rglob("*.html"):
        parser = Links()
        parser.feed(path.read_text())
        parsed[path.resolve()] = parser
    for path, parser in parsed.items():
        for target in parser.targets:
            url = urlparse(target)
            if url.scheme or url.netloc:
                if url.scheme not in ("https", "mailto"):
                    raise ValueError(f"Unexpected URL scheme in {path.name}: {target}")
                continue
            if url.path.startswith("/"):
                raise ValueError(f"Use project-relative links: {target}")
            resolved = (path.parent / unquote(url.path)).resolve() if url.path else path
            if resolved.is_dir():
                resolved /= "index.html"
            if not resolved.is_relative_to(folder.resolve()) or not resolved.is_file():
                raise ValueError(f"Broken local link in {path.name}: {target}")
            if url.fragment and url.fragment not in parsed.get(resolved, Links()).ids:
                raise ValueError(f"Missing fragment in {path.name}: {target}")


def build(release_json, base_url):
    if urlparse(base_url).scheme != "https":
        raise ValueError("The canonical site URL must use HTTPS")
    values = release_values(release_json)
    output = ROOT / "build/site"
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".site-", dir=output.parent) as temporary:
        stage = Path(temporary) / "site"
        stage.mkdir()
        shutil.copytree(ROOT / "site/assets", stage / "assets")
        shutil.copy2(ROOT / "assets/featherclip.svg", stage / "assets/icon.svg")
        layout = (ROOT / "site/layout.html").read_text()
        for name, (title, description) in PAGES.items():
            prefix = "../" * len(Path(name).parent.parts)
            page_values = {key: html.escape(value, quote=True) for key, value in values.items()}
            page_values.update({
                "ROOT": prefix,
                "TITLE": html.escape(title),
                "DESCRIPTION": html.escape(description, quote=True),
                "CANONICAL": html.escape(base_url.rstrip("/") + "/" + ("" if name == "index.html" else name), quote=True),
                "OG_TYPE": "article" if name == "blog/going-native.html" else "website",
            })
            page_values["CONTENT"] = render((ROOT / "site/pages" / name).read_text(), page_values)
            destination = stage / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_text(render(layout, page_values))
        (stage / ".nojekyll").touch()
        validate(stage)
        if output.exists():
            output.rename(Path(temporary) / "previous")
        stage.rename(output)
    print(f"Built and validated {len(PAGES)} pages for FeatherClip {values['VERSION']}: {output}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--release-json", required=True, type=Path)
    parser.add_argument("--base-url", default="https://a-k-6.github.io/featherclip/")
    args = parser.parse_args()
    try:
        build(args.release_json, args.base_url)
    except (ValueError, OSError, KeyError) as error:
        parser.exit(1, f"Site build failed: {error}\n")
