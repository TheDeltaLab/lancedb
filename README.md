# LanceDB — TheDeltaLab Fork (Rust + Node.js)

This is [TheDeltaLab](https://github.com/TheDeltaLab)'s fork of [lancedb/lancedb](https://github.com/lancedb/lancedb).

We maintain **only the Rust core and Node.js bindings**. Python, Java, and related
components have been removed.

## Upstream

| | |
|---|---|
| Upstream repo | https://github.com/lancedb/lancedb |
| Based on upstream version | **0.28.0-beta.1** |

See `[workspace.metadata.upstream]` in `Cargo.toml` for the current upstream version tag.

## Project layout

```
rust/lancedb    Rust core library (crate: lancedb)
nodejs/         Node.js / TypeScript bindings (napi-rs)
```

## Getting started

### Rust

```toml
[dependencies]
lancedb = "0.1.0"
```

```sh
cargo check --tests --examples
cargo test --tests
```

The optional `polars` feature integrates with Polars 0.43. Use the same Polars
version in applications passing DataFrames to LanceDB. Arrow conversions use
`CompatLevel::oldest()` to preserve standard string/binary types across the FFI
boundary. Run `cargo test -p lancedb --features polars arrow::tests` and
`cargo run -p lancedb --features polars --example polars` to verify this integration.

AWS SDK dependencies explicitly enable `default-https-client` and `rt-tokio`
instead of the legacy `rustls` feature, which pulls in hyper 0.14 and h2 0.3.
Dependency updates should retain this choice and pass `cargo deny check`.

### Node.js

```sh
cd nodejs
npm install
npm run build
npm test
```

## Upstream tracking

When upstream `lancedb/lancedb` releases a new version:

1. Review the release changelog / PRs.
2. Cherry-pick or merge changes that touch `rust/` and `nodejs/`.
3. Ignore changes isolated to `python/`, `java/`, or their docs.
4. Update `[workspace.metadata.upstream].version` in `Cargo.toml`.

## License

Apache-2.0 — same as upstream.

## Releasing this fork

The established publishing path is a version-bump PR followed by a tag push:

1. Choose a version unused both in npm and in the repository's tags. Historical
   upstream tags occupy many versions, including `v0.1.8` through `v0.1.14` and
   `v0.1.16` through `v0.1.19`; the next fork release is `0.1.20`.
2. On a release branch, run `bump-my-version bump --new-version VERSION
   --no-commit --no-tag patch`, then `bash ci/update_lockfiles.sh`. Review all
   package versions and locks, commit them, and open a PR.
3. After CI passes and the PR merges, tag the merged commit as `vVERSION` and
   push that single tag. **NPM Publish** builds and tests the native packages,
   then publishes using `NPM_TOKEN`. Stable versions use `latest`; prereleases
   use `preview`. PR runs only pack artifacts and do not publish.
4. Verify the workflow and npm package versions before updating consumers.

This path does not require `LANCEDB_RELEASE_TOKEN`. The fork's historical stable
npm releases can depend on prerelease Lance; the stricter dependency check below
belongs to the optional preparation workflow, not the npm publishing workflow.

**Create release commit** is an optional automation entry point, not the
historical release path. It requires `LANCEDB_RELEASE_TOKEN` even for checkout;
it defaults to `dry_run=true` and preview releases. Its stable mode rejects
prerelease Lance dependencies and does not depend on the removed Python package.

The release script requires a clean checkout, updates all Node/Rust package
versions, checks Conventional Commit `!` and `BREAKING CHANGE:` /
`BREAKING-CHANGE:` markers since the last stable tag, then updates `Cargo.lock`
and `nodejs/pnpm-lock.yaml`. Breaking changes require a minor (or major) bump.
It creates one final commit and an annotated tag only after lockfile updates
and pnpm's frozen-lockfile validation succeed. A failed preparation can leave
uncommitted version edits, so use a disposable checkout for local dry runs.
Release and changelog baselines follow the branch's first-parent history to
avoid unrelated upstream tags. Existing tags are never overwritten. This fork
inherited upstream tags (including `v0.1.8`), so choose an unused version before
preparing a stable release.

With `dry_run=false`, the workflow atomically pushes that branch commit and
that single tag. The tag triggers native builds/tests and npm publishing;
preview packages use the `preview` dist-tag and GitHub prerelease flag.
Dry runs do not push tags, create GitHub releases, or publish npm packages.

To test the release scripts without publishing, install Python 3.11+, Git,
Cargo, pnpm 11.1.1 and `bump-my-version==1.5.1 packaging==26.3`, then run:

```sh
python -m unittest discover -s ci -p test_release.py -v
```

Tests use disposable repositories without remotes and real version/lockfile
commands, including a simulated lock-update failure before commit/tag creation.
