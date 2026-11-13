#!/usr/bin/env python3
"""Create Tauri's static, signed GitHub update manifest from release artifacts.

This file only reads PUBLIC signatures. The private signing key must never be
committed or included in the release assets.
"""

import argparse
import datetime
import json
import pathlib
import re


def create_manifest(assets: pathlib.Path, tag: str, repository: str) -> dict:
    if not re.fullmatch(r"v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?", tag):
        raise ValueError(f"Expected a valid release tag like v0.2.0, got: {tag!r}")
    if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository):
        raise ValueError("Invalid GitHub owner/repository")

    platforms = {}
    for architecture, target in (
        ("apple-silicon", "darwin-aarch64"),
        ("intel", "darwin-x86_64"),
    ):
        archive = f"WhaRa_{tag}_{architecture}.app.tar.gz"
        path = assets / archive
        signature_file = assets / f"{archive}.sig"
        if not path.is_file() or path.stat().st_size == 0:
            raise FileNotFoundError(f"Missing signed updater archive: {path}")
        if not signature_file.is_file():
            raise FileNotFoundError(f"Missing update signature: {signature_file}")
        signature = signature_file.read_text(encoding="utf-8").strip()
        if not signature:
            raise ValueError(f"Empty update signature for {architecture}")
        platforms[target] = {
            "signature": signature,
            "url": f"https://github.com/{repository}/releases/download/{tag}/{archive}",
        }
        # Support updater variants that append the macOS .app bundle type.
        platforms[target + "-app"] = platforms[target]

    return {
        "version": tag.removeprefix("v"),
        "notes": f"WhaRa {tag}: Improvements and bug fixes. See the GitHub release notes.",
        "pub_date": datetime.datetime.now(datetime.timezone.utc)
        .isoformat(timespec="seconds")
        .replace("+00:00", "Z"),
        "platforms": platforms,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--assets", type=pathlib.Path, required=True)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--repository", required=True)
    options = parser.parse_args()

    manifest = create_manifest(options.assets, options.tag, options.repository)
    output = options.assets / "latest.json"
    output.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    print(f"Created {output.name} for {manifest['version']} with both macOS architectures")


if __name__ == "__main__":
    main()
