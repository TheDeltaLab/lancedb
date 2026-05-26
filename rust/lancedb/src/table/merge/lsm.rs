// SPDX-License-Identifier: Apache-2.0
// SPDX-FileCopyrightText: Copyright The LanceDB Authors

//! MemWAL LSM write-path spec management.
//!
//! [`set_lsm_write_spec`] installs a [`super::super::LsmWriteSpec`] on a
//! table, which selects Lance's MemWAL LSM-style write path for future
//! `merge_insert` calls. [`unset_lsm_write_spec`] removes it. The actual
//! `merge_insert` dispatch and writer are a follow-up.

use std::collections::HashMap;

use lance::dataset::mem_wal::{DatasetMemWalExt, MemWalConfig, MemWalShardConfig};
use lance::index::DatasetIndexExt;
use lance_index::mem_wal::{ShardField, ShardSpec};

use crate::error::{Error, Result};
use crate::table::{LsmWriteSpec, NativeTable};

// =============================================================================
// set_lsm_write_spec
// =============================================================================

/// Install an [`LsmWriteSpec`] on the table.
///
/// The sharding spec is translated into a [`ShardSpec`] stored in the MemWAL
/// index metadata via [`MemWalConfig`].
#[allow(clippy::redundant_pub_crate)]
pub(crate) async fn set_lsm_write_spec(table: &NativeTable, spec: LsmWriteSpec) -> Result<()> {
    table.dataset.ensure_mutable()?;

    {
        let dataset = table.dataset.get().await?;
        if dataset.mem_wal_index_details().await?.is_some() {
            return Err(Error::InvalidInput {
                message: "set_lsm_write_spec: an LSM write spec is already set on this table; mutation is not supported".into(),
            });
        }
    }

    let (shard_spec, maintained_indexes, writer_config_defaults, shard_config) = match spec {
        LsmWriteSpec::Bucket {
            column,
            num_buckets,
            maintained_indexes,
            writer_config_defaults,
        } => {
            let mut parameters = HashMap::new();
            parameters.insert("num_buckets".to_string(), num_buckets.to_string());
            let field = ShardField {
                field_id: column,
                source_ids: Vec::new(),
                transform: Some("bucket".to_string()),
                expression: None,
                result_type: "uint32".to_string(),
                parameters,
            };
            (
                Some(ShardSpec {
                    spec_id: 0,
                    fields: vec![field],
                }),
                maintained_indexes,
                writer_config_defaults,
                MemWalShardConfig {
                    num_shards: num_buckets,
                },
            )
        }
        LsmWriteSpec::Identity {
            column,
            maintained_indexes,
            writer_config_defaults,
        } => {
            let field = ShardField {
                field_id: column,
                source_ids: Vec::new(),
                transform: Some("identity".to_string()),
                expression: None,
                result_type: "string".to_string(),
                parameters: HashMap::new(),
            };
            (
                Some(ShardSpec {
                    spec_id: 0,
                    fields: vec![field],
                }),
                maintained_indexes,
                writer_config_defaults,
                MemWalShardConfig::default(),
            )
        }
        LsmWriteSpec::Unsharded {
            maintained_indexes,
            writer_config_defaults,
        } => {
            let field = ShardField {
                field_id: String::new(),
                source_ids: Vec::new(),
                transform: Some("unsharded".to_string()),
                expression: None,
                result_type: String::new(),
                parameters: HashMap::new(),
            };
            (
                Some(ShardSpec {
                    spec_id: 0,
                    fields: vec![field],
                }),
                maintained_indexes,
                writer_config_defaults,
                MemWalShardConfig { num_shards: 1 },
            )
        }
    };

    if !writer_config_defaults.is_empty() {
        tracing::warn!(
            "writer_config_defaults are not yet supported by this lance version and will be ignored"
        );
    }

    let config = MemWalConfig {
        shard_spec,
        maintained_indexes,
    };

    let mut dataset = (*table.dataset.get().await?).clone();
    dataset
        .initialize_mem_wal_with_shards(config, shard_config)
        .await?;
    table.dataset.update(dataset);
    Ok(())
}

// =============================================================================
// unset_lsm_write_spec
// =============================================================================

/// Remove the [`LsmWriteSpec`] from the table by dropping the MemWAL index.
///
/// Errors if no spec is currently set.
#[allow(clippy::redundant_pub_crate)]
pub(crate) async fn unset_lsm_write_spec(table: &NativeTable) -> Result<()> {
    table.dataset.ensure_mutable()?;

    {
        let dataset = table.dataset.get().await?;
        if dataset.mem_wal_index_details().await?.is_none() {
            return Err(Error::InvalidInput {
                message: "unset_lsm_write_spec: no LSM write spec is set on this table".into(),
            });
        }
    }

    let mut dataset = (*table.dataset.get().await?).clone();
    dataset
        .drop_index(lance_index::mem_wal::MEM_WAL_INDEX_NAME)
        .await?;
    table.dataset.update(dataset);
    Ok(())
}
