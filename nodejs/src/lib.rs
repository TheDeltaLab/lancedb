// SPDX-License-Identifier: Apache-2.0
// SPDX-FileCopyrightText: Copyright The LanceDB Authors

use std::collections::HashMap;
use std::sync::OnceLock;

use napi_derive::*;

mod connection;
mod error;
mod index;
mod iterator;
pub mod merge;
pub mod otel;
pub mod permutation;
mod query;
mod rerankers;
mod scannable;
mod session;
mod table;
mod tracing_util;
mod util;

/// Stores the reload handle from init_subscriber() so that initProfiling()
/// can hot-swap the tracing layers later.
#[cfg(any(feature = "profiling", feature = "profiling-otlp"))]
static RELOAD_HANDLE: OnceLock<lancedb::profiling::ReloadHandle> = OnceLock::new();

#[napi(object)]
#[derive(Debug)]
pub struct ConnectionOptions {
    /// The interval, in seconds, at which to check for updates to the table
    /// from other processes. If None, then consistency is not checked. For
    /// performance reasons, this is the default. For strong consistency, set
    /// this to zero seconds. Then every read will check for updates from other
    /// processes. As a compromise, you can set this to a non-zero value for
    /// eventual consistency. If more than that interval has passed since the
    /// last check, then the table will be checked for updates. Note: this
    /// consistency only applies to read operations. Write operations are
    /// always consistent.
    ///
    /// Stronger consistency is not free. The smaller the interval, the more
    /// often each read pays the cost of checking for updates against object
    /// storage, raising per-read latency and cost.
    pub read_consistency_interval: Option<f64>,
    /// Configuration for object storage.
    ///
    /// The available options are described at https://docs.lancedb.com/storage/
    pub storage_options: Option<HashMap<String, String>>,
    /// (For LanceDB OSS only): use directory namespace manifests as the source
    /// of truth for table metadata. Existing directory-listed root tables are
    /// migrated into the manifest on access.
    pub manifest_enabled: Option<bool>,
    /// (For LanceDB OSS only): extra properties for the backing namespace
    /// client used by manifest-enabled native connections.
    pub namespace_client_properties: Option<HashMap<String, String>>,
    /// (For LanceDB OSS only): the session to use for this connection. Holds
    /// shared caches and other session-specific state.
    pub session: Option<session::Session>,
}

#[napi(object)]
pub struct OpenTableOptions {
    pub storage_options: Option<HashMap<String, String>>,
}

#[napi(object)]
#[derive(Debug)]
pub struct ConnectNamespaceOptions {
    /// The interval, in seconds, at which to check for updates to the table
    /// from other processes. If None, then consistency is not checked. For
    /// performance reasons, this is the default. For strong consistency, set
    /// this to zero seconds. Then every read will check for updates from other
    /// processes. As a compromise, you can set this to a non-zero value for
    /// eventual consistency.
    pub read_consistency_interval: Option<f64>,
    /// Configuration for object storage. The available options are described
    /// at https://docs.lancedb.com/storage/
    pub storage_options: Option<HashMap<String, String>>,
    /// Extra properties for the backing namespace client.
    pub namespace_client_properties: Option<HashMap<String, String>>,
    /// The session to use for this connection. Holds shared caches and other
    /// session-specific state.
    pub session: Option<session::Session>,
}

#[napi_derive::module_init]
fn init() {
    #[cfg(any(feature = "profiling", feature = "profiling-otlp"))]
    {
        let handle = lancedb::profiling::init_subscriber();
        let _ = RELOAD_HANDLE.set(handle);
    }

    #[cfg(not(any(feature = "profiling", feature = "profiling-otlp")))]
    {
        use tracing_subscriber::EnvFilter;
        let filter =
            EnvFilter::try_from_env("LANCEDB_LOG").unwrap_or_else(|_| EnvFilter::new("warn"));
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .try_init()
            .ok();
    }
}

/// Options for configuring OTLP profiling export.
#[napi(object)]
pub struct ProfilingOptions {
    /// The OTLP collector endpoint (e.g. "http://localhost:4318").
    /// Falls back to OTEL_EXPORTER_OTLP_ENDPOINT env var.
    pub otlp_endpoint: Option<String>,
    /// The service name for telemetry (defaults to "lancedb").
    /// Falls back to OTEL_SERVICE_NAME env var.
    pub service_name: Option<String>,
    /// Transport protocol: "http" (default) or "grpc".
    pub protocol: Option<String>,
}

/// Initialize OTLP profiling for traces, metrics, and logs.
///
/// This hot-swaps the tracing subscriber's inner layers to export telemetry
/// data to an OTLP-compatible collector (e.g. Grafana Alloy, OpenTelemetry
/// Collector).
///
/// Can be called at any time — the global subscriber installed at module
/// load supports hot-reloading, so no "must call before any LanceDB
/// operations" constraint.
///
/// Requires the `profiling-otlp` feature to be enabled at build time.
#[napi]
pub fn init_profiling(options: Option<ProfilingOptions>) -> napi::Result<()> {
    #[cfg(feature = "profiling-otlp")]
    {
        let handle = RELOAD_HANDLE
            .get()
            .ok_or_else(|| napi::Error::from_reason("Tracing subscriber not initialized"))?;

        let mut config = lancedb::profiling::OtlpConfig::default();
        if let Some(opts) = options {
            if let Some(endpoint) = opts.otlp_endpoint {
                config.endpoint = endpoint;
            }
            if let Some(name) = opts.service_name {
                config.service_name = name;
            }
            if let Some(proto) = opts.protocol {
                config.protocol = match proto.as_str() {
                    "grpc" => lancedb::profiling::OtlpProtocol::Grpc,
                    _ => lancedb::profiling::OtlpProtocol::Http,
                };
            }
        }

        // Store the guard in a static to prevent shutdown on drop.
        // If already initialized, return early (idempotent).
        static GUARD: OnceLock<lancedb::profiling::OtlpGuard> = OnceLock::new();
        if GUARD.get().is_some() {
            return Ok(());
        }
        let guard = lancedb::profiling::upgrade_to_otlp(handle, config)
            .map_err(|e| napi::Error::from_reason(format!("Failed to init OTLP profiling: {e}")))?;
        let _ = GUARD.set(guard);
        Ok(())
    }

    #[cfg(not(feature = "profiling-otlp"))]
    {
        let _ = options;
        Err(napi::Error::from_reason(
            "OTLP profiling is not enabled. Rebuild with the 'profiling-otlp' feature flag."
                .to_string(),
        ))
    }
}
