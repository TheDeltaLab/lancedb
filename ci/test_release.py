"""Release transaction tests using real Git, bump-my-version, Cargo and pnpm.

Run with Python >=3.11: python -m unittest discover -s ci -p test_release.py -v
No test repository has a remote; nothing can be published.
"""
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import tomllib
import unittest

ROOT = Path(__file__).resolve().parents[1]


class ReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="lancedb-release-test-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.env = {**os.environ, "CI": "true"}
        self.env.pop("GITHUB_OUTPUT", None)
        shutil.copytree(ROOT / "ci", self.root / "ci", ignore=shutil.ignore_patterns("__pycache__"))
        # Keep fixtures independent of the repository's next published version.
        config = (ROOT / ".bumpversion.toml").read_text()
        self.write(".bumpversion.toml", re.sub(r'(?m)^current_version = .*$', 'current_version = "0.1.7"', config))
        self.write("Cargo.toml", '[workspace]\nmembers = ["rust/lancedb", "nodejs"]\nresolver = "2"\n[workspace.dependencies]\nlance = "1.0.0"\n')
        for path, name in [("rust/lancedb", "lancedb"), ("nodejs", "lancedb-nodejs")]:
            self.write(f"{path}/Cargo.toml", f'[package]\nname = "{name}"\nversion = "0.1.7"\nedition = "2021"\n[lib]\npath = "lib.rs"\n')
            self.write(f"{path}/lib.rs", "")
        for path in ["nodejs/package.json", "nodejs/npm/test-platform/package.json"]:
            self.write(path, json.dumps({"name": "release-fixture", "version": "0.1.7", "private": True}, indent=2) + "\n")
        self.run_cmd("bash", "ci/update_lockfiles.sh")
        self.run_cmd("git", "init", "-q")
        self.run_cmd("git", "config", "user.name", "Release Test")
        self.run_cmd("git", "config", "user.email", "release-test@example.invalid")
        self.run_cmd("git", "add", ".")
        self.run_cmd("git", "commit", "-qm", "chore: initial fixture")
        self.run_cmd("git", "tag", "v0.1.7")

    def write(self, path, text):
        target = self.root / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text)

    def run_cmd(self, *args, success=True):
        result = subprocess.run(args, cwd=self.root, env=self.env, text=True, capture_output=True)
        if success:
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        else:
            self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        return result.stdout + result.stderr

    def verify_release(self, version):
        tag = "v" + version
        self.assertEqual(self.run_cmd("git", "rev-parse", tag + "^{}"), self.run_cmd("git", "rev-parse", "HEAD"))
        self.assertEqual(self.run_cmd("git", "status", "--porcelain"), "")
        for path in ["nodejs/package.json", "nodejs/npm/test-platform/package.json"]:
            self.assertEqual(json.loads(self.run_cmd("git", "show", f"{tag}:{path}"))["version"], version)
        for path in ["rust/lancedb/Cargo.toml", "nodejs/Cargo.toml"]:
            self.assertEqual(tomllib.loads(self.run_cmd("git", "show", f"{tag}:{path}"))["package"]["version"], version)
        lock = tomllib.loads(self.run_cmd("git", "show", f"{tag}:Cargo.lock"))
        self.assertEqual({p["version"] for p in lock["package"]}, {version})
        self.run_cmd("git", "show", f"{tag}:nodejs/pnpm-lock.yaml")
        self.run_cmd("pnpm", "--dir", "nodejs", "install", "--lockfile-only", "--ignore-scripts", "--frozen-lockfile")
        self.assertFalse((self.root / "nodejs/package-lock.json").exists())
        self.assertEqual(self.run_cmd("git", "remote"), "")

    def test_preview_then_stable(self):
        self.run_cmd("bash", "ci/bump_version.sh", "preview", "false")
        self.verify_release("0.1.8-beta.0")
        self.run_cmd("bash", "ci/bump_version.sh", "preview", "false")
        self.verify_release("0.1.8-beta.1")
        self.run_cmd("bash", "ci/bump_version.sh", "stable", "false")
        self.verify_release("0.1.8")

    def test_unrelated_upstream_tags_do_not_set_baseline(self):
        tree = self.run_cmd("git", "rev-parse", "HEAD^{tree}").strip()
        unrelated = self.run_cmd("git", "commit-tree", tree, "-m", "upstream release").strip()
        self.run_cmd("git", "tag", "v99.0.0", unrelated)
        self.run_cmd("bash", "ci/bump_version.sh", "preview", "false")
        self.verify_release("0.1.8-beta.0")

    def test_existing_tag_is_never_overwritten(self):
        before = self.run_cmd("git", "rev-parse", "HEAD")
        self.run_cmd("git", "tag", "v0.1.8-beta.0")
        output = self.run_cmd("bash", "ci/bump_version.sh", "preview", "false", success=False)
        self.assertIn("Release tag already exists", output)
        self.assertEqual(self.run_cmd("git", "rev-parse", "HEAD"), before)
        self.assertEqual(self.run_cmd("git", "rev-parse", "v0.1.8-beta.0"), before)

    def test_direct_stable(self):
        self.run_cmd("bash", "ci/bump_version.sh", "stable", "false")
        self.verify_release("0.1.8")
        self.assertEqual(self.run_cmd("git", "rev-list", "--count", "v0.1.7..HEAD").strip(), "1")

    def test_breaking_change_requires_minor(self):
        self.run_cmd("git", "commit", "--allow-empty", "-qm", "feat(api)!: change interface")
        before = self.run_cmd("git", "rev-parse", "HEAD")
        output = self.run_cmd("bash", "ci/bump_version.sh", "preview", "false", success=False)
        self.assertIn("Breaking changes require", output)
        self.assertEqual(self.run_cmd("git", "rev-parse", "HEAD"), before)
        self.assertEqual(self.run_cmd("git", "tag", "--list").strip(), "v0.1.7")
        # Restore only fixture files changed by the rejected bump, then choose minor.
        self.run_cmd("git", "restore", ".")
        self.run_cmd("bash", "ci/bump_version.sh", "preview", "true")
        self.verify_release("0.2.0-beta.0")

    def test_stable_rejects_prerelease_before_mutation(self):
        path = self.root / "Cargo.toml"
        path.write_text(path.read_text().replace('lance = "1.0.0"', 'lance = ">=9.1.0-beta"'))
        self.run_cmd("git", "add", ".")
        self.run_cmd("git", "commit", "-qm", "chore: use preview Lance")
        before = self.run_cmd("git", "rev-parse", "HEAD")
        output = self.run_cmd("bash", "ci/bump_version.sh", "stable", "false", success=False)
        self.assertIn("Stable release cannot use prerelease", output)
        self.assertEqual(self.run_cmd("git", "rev-parse", "HEAD"), before)
        self.assertEqual(self.run_cmd("git", "status", "--porcelain"), "")

    def test_lock_failure_does_not_commit_or_tag(self):
        # Inject failure only at the external lock-update boundary.
        self.write("fail-bin/pnpm", "#!/bin/sh\necho lock-update-failed >&2\nexit 23\n")
        (self.root / "fail-bin/pnpm").chmod(0o755)
        self.run_cmd("git", "add", ".")
        self.run_cmd("git", "commit", "-qm", "test: inject lock update failure")
        before = self.run_cmd("git", "rev-parse", "HEAD")
        self.env["PATH"] = str(self.root / "fail-bin") + os.pathsep + self.env["PATH"]
        output = self.run_cmd("bash", "ci/bump_version.sh", "preview", "false", success=False)
        self.assertIn("lock-update-failed", output)
        self.assertEqual(self.run_cmd("git", "rev-parse", "HEAD"), before)
        self.assertEqual(self.run_cmd("git", "tag", "--list").strip(), "v0.1.7")


if __name__ == "__main__":
    unittest.main()
