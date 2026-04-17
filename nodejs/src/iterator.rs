// SPDX-License-Identifier: Apache-2.0
// SPDX-FileCopyrightText: Copyright The LanceDB Authors

use futures::StreamExt;
use lancedb::arrow::SendableRecordBatchStream;
use lancedb::ipc::batches_to_ipc_file;
use napi::bindgen_prelude::*;
use napi_derive::napi;
use tracing::Instrument;

/** Typescript-style Async Iterator over RecordBatches */
#[napi]
pub struct RecordBatchIterator {
    inner: SendableRecordBatchStream,
    /// Parent span that scopes the stream's entire lifetime.
    ///
    /// Held here (not just on the setup future) because the Rust core
    /// emits its `lancedb.query.execute` / `lancedb.native.query` spans
    /// while we poll the stream in [`Self::next`] — which happens long
    /// after the `execute_with_options` setup future resolves. By keeping
    /// the span alive in the iterator and entering it around every
    /// `.next()` call we make, those inner core spans attach to this
    /// span as their tracing-opentelemetry parent, preserving the
    /// trace tree across the JS↔Rust async boundary.
    ///
    /// Defaults to [`tracing::Span::none`] when no traceparent was
    /// passed from the JS side (no-op instrumentation).
    span: tracing::Span,
}

#[napi]
impl RecordBatchIterator {
    pub(crate) fn new_with_span(inner: SendableRecordBatchStream, span: tracing::Span) -> Self {
        Self { inner, span }
    }

    #[napi(catch_unwind)]
    pub async unsafe fn next(&mut self) -> napi::Result<Option<Buffer>> {
        let span = self.span.clone();
        async {
            if let Some(rst) = self.inner.next().await {
                let batch = rst.map_err(|e| {
                    napi::Error::from_reason(format!(
                        "Failed to get next batch from stream: {}",
                        e
                    ))
                })?;
                batches_to_ipc_file(&[batch])
                    .map_err(|e| {
                        napi::Error::from_reason(format!("Failed to write IPC file: {}", e))
                    })
                    .map(|buf| Some(Buffer::from(buf)))
            } else {
                // We are done with the stream.
                Ok(None)
            }
        }
        .instrument(span)
        .await
    }
}
