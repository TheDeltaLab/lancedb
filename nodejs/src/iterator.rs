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
        let span = crate::tracing_util::napi_span!(self.traceparent, "lancedb.napi.query.next");
        let result = async {
            if let Some(rst) = self.inner.next().await {
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
        .instrument(span)
        .await;
        result
    }
}
