LanceDB is a database designed for retrieval, including vector, full-text, and hybrid search.
It is a wrapper around Lance, running in-process like SQLite.

This is TheDeltaLab's fork. We maintain only the Rust core and Node.js bindings.
Python, Java bindings, and remote (LanceDB Cloud) support have been removed.

Project layout:

* `rust/lancedb`: The LanceDB core Rust implementation.
* `nodejs`: The Typescript bindings, using napi-rs

Common commands:

* Check for compiler errors: `cargo check --quiet --tests --examples`
* Run tests: `cargo test --quiet --tests`
* Run specific test: `cargo test --quiet -p <package_name> --test <test_name>`
* Lint: `cargo clippy --quiet --tests --examples`
* Format: `cargo fmt --all`

Before committing changes, run formatting and lint:

1. `cargo fmt --all`
2. `cargo clippy --quiet --tests --examples`

## Coding tips

* When writing Rust doctests for things that require a connection or table reference,
  write them as a function instead of a fully executable test. This allows type checking
  to run but avoids needing a full test environment. For example:
    ```rust
    /// ```
    /// use lance_index::scalar::FullTextSearchQuery;
    /// use lancedb::query::{QueryBase, ExecutableQuery};
    ///
    /// # use lancedb::Table;
    /// # async fn query(table: &Table) -> Result<(), Box<dyn std::error::Error>> {
    /// let results = table.query()
    ///     .full_text_search(FullTextSearchQuery::new("hello world".into()))
    ///     .execute()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    ```

## Example plan: adding a new method on Table

Adding a new method involves first adding it to the Rust core, then exposing it
in the TypeScript bindings.

Rust core changes:

1. Add method on `Table` struct in `rust/lancedb/src/table.rs` (calls `BaseTable` trait).
2. Add method to `BaseTable` trait in `rust/lancedb/src/table.rs`.
3. Implement new trait method on `NativeTable` in `rust/lancedb/src/table.rs`.
    * Test with unit test in `rust/lancedb/src/table.rs`.

TypeScript bindings changes:

1. Add napi-rs method binding on `Table` in `nodejs/src/table.rs`.
2. Run `npm run build` to generate TypeScript definitions.
3. Add typescript method on abstract class `Table` in `nodejs/src/table.ts`.
4. Add concrete method on `LocalTable` class in `nodejs/src/native_table.ts`.
5. Add test in `nodejs/__test__/table.test.ts`.
6. Run `npm run docs` to generate TypeScript documentation.

## Upstream tracking

This fork tracks [lancedb/lancedb](https://github.com/lancedb/lancedb). When upstream
releases a new version:

* Review the release changelog and associated PRs.
* Cherry-pick or merge changes that touch `rust/` and `nodejs/`.
* Ignore changes isolated to `python/`, `java/`, or their docs.
* Ignore changes isolated to `rust/lancedb/src/remote/` (remote support removed).
* Update `[workspace.metadata.upstream].version` in `Cargo.toml` after syncing.

## Review Guidelines

### Rust API design
* Design public APIs so they can be evolved easily in the future without breaking
  changes. Often this means using builder patterns or options structs instead of
  long argument lists.
* For public APIs, prefer inputs that use `Into<T>` or `AsRef<T>` traits to allow
  more flexible inputs. For example, use `name: Into<String>` instead of `name: String`,
  so we don't have to write `func("my_string".to_string())`.

### Testing
* Ensure all new public APIs have documentation and examples.
* Ensure that all bugfixes and features have corresponding tests. **We do not merge
  code without tests.**

### Documentation
* New features must include updates to the rust documentation comments. Link to
  relevant structs and methods to increase the value of documentation.
* **Every code change must include corresponding documentation updates.** After
  modifying TypeScript bindings, always run `cd nodejs && npm run docs` and commit
  the generated doc changes. CI will fail if generated docs are out of date.
