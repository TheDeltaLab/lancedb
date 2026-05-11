// SPDX-License-Identifier: Apache-2.0
// SPDX-FileCopyrightText: Copyright The LanceDB Authors

use std::{
    fmt::{Display, Formatter},
    sync::{Arc, Mutex},
};

use bytes::Bytes;
use futures::stream::BoxStream;
use lance::io::WrappingObjectStore;
use object_store::{
    CopyOptions, GetOptions, GetResult, ListResult, MultipartUpload, ObjectMeta, ObjectStore,
    PutMultipartOptions, PutOptions, PutPayload, PutResult, RenameOptions, Result as OSResult,
    UploadPart, path::Path,
};
use tracing::Instrument;

#[derive(Debug, Default)]
pub struct IoStats {
    pub read_iops: u64,
    pub read_bytes: u64,
    pub write_iops: u64,
    pub write_bytes: u64,
}

impl Display for IoStats {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:#?}", self)
    }
}

/// OTel metric instruments for I/O tracking.
#[cfg(feature = "profiling-otlp")]
#[derive(Clone, Debug)]
struct IoMetrics {
    read_bytes: opentelemetry::metrics::Histogram<u64>,
    write_bytes: opentelemetry::metrics::Histogram<u64>,
    read_count: opentelemetry::metrics::Counter<u64>,
    write_count: opentelemetry::metrics::Counter<u64>,
    read_duration: opentelemetry::metrics::Histogram<f64>,
    write_duration: opentelemetry::metrics::Histogram<f64>,
}

#[cfg(feature = "profiling-otlp")]
impl IoMetrics {
    fn global() -> &'static Self {
        static INSTANCE: std::sync::OnceLock<IoMetrics> = std::sync::OnceLock::new();
        INSTANCE.get_or_init(Self::new)
    }

    fn new() -> Self {
        let meter = opentelemetry::global::meter("lancedb");
        Self {
            read_bytes: meter
                .u64_histogram("lancedb.io.read.bytes")
                .with_unit("By")
                .with_description("I/O read operation size distribution")
                .build(),
            write_bytes: meter
                .u64_histogram("lancedb.io.write.bytes")
                .with_unit("By")
                .with_description("I/O write operation size distribution")
                .build(),
            read_count: meter
                .u64_counter("lancedb.io.read.count")
                .with_description("Total number of read I/O operations")
                .build(),
            write_count: meter
                .u64_counter("lancedb.io.write.count")
                .with_description("Total number of write I/O operations")
                .build(),
            read_duration: meter
                .f64_histogram("lancedb.io.read.duration")
                .with_unit("ms")
                .with_description("Single read operation latency")
                .build(),
            write_duration: meter
                .f64_histogram("lancedb.io.write.duration")
                .with_unit("ms")
                .with_description("Single write operation latency")
                .build(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct IoTrackingStore {
    target: Arc<dyn ObjectStore>,
    stats: Arc<Mutex<IoStats>>,
    #[cfg(feature = "profiling-otlp")]
    metrics: Option<IoMetrics>,
}

impl Display for IoTrackingStore {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:#?}", self)
    }
}

#[derive(Debug, Default, Clone)]
pub struct IoStatsHolder(Arc<Mutex<IoStats>>);

impl IoStatsHolder {
    pub fn incremental_stats(&self) -> IoStats {
        std::mem::take(&mut self.0.lock().expect("failed to lock IoStats"))
    }
}

impl WrappingObjectStore for IoStatsHolder {
    fn wrap(&self, _store_prefix: &str, target: Arc<dyn ObjectStore>) -> Arc<dyn ObjectStore> {
        Arc::new(IoTrackingStore {
            target,
            stats: self.0.clone(),
            #[cfg(feature = "profiling-otlp")]
            metrics: Some(IoMetrics::global().clone()),
        })
    }
}

impl IoTrackingStore {
    pub fn new_wrapper() -> (Arc<dyn WrappingObjectStore>, Arc<Mutex<IoStats>>) {
        let stats = Arc::new(Mutex::new(IoStats::default()));
        (Arc::new(IoStatsHolder(stats.clone())), stats)
    }

    fn record_read(&self, num_bytes: u64, op: &'static str, duration_ms: f64) {
        let mut stats = self.stats.lock().unwrap_or_else(|e| e.into_inner());
        stats.read_iops += 1;
        stats.read_bytes += num_bytes;

        #[cfg(feature = "profiling-otlp")]
        if let Some(ref m) = self.metrics {
            let attrs = [opentelemetry::KeyValue::new("op", op)];
            m.read_bytes.record(num_bytes, &attrs);
            m.read_count.add(1, &attrs);
            m.read_duration.record(duration_ms, &attrs);
        }
        #[cfg(not(feature = "profiling-otlp"))]
        {
            let _ = (op, duration_ms);
        }
    }

    fn record_write(&self, num_bytes: u64, op: &'static str, duration_ms: f64) {
        let mut stats = self.stats.lock().unwrap_or_else(|e| e.into_inner());
        stats.write_iops += 1;
        stats.write_bytes += num_bytes;

        #[cfg(feature = "profiling-otlp")]
        if let Some(ref m) = self.metrics {
            let attrs = [opentelemetry::KeyValue::new("op", op)];
            m.write_bytes.record(num_bytes, &attrs);
            m.write_count.add(1, &attrs);
            m.write_duration.record(duration_ms, &attrs);
        }
        #[cfg(not(feature = "profiling-otlp"))]
        {
            let _ = (op, duration_ms);
        }
    }
}

#[async_trait::async_trait]
#[deny(clippy::missing_trait_methods)]
impl ObjectStore for IoTrackingStore {
    async fn put_opts(
        &self,
        location: &Path,
        bytes: PutPayload,
        opts: PutOptions,
    ) -> OSResult<PutResult> {
        let num_bytes = bytes.content_length() as u64;
        let start = std::time::Instant::now();
        let result = self
            .target
            .put_opts(location, bytes, opts)
            .instrument(tracing::info_span!("lancedb.io.put", path = %location, bytes = num_bytes))
            .await;
        self.record_write(num_bytes, "put", start.elapsed().as_secs_f64() * 1000.0);
        result
    }

    async fn put_multipart_opts(
        &self,
        location: &Path,
        opts: PutMultipartOptions,
    ) -> OSResult<Box<dyn MultipartUpload>> {
        let target = self.target.put_multipart_opts(location, opts).await?;
        Ok(Box::new(IoTrackingMultipartUpload {
            target,
            stats: self.stats.clone(),
            #[cfg(feature = "profiling-otlp")]
            metrics: self.metrics.clone(),
        }))
    }

    async fn get_opts(&self, location: &Path, options: GetOptions) -> OSResult<GetResult> {
        let start = std::time::Instant::now();
        let result = self
            .target
            .get_opts(location, options)
            .instrument(tracing::info_span!("lancedb.io.get", path = %location))
            .await;
        if let Ok(result) = &result {
            let num_bytes = result.range.end - result.range.start;
            self.record_read(num_bytes, "get", start.elapsed().as_secs_f64() * 1000.0);
        }
        result
    }

    async fn get_ranges(
        &self,
        location: &Path,
        ranges: &[std::ops::Range<u64>],
    ) -> OSResult<Vec<Bytes>> {
        let start = std::time::Instant::now();
        let result = self.target.get_ranges(location, ranges)
            .instrument(tracing::info_span!("lancedb.io.get_ranges", path = %location, num_ranges = ranges.len()))
            .await;
        if let Ok(result) = &result {
            self.record_read(
                result.iter().map(|b| b.len() as u64).sum(),
                "get_ranges",
                start.elapsed().as_secs_f64() * 1000.0,
            );
        }
        result
    }

    fn delete_stream(
        &self,
        locations: BoxStream<'static, OSResult<Path>>,
    ) -> BoxStream<'static, OSResult<Path>> {
        self.record_write(0, "delete_stream", 0.0);
        self.target.delete_stream(locations)
    }

    // list/list_with_offset return a stream whose lifetime we cannot time
    // end-to-end, so duration is recorded as 0.0.
    fn list(&self, prefix: Option<&Path>) -> BoxStream<'static, OSResult<ObjectMeta>> {
        self.record_read(0, "list", 0.0);
        self.target.list(prefix)
    }

    fn list_with_offset(
        &self,
        prefix: Option<&Path>,
        offset: &Path,
    ) -> BoxStream<'static, OSResult<ObjectMeta>> {
        // Returns a stream — duration cannot be measured, recorded as 0.0.
        self.record_read(0, "list", 0.0);
        self.target.list_with_offset(prefix, offset)
    }

    async fn list_with_delimiter(&self, prefix: Option<&Path>) -> OSResult<ListResult> {
        let start = std::time::Instant::now();
        let result = self.target.list_with_delimiter(prefix).await;
        self.record_read(0, "list", start.elapsed().as_secs_f64() * 1000.0);
        result
    }

    async fn copy_opts(&self, from: &Path, to: &Path, options: CopyOptions) -> OSResult<()> {
        let start = std::time::Instant::now();
        let result = self.target.copy_opts(from, to, options).await;
        self.record_write(0, "copy", start.elapsed().as_secs_f64() * 1000.0);
        result
    }

    async fn rename_opts(&self, from: &Path, to: &Path, options: RenameOptions) -> OSResult<()> {
        let start = std::time::Instant::now();
        let result = self.target.rename_opts(from, to, options).await;
        self.record_write(0, "rename", start.elapsed().as_secs_f64() * 1000.0);
        result
    }
}

#[derive(Debug)]
struct IoTrackingMultipartUpload {
    target: Box<dyn MultipartUpload>,
    stats: Arc<Mutex<IoStats>>,
    #[cfg(feature = "profiling-otlp")]
    metrics: Option<IoMetrics>,
}

#[async_trait::async_trait]
impl MultipartUpload for IoTrackingMultipartUpload {
    async fn abort(&mut self) -> OSResult<()> {
        self.target.abort().await
    }

    async fn complete(&mut self) -> OSResult<PutResult> {
        self.target.complete().await
    }

    fn put_part(&mut self, payload: PutPayload) -> UploadPart {
        let num_bytes = payload.content_length() as u64;
        let stats = self.stats.clone();
        #[cfg(feature = "profiling-otlp")]
        let metrics = self.metrics.clone();
        let fut = self.target.put_part(payload);
        #[cfg(feature = "profiling-otlp")]
        let start = std::time::Instant::now();
        Box::pin(async move {
            let result = fut.await;
            {
                let mut s = stats.lock().unwrap_or_else(|e| e.into_inner());
                s.write_iops += 1;
                s.write_bytes += num_bytes;
            }
            #[cfg(feature = "profiling-otlp")]
            if let Some(ref m) = metrics {
                let duration_ms = start.elapsed().as_secs_f64() * 1000.0;
                let attrs = [opentelemetry::KeyValue::new("op", "put_multipart")];
                m.write_bytes.record(num_bytes, &attrs);
                m.write_count.add(1, &attrs);
                m.write_duration.record(duration_ms, &attrs);
            }
            result
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use object_store::ObjectStoreExt;

    /// Helper: poison a Mutex<IoStats> by panicking while holding the lock.
    fn poison_stats(stats: &Arc<Mutex<IoStats>>) {
        let stats_clone = stats.clone();
        let handle = std::thread::spawn(move || {
            let _guard = stats_clone.lock().unwrap();
            panic!("intentional panic to poison stats mutex");
        });
        let _ = handle.join();
        assert!(stats.lock().is_err(), "mutex should be poisoned");
    }

    #[test]
    fn test_record_read_recovers_from_poisoned_lock() {
        let stats = Arc::new(Mutex::new(IoStats::default()));
        let store = IoTrackingStore {
            target: Arc::new(object_store::memory::InMemory::new()),
            stats: stats.clone(),
            #[cfg(feature = "profiling-otlp")]
            metrics: None,
        };

        poison_stats(&stats);

        // record_read should not panic
        store.record_read(1024, "get", 0.0);

        // Verify the stats were updated despite poisoning
        let s = stats.lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(s.read_iops, 1);
        assert_eq!(s.read_bytes, 1024);
    }

    #[test]
    fn test_record_write_recovers_from_poisoned_lock() {
        let stats = Arc::new(Mutex::new(IoStats::default()));
        let store = IoTrackingStore {
            target: Arc::new(object_store::memory::InMemory::new()),
            stats: stats.clone(),
            #[cfg(feature = "profiling-otlp")]
            metrics: None,
        };

        poison_stats(&stats);

        // record_write should not panic
        store.record_write(2048, "put", 0.0);

        let s = stats.lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(s.write_iops, 1);
        assert_eq!(s.write_bytes, 2048);
    }

    #[tokio::test]
    async fn test_io_tracking_produces_tracing_spans() {
        use std::sync::Mutex as StdMutex;
        use tracing_subscriber::layer::SubscriberExt;

        #[derive(Debug, Clone)]
        struct SpanInfo {
            name: String,
            id: u64,
            #[allow(dead_code)] // recorded for debugging parent-child relationships
            parent_id: Option<u64>,
        }
        let spans: Arc<StdMutex<Vec<SpanInfo>>> = Arc::new(StdMutex::new(Vec::new()));
        let spans_clone = spans.clone();

        struct SpanCollector(Arc<StdMutex<Vec<SpanInfo>>>);
        impl<S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>>
            tracing_subscriber::Layer<S> for SpanCollector
        {
            fn on_new_span(
                &self,
                attrs: &tracing::span::Attributes<'_>,
                id: &tracing::span::Id,
                ctx: tracing_subscriber::layer::Context<'_, S>,
            ) {
                let parent_id = attrs
                    .parent()
                    .cloned()
                    .or_else(|| ctx.current_span().id().cloned())
                    .map(|pid| pid.into_u64());
                self.0.lock().unwrap().push(SpanInfo {
                    name: attrs.metadata().name().to_string(),
                    id: id.into_u64(),
                    parent_id,
                });
            }
        }

        let subscriber = tracing_subscriber::registry().with(SpanCollector(spans_clone));
        let _guard = tracing::subscriber::set_default(subscriber);

        let store = IoTrackingStore {
            target: Arc::new(object_store::memory::InMemory::new()),
            stats: Arc::new(Mutex::new(IoStats::default())),
            #[cfg(feature = "profiling-otlp")]
            metrics: None,
        };

        // Write and read to trigger spans
        let data = PutPayload::from_static(b"hello");
        store.put(&Path::from("test.txt"), data).await.unwrap();
        let _ = store.get(&Path::from("test.txt")).await.unwrap();

        let collected = spans.lock().unwrap();
        assert!(
            collected.iter().any(|s| s.name == "lancedb.io.put"),
            "Expected 'lancedb.io.put' span, found: {:?}",
            collected.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
        assert!(
            collected.iter().any(|s| s.name == "lancedb.io.get"),
            "Expected 'lancedb.io.get' span, found: {:?}",
            collected.iter().map(|s| &s.name).collect::<Vec<_>>()
        );

        // Verify all spans have valid IDs
        for span in collected.iter() {
            assert!(span.id > 0, "Span '{}' has invalid id 0", span.name);
        }
    }
}
