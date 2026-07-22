# Upstream Sync Workflow

This document describes how we keep our fork in sync with the upstream [lancedb/lancedb](https://github.com/lancedb/lancedb) repository.

## Branch Strategy

| Branch | Purpose |
|--------|---------|
| `main` | Our primary development branch. Contains our customizations and maintained code (Rust core + Node.js bindings only). |
| `sync` | Mirror branch that tracks upstream `lancedb/lancedb` main. Used solely as a staging area for incoming upstream changes. |

```
upstream/main ──> sync ──PR──> main
```

## Updating main from Upstream

### Step 1: Update the sync branch

Fetch the latest changes from upstream and fast-forward the `sync` branch:

```bash
# Add upstream remote (one-time setup)
git remote add upstream https://github.com/lancedb/lancedb.git

# Fetch upstream changes
git fetch upstream

# Update the sync branch
git checkout sync
git merge --ff-only upstream/main
git push origin sync
```

### Step 2: Create a PR from sync to main

```bash
# Create a new branch for the merge PR
git checkout main
git pull origin main
git checkout -b sync/merge-YYYY-MM-DD

# Merge sync into the new branch
git merge sync
```

### Step 3: Resolve conflicts

Conflicts are expected since our fork removes Python/Java bindings and carries custom patches. Common conflict areas:

- **`Cargo.toml` / `Cargo.lock`**: Dependency changes. Keep our workspace structure; adopt upstream version bumps.
- **`rust/lancedb/`**: Core Rust changes. Merge carefully — these are the changes we most want.
- **`nodejs/`**: Node.js binding changes. Merge carefully — also important for us.
- **`python/` / `java/`**: We have removed these. Discard any upstream changes in these directories:
    ```bash
    git checkout --ours python/ java/
    git rm -r python/ java/
    ```
- **`rust/lancedb/src/remote/`**: Remote/LanceDB Cloud support has been removed from this fork. Discard any upstream changes to remote modules:
    ```bash
    git rm -rf rust/lancedb/src/remote/
    ```
    Also watch for upstream changes that add `#[cfg(feature = "remote")]` blocks in shared files (e.g., `connection.rs`, `table.rs`, `error.rs`, `database/listing.rs`) — these should be dropped during conflict resolution.
- **CI/CD configs (`.github/`)**: Review case-by-case. Keep our customized workflows; adopt useful upstream CI improvements.

After resolving conflicts:

```bash
git add .
git commit
git push origin sync/merge-YYYY-MM-DD
```

### Step 4: Review and merge the PR

Open a Pull Request from `sync/merge-YYYY-MM-DD` into `main`.

The PR body **must** list every upstream PR included in the sync, using full
URLs so reviewers can trace each change back to the original discussion:

```bash
gh pr create --base main --head sync/merge-YYYY-MM-DD \
  --title "sync: merge upstream YYYY-MM-DD" \
  --body "Merge upstream lancedb/lancedb changes into main.

Upstream PRs included:
- https://github.com/lancedb/lancedb/pull/XXXX
- https://github.com/lancedb/lancedb/pull/YYYY
"
```

Review checklist:

- [ ] All `rust/` and `nodejs/` changes are included
- [ ] No `python/` or `java/` files reintroduced
- [ ] No `rust/lancedb/src/remote/` files or `#[cfg(feature = "remote")]` code reintroduced
- [ ] `cargo check --quiet --tests --examples` passes
- [ ] `cargo test --quiet --tests` passes
- [ ] Node.js bindings build: `cd nodejs && npm run build`
- [ ] Update `[workspace.metadata.upstream].version` in root `Cargo.toml`

### Step 5: Post-merge cleanup

```bash
# Delete the merge branch
git branch -d sync/merge-YYYY-MM-DD
git push origin --delete sync/merge-YYYY-MM-DD
```

## Quick Reference

```bash
# Full sync in one go (after initial setup)
git fetch upstream
git checkout sync && git merge upstream/main && git push origin sync
git checkout main && git pull
git checkout -b sync/merge-$(date +%Y-%m-%d)
git merge sync
# ... resolve conflicts ...
git add . && git commit
git push origin sync/merge-$(date +%Y-%m-%d)
gh pr create --base main --head sync/merge-$(date +%Y-%m-%d) \
  --title "sync: merge upstream $(date +%Y-%m-%d)" \
  --body "Merge upstream lancedb/lancedb changes into main."
```

## Automated Workflow

A GitHub Actions workflow (`.github/workflows/upstream-sync.yml`) runs every Monday at 09:07 UTC to automate the initial sync steps.

### What it does

1. Fetches `upstream/main` and compares it with the `sync` branch.
2. If there are no new commits, the workflow exits early.
3. Fast-forwards the `sync` branch to match `upstream/main`.
4. Checks whether an open sync PR already exists (to avoid duplicates).
5. Creates a merge branch (`sync/merge-YYYY-MM-DD`) from `main` and attempts `git merge sync`.
6. **No conflicts**: pushes the branch and opens a PR into `main` automatically.
7. **Conflicts**: aborts the merge and creates a GitHub issue tagged `upstream-sync` with instructions for manual resolution.

### When manual intervention is needed

- **Merge conflicts** — The workflow cannot resolve conflicts. When an issue is created, follow the manual steps in the issue body (or the instructions above in this document) to complete the merge.
- **Duplicate PRs** — If an open sync PR already exists, the workflow skips creating a new one. Merge or close the existing PR first.
- **Fast-forward failures** — If the `sync` branch has diverged from `upstream/main` (it should not under normal use), the `--ff-only` merge will fail. Reset `sync` to `upstream/main` manually.

The workflow can also be triggered manually via `workflow_dispatch` from the Actions tab.

## Notes

- We only maintain **Rust core** (`rust/`) and **Node.js bindings** (`nodejs/`). Ignore upstream changes to `python/` and `java/`.
- **Remote/Cloud support is removed.** Ignore upstream changes isolated to `rust/lancedb/src/remote/`, `nodejs/src/remote.rs`, `nodejs/src/header.rs`, `nodejs/lancedb/header.ts`, and the `remote` cargo feature. See PR #35 for the full removal.
- Always review the upstream [release changelog](https://github.com/lancedb/lancedb/releases) before syncing to understand what changed.
- See [CLAUDE.md](../../CLAUDE.md) "Upstream tracking" section for additional guidelines.
