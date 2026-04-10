# Performance Profiling Design

> Status: Draft
> Date: 2026-04-08
> Author: TheDeltaLab

---

## 1. Current State

### 1.1 Logging Framework

LanceDB currently uses `log` (0.4) + `env_logger` (0.11) as its logging solution. A small
number of `log::warn!` / `log::debug!` / `log::info!` calls are scattered throughout the
codebase, primarily for warnings and debugging, with no performance measurement. The Node.js
bindings control the log level via the `LANCEDB_LOG` environment variable (defaults to `warn`).

**Not used:** `tracing` crate, `#[instrument]` annotations, spans, or structured events.

### 1.2 Underlying Lance Library

Lance (v5.0.0-beta.5) already depends on `tracing` (0.1), which means:

- Internal spans/events in Lance can already be collected by an upstream subscriber
- LanceDB can directly reuse the `tracing` ecosystem without introducing additional bridging layers
- `tracing` is natively compatible with the `log` crate (`tracing` provides a `log` feature for automatic bridging)

### 1.3 Existing Performance Measurement Capabilities

| Module | File | Capability |
|--------|------|------------|
| I/O Statistics | `rust/lancedb/src/io/object_store/io_tracking.rs` | `IoTrackingStore` wraps `ObjectStore`, accumulating read/write IOPS and bytes |
| Write Progress | `rust/lancedb/src/table/write_progress.rs` | `WriteProgressTracker` reports row count, bytes, and elapsed time via callbacks |
| Optimization Stats | `rust/lancedb/src/table/optimize.rs` | `OptimizeStats` aggregates compaction and removal metrics |
| Query Timeout | `rust/lancedb/src/utils/mod.rs` | `TimeoutStream` adds timeout control to query streams |

These mechanisms are independent of each other, with no unified collection or export channel.

### 1.4 Query Execution Path Overview

```
User API (Table::query())
  → Query builder (QueryBase / ExecutableQuery trait)
    → BaseTable::query() / BaseTable::create_plan()
      → Lance dataset scanner
        → ObjectStore (possibly wrapped by IoTrackingStore)
          → Local files / Cloud storage
```

Key traits:

- `BaseTable` (`rust/lancedb/src/table.rs:232`): defines `query()` / `create_plan()`
- `ExecutableQuery` (`rust/lancedb/src/query.rs:621`): defines `execute()` / `create_plan()`
- `NativeTable`: local table implementation

---

## 2. Goals

1. **Function call tracing**: Record key function invocations and their duration (query, index, write, optimize paths)
2. **File I/O tracing**: Record each I/O operation at the ObjectStore layer and its duration (read/write/list, etc.)
3. **OpenTelemetry support**: Allow users to export traces/metrics to OTLP-compatible backends
4. **Low coupling**: Do not intrude on existing business logic; can be fully removed via feature flags

---

## 3. Scope

### 3.1 In-Scope

- Introduce the `tracing` crate to replace (and bridge) the `log` crate
- Add `#[instrument]` annotations to key functions
- Enhance `IoTrackingStore` to emit `tracing` spans/events (with duration)
- Provide `tracing-opentelemetry` integration as an optional feature
- Provide a built-in simple profiling subscriber (e.g., JSON log output)
- Expose profiling control interfaces in the Node.js bindings (enable/disable, configure OTLP endpoint)

### 3.2 Out-of-Scope

- Instrumentation changes inside the Lance library (maintained by upstream)
- Real-time dashboards or UI visualization
- Custom metrics beyond I/O tracking (e.g., query-level histograms) — first phase focuses on tracing and basic I/O metrics (read/write bytes, IOPS, duration)
- CPU profiling (e.g., pprof integration)
- Memory allocation tracking

---

## 4. Design Principles

1. **Zero-cost abstraction**: When profiling is not enabled, tracing macros compile to no-ops or extremely low-overhead calls. `tracing` crate spans have nanosecond-level overhead when no subscriber is active.
2. **Feature-gated**: All profiling dependencies (`tracing-subscriber`, `tracing-opentelemetry`, `opentelemetry-otlp`, etc.) are controlled via cargo features, with no impact on default binary size.
3. **Decorator / Wrapper pattern**: Inject tracing logic by wrapping existing components (e.g., `ObjectStore`) rather than modifying their internals, minimizing changes to existing code.
4. **Structured data**: All spans and events carry structured fields (table name, query type, bytes read, etc.) rather than plain text messages, facilitating downstream analysis.
5. **User-controllable**: Profiling enable/disable, sampling rate, and export targets are all configurable at runtime without requiring recompilation.

---

## 5. Design Considerations

### 5.1 Approach Selection: Why `tracing` + OpenTelemetry

| Approach | Pros | Cons |
|----------|------|------|
| **`tracing` ecosystem** | Rust ecosystem standard; Lance already depends on it; zero-cost; async-friendly; rich subscriber ecosystem | Requires learning the span model |
| Custom profiling framework | Full control | Reinventing the wheel; isolated ecosystem; high maintenance cost |
| `metrics` crate | Focused on metrics | No trace support; lacks call chain context |
| Pure `log` enhancement | Minimal changes | Cannot record duration; no call chains; no OTLP support |

**Conclusion**: `tracing` + `tracing-opentelemetry` is the most reasonable choice.

### 5.2 Instrumentation Layers

A layered instrumentation strategy, from coarse to fine:

```
┌──────────────────────────────────────────────────────┐
│ Layer 1: Public API entry points                     │
│   Table::query(), Table::add(), Table::delete()      │
│   → #[instrument] annotations                        │
├──────────────────────────────────────────────────────┤
│ Layer 2: Internal critical paths                     │
│   BaseTable::query(), create_plan(),                 │
│   index creation, optimize                           │
│   → #[instrument] annotations                        │
├──────────────────────────────────────────────────────┤
│ Layer 3: I/O operations                              │
│   IoTrackingStore's ObjectStore implementation        │
│   → Emit spans with duration for each get/put/list   │
├──────────────────────────────────────────────────────┤
│ Layer 4: Lance internals (provided by upstream)      │
│   Dataset scan, index search, etc.                   │
│   → Existing tracing spans, automatically collected  │
└──────────────────────────────────────────────────────┘
```

### 5.3 `IoTrackingStore` Enhancement

Currently `IoTrackingStore` only accumulates statistics. The enhancement direction:

```rust
// Before
async fn get(&self, location: &Path) -> OSResult<GetResult> {
    let result = self.target.get(location).await;
    if let Ok(result) = &result {
        self.record_read(num_bytes);
    }
    result
}

// After
async fn get(&self, location: &Path) -> OSResult<GetResult> {
    let span = tracing::info_span!("object_store.get",
        path = %location,
        bytes = tracing::field::Empty,
    );
    let _guard = span.enter();
    let result = self.target.get(location).await;
    if let Ok(result) = &result {
        let num_bytes = result.range.end - result.range.start;
        span.record("bytes", num_bytes);
        self.record_read(num_bytes);
    }
    result
}
```

`tracing` spans automatically record entry and exit times; subscribers can compute duration from this. The existing `IoStats` cumulative statistics remain unchanged — both coexist.

### 5.4 Feature Flag Design

```toml
# rust/lancedb/Cargo.toml
[features]
default = []
profiling = ["tracing/attributes", "tracing-subscriber"]
profiling-otlp = [
    "profiling",
    "tracing-opentelemetry",
    "opentelemetry",
    "opentelemetry_sdk",
    "opentelemetry-otlp",
]
```

- `profiling`: Enables `#[instrument]` and basic subscribers (log/JSON output)
- `profiling-otlp`: Adds OTLP export capability

**When features are not enabled**: `#[instrument]` degrades to an empty attribute via conditional compilation; tracing macros produce only nanosecond-level overhead without a subscriber.

### 5.5 Conditional `#[instrument]` Usage Strategy

To avoid unnecessary intrusion on core code paths, `#[instrument]` is only added to:

1. **Public API methods** (`Table`'s `query`, `add`, `delete`, `optimize`, etc.)
2. **Key trait implementation methods** (`BaseTable::query`, `BaseTable::create_plan`)
3. **I/O wrapper methods** (`IoTrackingStore`'s `ObjectStore` methods)

Internal helper functions are not instrumented to avoid noise and performance impact.

### 5.6 User Integration

#### Rust Users

```rust
use lancedb::profiling;

// Option 1: Use the built-in JSON subscriber
profiling::init_json_subscriber();

// Option 2: Use OTLP export
let otlp_config = profiling::OtlpConfig::default();
let _guard = profiling::init_otlp(otlp_config).unwrap();

// Option 3: User-built subscriber (full control)
use tracing_subscriber::prelude::*;
let otel_layer = /* custom OpenTelemetry layer */;
tracing_subscriber::registry()
    .with(otel_layer)
    .init();
```

#### Node.js Users

```typescript
import { initProfiling } from '@lancedb/lancedb';

// Enable OTLP export
initProfiling({ endpoint: 'http://localhost:4318' });

// Or JSON log output
initProfiling({ format: 'json' });
```

### 5.7 Span Naming Convention

Following OpenTelemetry semantic convention style:

| Span Name | Structured Fields |
|-----------|-------------------|
| `lancedb.table.query` | `table`, `query_type`, `top_k`, `filter` |
| `lancedb.table.add` | `table`, `num_rows`, `mode` |
| `lancedb.table.delete` | `table`, `predicate` |
| `lancedb.table.optimize` | `table`, `action` |
| `lancedb.index.create` | `table`, `index_type`, `columns` |
| `lancedb.io.get` | `path`, `bytes` |
| `lancedb.io.put` | `path`, `bytes` |
| `lancedb.io.list` | `prefix` |

### 5.8 Handling Existing `log` Macros

`tracing` provides a `log` feature (enabled by default) that automatically converts
`log::info!` and similar calls into tracing events. Therefore, replace `log::*` with
`tracing::*` to gain structured field support.

---

## 6. Change Summary

### Phase 1: Infrastructure (low risk, ~5 files)

| Change | File |
|--------|------|
| Add workspace dependencies | `Cargo.toml` |
| Add feature flags and dependencies | `rust/lancedb/Cargo.toml` |
| Create profiling module (subscriber initialization utilities) | `rust/lancedb/src/profiling.rs` (new file) |
| Export profiling module in `lib.rs` | `rust/lancedb/src/lib.rs` |
| Update Node.js bindings Cargo.toml | `nodejs/Cargo.toml` |

### Phase 2: Critical Path Instrumentation (moderate risk, ~4 files)

| Change | File |
|--------|------|
| Add `#[instrument]` to `Table` public methods | `rust/lancedb/src/table.rs` |
| Add spans to `ExecutableQuery::execute_with_options` | `rust/lancedb/src/query.rs` |
| Add tracing spans to `IoTrackingStore` methods | `rust/lancedb/src/io/object_store/io_tracking.rs` |
| Add instrument to `NativeTable` / `RemoteTable` key methods | `rust/lancedb/src/table.rs`, `rust/lancedb/src/remote/table.rs` |

### Phase 3: Node.js Bindings (low risk, ~3 files)

| Change | File |
|--------|------|
| Expose profiling initialization function | `nodejs/src/lib.rs` |
| TypeScript type definitions and wrappers | `nodejs/src/index.ts` (or new file) |
| Update `env_logger` initialization for tracing compatibility | `nodejs/src/lib.rs` |

### Expected Impact

- **New dependencies** (feature-gated): `tracing` (0.1), `tracing-subscriber`,
  `tracing-opentelemetry`, `opentelemetry` (0.28+), `opentelemetry_sdk`,
  `opentelemetry-otlp`
- **Code change volume**: ~200-300 lines added, ~50 lines modified
- **API changes**: Only new `profiling` module public API added, no breaking changes
- **Performance impact**: < 5ns/span when no subscriber is enabled; depends on sampling rate with OTLP export
- **Compilation impact**: Zero impact when features are not enabled; enabling `profiling-otlp` feature adds ~10-15s compile time (from the OpenTelemetry crate tree)
