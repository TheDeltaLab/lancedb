// SPDX-License-Identifier: Apache-2.0
// SPDX-FileCopyrightText: Copyright The LanceDB Authors

//! Performance profiling and observability support.
//!
//! This module provides helpers to initialize [`tracing`] subscribers for
//! structured logging and, optionally, OpenTelemetry export of traces, metrics,
//! and logs.
//!
//! # Feature flags
//!
//! * `profiling` — enables local fmt/json subscriber helpers.
//! * `profiling-otlp` — enables full OTLP export (traces + metrics + logs).
//!
//! # Hot-reload pattern
//!
//! Use [`init_subscriber`] to install a global subscriber with hot-reload
//! support, then call [`upgrade_to_otlp`] later to add OTLP export without
//! replacing the global subscriber.
//!
//! ```ignore
//! use lancedb::profiling::{init_subscriber, upgrade_to_otlp, OtlpConfig};
//!
//! let handle = init_subscriber();
//! // ... later, when ready to enable OTLP:
//! let guard = upgrade_to_otlp(&handle, OtlpConfig::default()).unwrap();
//! ```

use tracing::level_filters::LevelFilter;
use tracing_subscriber::layer::Context;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{Layer, Registry, reload};

type BoxedLayer = Box<dyn Layer<Registry> + Send + Sync>;

/// A composite layer that delegates to multiple inner boxed layers and
/// applies a [`LevelFilter`] to gate which events/spans are processed.
///
/// This avoids using `tracing_subscriber::filter::Filtered` (which requires
/// a `FilterId` assigned during subscriber registration and is incompatible
/// with [`reload::Layer`] hot-swapping).
struct MultiLayer {
    level: LevelFilter,
    layers: Vec<BoxedLayer>,
}

impl Layer<Registry> for MultiLayer {
    fn enabled(&self, metadata: &tracing::Metadata<'_>, _ctx: Context<'_, Registry>) -> bool {
        metadata.level() <= &self.level
    }

    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        id: &tracing::span::Id,
        ctx: Context<'_, Registry>,
    ) {
        for layer in &self.layers {
            layer.on_new_span(attrs, id, ctx.clone());
        }
    }

    fn on_record(
        &self,
        span: &tracing::span::Id,
        values: &tracing::span::Record<'_>,
        ctx: Context<'_, Registry>,
    ) {
        for layer in &self.layers {
            layer.on_record(span, values, ctx.clone());
        }
    }

    fn on_follows_from(
        &self,
        span: &tracing::span::Id,
        follows: &tracing::span::Id,
        ctx: Context<'_, Registry>,
    ) {
        for layer in &self.layers {
            layer.on_follows_from(span, follows, ctx.clone());
        }
    }

    fn on_event(&self, event: &tracing::Event<'_>, ctx: Context<'_, Registry>) {
        for layer in &self.layers {
            layer.on_event(event, ctx.clone());
        }
    }

    fn on_enter(&self, id: &tracing::span::Id, ctx: Context<'_, Registry>) {
        for layer in &self.layers {
            layer.on_enter(id, ctx.clone());
        }
    }

    fn on_exit(&self, id: &tracing::span::Id, ctx: Context<'_, Registry>) {
        for layer in &self.layers {
            layer.on_exit(id, ctx.clone());
        }
    }

    fn on_close(&self, id: tracing::span::Id, ctx: Context<'_, Registry>) {
        for layer in &self.layers {
            layer.on_close(id.clone(), ctx.clone());
        }
    }

    #[doc(hidden)]
    unsafe fn downcast_raw(&self, id: std::any::TypeId) -> Option<*const ()> {
        if id == std::any::TypeId::of::<Self>() {
            return Some(self as *const _ as *const ());
        }
        // SAFETY: The inner layers live as long as this `MultiLayer`. When the
        // `reload::Layer` hot-swaps the `MultiLayer`, the old instance is kept
        // alive (behind an `Arc` inside `reload`) until all outstanding span
        // references are dropped, so pointers returned here remain valid for
        // the lifetime of any span that holds them.
        //
        // Forwarding is necessary so that `tracing_opentelemetry::WithContext`
        // can be found by `set_parent()` through the reload boundary.
        self.layers
            .iter()
            .find_map(|l| unsafe { l.downcast_raw(id) })
    }
}

/// Parse `LANCEDB_LOG` env var as a [`LevelFilter`].
///
/// Supports simple level names: "trace", "debug", "info", "warn", "error",
/// "off". For advanced per-module filtering (e.g. `lancedb=debug,lance=info`),
/// use [`init_fmt_subscriber`] which accepts the full [`EnvFilter`] syntax.
///
/// Defaults to [`LevelFilter::WARN`] if unset or unparseable.
fn level_from_env() -> LevelFilter {
    std::env::var("LANCEDB_LOG")
        .ok()
        .and_then(|s| s.parse::<LevelFilter>().ok())
        .unwrap_or(LevelFilter::WARN)
}

/// Handle for hot-reloading the tracing subscriber's inner layers.
///
/// Returned by [`init_subscriber`]. Pass to [`upgrade_to_otlp`] to add
/// OTLP export without replacing the global subscriber.
pub struct ReloadHandle(reload::Handle<MultiLayer, Registry>);

/// Install a global tracing subscriber with hot-reload support.
///
/// The initial subscriber uses a fmt layer filtered by the `LANCEDB_LOG`
/// environment variable (defaults to `warn`). Returns a [`ReloadHandle`]
/// that can later be passed to [`upgrade_to_otlp`] to add OTLP export.
///
/// This function should be called exactly once, typically at process
/// startup or module init.
pub fn init_subscriber() -> ReloadHandle {
    let level = level_from_env();
    let fmt_layer: BoxedLayer = Box::new(tracing_subscriber::fmt::layer());
    let initial = MultiLayer {
        level,
        layers: vec![fmt_layer],
    };
    let (reload_layer, handle) = reload::Layer::new(initial);
    tracing_subscriber::registry()
        .with(reload_layer)
        .try_init()
        .ok(); // If another subscriber is already set, silently ignore
    ReloadHandle(handle)
}

/// Initialize a human-readable fmt subscriber.
///
/// The filter is controlled by the `LANCEDB_LOG` environment variable
/// (defaults to `warn`).
pub fn init_fmt_subscriber() {
    let filter = tracing_subscriber::EnvFilter::try_from_env("LANCEDB_LOG")
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .try_init()
        .ok();
}

/// Initialize a JSON-formatted subscriber.
///
/// The filter is controlled by the `LANCEDB_LOG` environment variable
/// (defaults to `warn`).
pub fn init_json_subscriber() {
    let filter = tracing_subscriber::EnvFilter::try_from_env("LANCEDB_LOG")
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn"));
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(filter)
        .try_init()
        .ok();
}

/// OTLP transport protocol.
#[cfg(feature = "profiling-otlp")]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OtlpProtocol {
    /// gRPC (port 4317 by convention)
    Grpc,
    /// HTTP/protobuf (port 4318 by convention)
    #[default]
    Http,
}

/// Configuration for OTLP export.
#[cfg(feature = "profiling-otlp")]
#[derive(Debug, Clone)]
pub struct OtlpConfig {
    /// OTLP collector endpoint.
    ///
    /// Falls back to `OTEL_EXPORTER_OTLP_ENDPOINT` env var, then
    /// `http://localhost:4318`.
    pub endpoint: String,
    /// The `service.name` resource attribute.
    ///
    /// Falls back to `OTEL_SERVICE_NAME` env var, then `lancedb`.
    pub service_name: String,
    /// Transport protocol.
    pub protocol: OtlpProtocol,
}

#[cfg(feature = "profiling-otlp")]
impl Default for OtlpConfig {
    fn default() -> Self {
        let endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
            .unwrap_or_else(|_| "http://localhost:4318".to_string());
        let service_name =
            std::env::var("OTEL_SERVICE_NAME").unwrap_or_else(|_| "lancedb".to_string());
        Self {
            endpoint,
            service_name,
            protocol: OtlpProtocol::default(),
        }
    }
}

/// Guard returned by [`upgrade_to_otlp`] / [`init_otlp`].
///
/// Dropping this guard will flush and shut down all OTLP exporters.
/// For explicit control, call [`OtlpGuard::shutdown`] before process exit.
#[cfg(feature = "profiling-otlp")]
pub struct OtlpGuard {
    tracer_provider: Option<opentelemetry_sdk::trace::SdkTracerProvider>,
    meter_provider: Option<opentelemetry_sdk::metrics::SdkMeterProvider>,
    log_provider: Option<opentelemetry_sdk::logs::SdkLoggerProvider>,
}

#[cfg(feature = "profiling-otlp")]
impl OtlpGuard {
    fn shutdown_inner(&mut self) {
        if let Some(tp) = self.tracer_provider.take()
            && let Err(e) = tp.shutdown()
        {
            eprintln!("lancedb profiling: failed to shutdown tracer provider: {e}");
        }
        if let Some(mp) = self.meter_provider.take()
            && let Err(e) = mp.shutdown()
        {
            eprintln!("lancedb profiling: failed to shutdown meter provider: {e}");
        }
        if let Some(lp) = self.log_provider.take()
            && let Err(e) = lp.shutdown()
        {
            eprintln!("lancedb profiling: failed to shutdown log provider: {e}");
        }
    }

    /// Flush and shut down all OTLP exporters.
    pub fn shutdown(mut self) {
        self.shutdown_inner();
    }
}

#[cfg(feature = "profiling-otlp")]
impl Drop for OtlpGuard {
    fn drop(&mut self) {
        self.shutdown_inner();
    }
}

/// Upgrade the subscriber to include OTLP export of traces, metrics, and logs.
///
/// This hot-swaps the inner layers of the subscriber installed by
/// [`init_subscriber`], without replacing the global subscriber. The level
/// filter is changed to [`LevelFilter::TRACE`] so that all spans and events
/// are captured by the OTLP exporters.
///
/// OTel providers (tracer, meter, logger) are registered globally via
/// [`opentelemetry::global`].
///
/// Returns an [`OtlpGuard`] whose [`shutdown`](OtlpGuard::shutdown) method
/// should be called before exit to flush pending data.
#[cfg(feature = "profiling-otlp")]
pub fn upgrade_to_otlp(
    handle: &ReloadHandle,
    config: OtlpConfig,
) -> Result<OtlpGuard, Box<dyn std::error::Error>> {
    use opentelemetry::KeyValue;
    use opentelemetry::trace::TracerProvider;
    use opentelemetry_otlp::WithExportConfig;
    use opentelemetry_sdk::Resource;

    let resource = Resource::builder()
        .with_attributes([KeyValue::new(
            opentelemetry::Key::new("service.name"),
            opentelemetry::Value::from(config.service_name.clone()),
        )])
        .build();

    let base = config.endpoint.trim_end_matches('/');

    // --- Traces ---
    let trace_exporter = match config.protocol {
        OtlpProtocol::Http => opentelemetry_otlp::SpanExporter::builder()
            .with_http()
            .with_endpoint(format!("{base}/v1/traces"))
            .build()?,
        OtlpProtocol::Grpc => opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .with_endpoint(base)
            .build()?,
    };
    let tracer_provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
        .with_resource(resource.clone())
        .with_batch_exporter(trace_exporter)
        .build();
    let tracer = tracer_provider.tracer("lancedb");
    let _prev = opentelemetry::global::set_tracer_provider(tracer_provider.clone());
    let traces_layer: BoxedLayer = Box::new(tracing_opentelemetry::layer().with_tracer(tracer));

    // --- Metrics ---
    let metrics_exporter = match config.protocol {
        OtlpProtocol::Http => opentelemetry_otlp::MetricExporter::builder()
            .with_http()
            .with_endpoint(format!("{base}/v1/metrics"))
            .build()?,
        OtlpProtocol::Grpc => opentelemetry_otlp::MetricExporter::builder()
            .with_tonic()
            .with_endpoint(base)
            .build()?,
    };
    let meter_provider = opentelemetry_sdk::metrics::SdkMeterProvider::builder()
        .with_resource(resource.clone())
        .with_periodic_exporter(metrics_exporter)
        .build();
    opentelemetry::global::set_meter_provider(meter_provider.clone());

    // --- Logs ---
    let log_exporter = match config.protocol {
        OtlpProtocol::Http => opentelemetry_otlp::LogExporter::builder()
            .with_http()
            .with_endpoint(format!("{base}/v1/logs"))
            .build()?,
        OtlpProtocol::Grpc => opentelemetry_otlp::LogExporter::builder()
            .with_tonic()
            .with_endpoint(base)
            .build()?,
    };
    let log_provider = opentelemetry_sdk::logs::SdkLoggerProvider::builder()
        .with_resource(resource)
        .with_batch_exporter(log_exporter)
        .build();
    let logs_layer: BoxedLayer = Box::new(
        opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge::new(&log_provider),
    );

    // --- Assemble new layer stack ---
    // Use TRACE level so all spans/events reach the OTLP exporters.
    // The fmt layer (console output) keeps its own level filter so it is not
    // flooded by TRACE-level events from the OTLP pipeline.
    let mut layers: Vec<BoxedLayer> = vec![traces_layer, logs_layer];
    if std::env::var("LANCEDB_LOG").is_ok() {
        let fmt_level = level_from_env();
        layers.push(Box::new(
            tracing_subscriber::fmt::layer().with_filter(fmt_level),
        ));
    }

    let upgraded = MultiLayer {
        level: LevelFilter::TRACE,
        layers,
    };

    // --- Hot-swap ---
    handle
        .0
        .reload(upgraded)
        .map_err(|e| format!("failed to reload tracing layers: {e}"))?;

    Ok(OtlpGuard {
        tracer_provider: Some(tracer_provider),
        meter_provider: Some(meter_provider),
        log_provider: Some(log_provider),
    })
}

/// Initialize the three-signal OTLP pipeline (traces + metrics + logs).
///
/// This is a convenience function that combines [`init_subscriber`] and
/// [`upgrade_to_otlp`] in one call. Prefer the two-step pattern when you
/// need the subscriber to be available before OTLP is configured (e.g.
/// in Node.js module init).
///
/// Returns an [`OtlpGuard`] whose [`shutdown`](OtlpGuard::shutdown) method
/// must be called before exit to flush pending data.
#[cfg(feature = "profiling-otlp")]
pub fn init_otlp(config: OtlpConfig) -> Result<OtlpGuard, Box<dyn std::error::Error>> {
    let handle = init_subscriber();
    upgrade_to_otlp(&handle, config)
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_init_fmt_subscriber_does_not_panic() {
        super::init_fmt_subscriber();
    }

    #[test]
    fn test_init_json_subscriber_does_not_panic() {
        super::init_json_subscriber();
    }

    #[test]
    fn test_init_subscriber_returns_handle() {
        let _handle = super::init_subscriber();
    }

    #[cfg(feature = "profiling-otlp")]
    #[test]
    fn test_otlp_config_default() {
        let config = super::OtlpConfig::default();
        assert!(!config.endpoint.is_empty());
        assert!(!config.service_name.is_empty());
        assert_eq!(config.protocol, super::OtlpProtocol::Http);
    }
}
