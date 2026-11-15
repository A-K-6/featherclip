#!/usr/bin/env python3
"""Check shared release metadata, or extract a version's authored release notes."""
import argparse
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def check_version():
    version = (ROOT / "VERSION").read_text().strip()
    if not re.fullmatch(r"\d+\.\d+\.\d+", version):
        raise ValueError("VERSION must contain a stable version such as 2.0.0")
    manifest = (ROOT / "linux/Cargo.toml").read_text()
    package = re.search(r"(?ms)^\[package\]\n(.*?)(?=^\[|\Z)", manifest)
    if not package:
        raise ValueError("Missing [package] section in linux/Cargo.toml")
    manifest_version = re.search(r'^version = "([^"]+)"$', package[1], re.M)
    lock = (ROOT / "linux/Cargo.lock").read_text()
    lock_version = re.search(
        r'\[\[package\]\]\nname = "featherclip"\nversion = "([^"]+)"', lock
    )
    if not manifest_version or not lock_version:
        raise ValueError("Missing FeatherClip package version in Cargo.toml or Cargo.lock")
    if version != manifest_version[1] or version != lock_version[1]:
        raise ValueError("VERSION, linux/Cargo.toml and linux/Cargo.lock must match")
    if f"FeatherClip_{version}_amd64.deb" not in (ROOT / "README.md").read_text():
        raise ValueError("Update the Ubuntu install example in README.md for this version")
    return version


def release_notes(version):
    changelog = (ROOT / "CHANGELOG.md").read_text()
    section = re.search(
        rf"(?ms)^## \[{re.escape(version)}\][^\n]*\n(.*?)(?=^## |\Z)", changelog
    )
    if not section or not section[1].strip():
        raise ValueError(f"CHANGELOG.md needs release notes for {version}")
    return section[1].strip()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--release-notes", action="store_true")
    args = parser.parse_args()
    try:
        current = check_version()
        notes = release_notes(current)
    except (ValueError, OSError) as error:
        parser.exit(1, f"Repository check failed: {error}\n")
    print(notes if args.release_notes else f"Repository metadata OK: {current}")
