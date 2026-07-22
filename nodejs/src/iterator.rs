// SPDX-License-Identifier: Apache-2.0
// SPDX-FileCopyrightText: Copyright The LanceDB Authors

use futures::StreamExt;
use lancedb::arrow::SendableRecordBatchStream;
use lancedb::ipc::batches_to_ipc_file;
use napi::bindgen_prelude::*;
use napi_derive::napi;
use tracing::{Instrument, Span};

/** Typescript-style Async Iterator over RecordBatches */
#[napi]
pub struct RecordBatchIterator {
    inner: SendableRecordBatchStream,
    span: Span,
}

#[napi]
impl RecordBatchIterator {
    pub(crate) fn new(inner: SendableRecordBatchStream, span: Span) -> Self {
        Self { inner, span }
    }

    #[napi(catch_unwind)]
    pub async unsafe fn next(&mut self) -> napi::Result<Option<Buffer>> {
        // Re-enter the same stream span for every poll so downstream Lance tasks
        // always capture a stable local parent across the whole iterator lifetime.
        // A fresh child span per next() call can leave buffered work hanging off a
        // short-lived parent once JavaScript yields between batches.
        if let Some(rst) = self.inner.next().instrument(self.span.clone()).await {
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
