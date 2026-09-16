#!/usr/bin/env python3
"""Validate Rust Lance dependencies without relying on removed Python bindings."""
import re
import sys
import tomllib


def validate(manifest="Cargo.toml"):
    with open(manifest, "rb") as handle:
        dependencies = tomllib.load(handle)["workspace"]["dependencies"]
    for name, dependency in dependencies.items():
        if name != "lance" and not name.startswith("lance-"):
            continue
        versions = [dependency] if isinstance(dependency, str) else [
            dependency.get("version", ""), dependency.get("tag", "")
        ]
        if not any(versions):
            raise ValueError(f"Cannot establish a stable version for {name}")
        if any(re.search(r"\d+\.\d+\.\d+-[0-9A-Za-z]", v) for v in versions):
            raise ValueError(f"Stable release cannot use prerelease dependency {name}: {versions}")


if __name__ == "__main__":
    try:
        validate()
    except ValueError as error:
        sys.exit(str(error))
