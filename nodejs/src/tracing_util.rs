// SPDX-License-Identifier: Apache-2.0
// SPDX-FileCopyrightText: Copyright The LanceDB Authors

/// Attach a remote OTel context parsed from a W3C `traceparent` header.
///
/// Returns a [`ContextGuard`] that keeps the remote context active on the
/// current thread's OTel context stack.  Any `tracing` span created while
/// the guard is alive will inherit the remote trace-id and parent span-id,
/// because [`tracing_opentelemetry::OpenTelemetryLayer::on_new_span`] picks
/// up the parent via [`opentelemetry::Context::current()`].
///
/// The expected format is: `00-{trace_id}-{span_id}-{trace_flags}`
///
/// Returns `None` if the string is malformed.
///
/// # Why not `set_parent`?
///
/// [`tracing_opentelemetry::OpenTelemetrySpanExt::set_parent`] relies on
/// `downcast_ref::<WithContext>()` through the subscriber.  When the
/// subscriber uses a [`tracing_subscriber::reload::Layer`] (as our
/// `profiling::init_subscriber` does), `reload::Layer::downcast_raw`
/// returns `None` for all types except `NoneLayerMarker`, so `set_parent`
/// silently does nothing.  By instead pushing the remote context onto the
/// OTel context stack *before* the tracing span is created, `on_new_span`
/// reads it directly from `Context::current()`, bypassing the downcast
/// entirely.
#[cfg(feature = "profiling-otlp")]
pub fn attach_remote_context(traceparent: &str) -> Option<opentelemetry::ContextGuard> {
    use opentelemetry::Context;
    use opentelemetry::trace::{
        SpanContext, SpanId, TraceContextExt, TraceFlags, TraceId, TraceState,
    };

    let parts: Vec<&str> = traceparent.split('-').collect();
    if parts.len() != 4 {
        return None;
    }

    let trace_id = TraceId::from_hex(parts[1]).ok()?;
    let span_id = SpanId::from_hex(parts[2]).ok()?;
    let flags = u8::from_str_radix(parts[3], 16).unwrap_or(1);

    let span_context = SpanContext::new(
        trace_id,
        span_id,
        TraceFlags::new(flags),
        true, // is_remote
        TraceState::NONE,
    );

    let otel_ctx = Context::current().with_remote_span_context(span_context);
    Some(otel_ctx.attach())
}

/// Create a tracing span with optional traceparent propagation.
///
/// When `profiling-otlp` is enabled, this attaches the remote OTel context
/// (if any) before creating the span so it inherits the remote trace.
/// When the feature is disabled, the traceparent is silently ignored.
macro_rules! napi_span {
    ($traceparent:expr, $span_name:expr) => {{
        #[cfg(feature = "profiling-otlp")]
        let _otel_guard = $traceparent
            .as_deref()
            .and_then(crate::tracing_util::attach_remote_context);
        #[cfg(not(feature = "profiling-otlp"))]
        let _ = $traceparent;
        tracing::info_span!($span_name)
    }};
}
pub(crate) use napi_span;
