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
