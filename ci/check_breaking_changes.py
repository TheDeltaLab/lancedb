#!/usr/bin/env python3
"""Require a minor (or major) release for Conventional Commit breaking changes."""
import re
import subprocess
import sys

from packaging.version import Version


def validate(base, head, old_version, new_version):
    old, new = Version(old_version), Version(new_version)
    if new <= old:
        raise ValueError("Release must be newer than the stable baseline")
    messages = subprocess.check_output(
        ["git", "log", "--format=%B%x00", f"{base}..{head}"], text=True
    )
    breaking = re.search(
        r"(?m)^(?:[a-z]+(?:\([^\n]+\))?!:|BREAKING[ -]CHANGE:)", messages
    )
    if breaking and (new.major, new.minor) <= (old.major, old.minor):
        raise ValueError("Breaking changes require bump-minor=true (or a major release)")


if __name__ == "__main__":
    if len(sys.argv) != 5:
        sys.exit("Usage: check_breaking_changes.py BASE HEAD OLD_VERSION NEW_VERSION")
    try:
        validate(*sys.argv[1:])
    except ValueError as error:
        sys.exit(str(error))
