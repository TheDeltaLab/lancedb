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
    traceparent: Option<String>,
}

#[napi]
impl RecordBatchIterator {
    pub(crate) fn new(inner: SendableRecordBatchStream, traceparent: Option<String>) -> Self {
        Self { inner, traceparent }
    }

    #[napi(catch_unwind)]
    pub async unsafe fn next(&mut self) -> napi::Result<Option<Buffer>> {
        // Create a tracing span linked to the remote OTel parent so that
        // internal lance spans (next_batch_task, get_range, etc.) created
        // during stream polling are children of the original request trace
        // instead of becoming orphaned root spans.
        //
        // napi_span! attaches the remote context (ContextGuard) only within
        // its block expression to create the span with the correct parent,
        // then drops the guard immediately.  The returned tracing::Span is
        // Send-safe and is held across the await via .instrument().
        let span = crate::tracing_util::napi_span!(
            self.traceparent,
            "lancedb.napi.iterator.next"
        );

        if let Some(rst) = self.inner.next().instrument(span).await {
            let batch = rst.map_err(|e| {
                napi::Error::from_reason(format!("Failed to get next batch from stream: {}", e))
            })?;
            batches_to_ipc_file(&[batch])
                .map_err(|e| napi::Error::from_reason(format!("Failed to write IPC file: {}", e)))
                .map(|buf| Some(Buffer::from(buf)))
        } else {
            // We are done with the stream.
            Ok(None)
        }
    }
}
