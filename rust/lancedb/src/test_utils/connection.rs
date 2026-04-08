// SPDX-License-Identifier: Apache-2.0
// SPDX-FileCopyrightText: Copyright The LanceDB Authors

//! Functions for testing connections.

use crate::{Connection, connect};
use anyhow::Result;
use tempfile::{TempDir, tempdir};

pub struct TestConnection {
    pub uri: String,
    pub connection: Connection,
    _temp_dir: Option<TempDir>,
}

pub async fn new_test_connection() -> Result<TestConnection> {
    new_local_connection().await
}

async fn new_local_connection() -> Result<TestConnection> {
    let temp_dir = tempdir()?;
    let uri = temp_dir.path().to_str().unwrap();
    let connection = connect(uri).execute().await?;
    Ok(TestConnection {
        uri: uri.to_string(),
        connection,
        _temp_dir: Some(temp_dir),
    })
}
