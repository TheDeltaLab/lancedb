// SPDX-License-Identifier: Apache-2.0
// SPDX-FileCopyrightText: Copyright The LanceDB Authors

use std::collections::BTreeMap;
use std::sync::Arc;

use arrow::{
    array::downcast_array,
    compute::{concat_batches, filter_record_batch, sort_to_indices, take},
};
use arrow_array::{Array, BooleanArray, Float32Array, RecordBatch, UInt64Array};
use arrow_schema::{DataType, Field, Schema, SortOptions};
use async_trait::async_trait;
use lance::dataset::ROW_ID;

use crate::error::{Error, Result};
use crate::rerankers::{RELEVANCE_SCORE, Reranker, ReturnScore};

/// Reranks the results using Reciprocal Rank Fusion(RRF) algorithm based
/// on the scores of vector and FTS search.
///
/// # Parameters
///
/// - `k`: A constant used in the RRF formula (default `60`). Experiments
///   indicate that `k = 60` was near-optimal, but the choice is not critical.
///   See paper: <https://plg.uwaterloo.ca/~gvcormac/cormacksigir09-rrf.pdf>
/// - `return_score`: Controls which score columns appear in the output.
///   - [`ReturnScore::Relevance`] (default): preserves the original merge
///     behavior — the output contains `_relevance_score` and `_score` (FTS),
///     but not `_distance` (vector), because the merge uses the FTS schema
///     as its base.
///   - [`ReturnScore::All`]: retains every raw score column. Missing columns
///     (`_distance` for FTS rows, `_score` for vector-only rows) are filled
///     with `null` so both result sets can be concatenated.
#[derive(Debug)]
pub struct RRFReranker {
    k: f32,
    return_score: ReturnScore,
}

impl RRFReranker {
    /// Create a new [`RRFReranker`] with the given `k` value and
    /// [`ReturnScore::Relevance`] (default behavior, preserves original merge
    /// output).
    ///
    /// The parameter `k` is a constant used in the RRF formula (default is
    /// `60`). Experiments indicate that `k = 60` was near-optimal, but that
    /// the choice is not critical. See paper:
    /// <https://plg.uwaterloo.ca/~gvcormac/cormacksigir09-rrf.pdf>
    pub fn new(k: f32) -> Self {
        Self {
            k,
            return_score: ReturnScore::Relevance,
        }
    }

    /// Create a new [`RRFReranker`] specifying both `k` and `return_score`.
    ///
    /// ```
    /// # use lancedb::rerankers::rrf::RRFReranker;
    /// # use lancedb::rerankers::ReturnScore;
    /// let reranker = RRFReranker::new_with_score(60.0, ReturnScore::All);
    /// ```
    pub fn new_with_score(k: f32, return_score: ReturnScore) -> Self {
        Self { k, return_score }
    }

    /// Merge vector and FTS results keeping all score columns.
    ///
    /// For rows in **both** result sets the vector `_distance` and the FTS
    /// `_score` are both preserved.  For rows that appear in only one set the
    /// missing score column is filled with `null`:
    ///
    /// | source        | `_distance` | `_score`  |
    /// |---------------|-------------|-----------|
    /// | vector only   | value       | `null`    |
    /// | FTS only      | `null`      | value     |
    /// | both          | value       | value     |
    fn merge_results_all_scores(
        &self,
        vector_results: RecordBatch,
        fts_results: RecordBatch,
    ) -> Result<RecordBatch> {
        use arrow_array::new_null_array;
        use arrow_schema::SchemaBuilder;
        use std::collections::{HashMap, HashSet};

        // ------------------------------------------------------------------
        // 1. Build row_id -> _score map from FTS results
        // ------------------------------------------------------------------
        let fts_row_ids: UInt64Array =
            downcast_array(fts_results.column_by_name(ROW_ID).ok_or_else(|| {
                Error::InvalidInput {
                    message: format!("column '{}' missing from fts_results", ROW_ID),
                }
            })?);
        let fts_score_map: HashMap<u64, Option<f32>> = {
            let score_col = fts_results.column_by_name("_score");
            fts_row_ids
                .values()
                .iter()
                .enumerate()
                .map(|(i, &id)| {
                    let val = score_col.and_then(|col| {
                        let arr: Float32Array = downcast_array(col);
                        if arr.is_null(i) {
                            None
                        } else {
                            Some(arr.value(i))
                        }
                    });
                    (id, val)
                })
                .collect()
        };

        // ------------------------------------------------------------------
        // 2. Build updated vector_results:
        //    Add _score column, filled with the FTS score for overlapping rows
        //    and null for vector-only rows.
        // ------------------------------------------------------------------
        let vector_results = if vector_results.schema().column_with_name("_score").is_some() {
            vector_results
        } else {
            let vec_row_ids: UInt64Array =
                downcast_array(vector_results.column_by_name(ROW_ID).ok_or_else(|| {
                    Error::InvalidInput {
                        message: format!("column '{}' missing from vector_results", ROW_ID),
                    }
                })?);
            let scores: Float32Array = Float32Array::from_iter(
                vec_row_ids
                    .values()
                    .iter()
                    .map(|id| fts_score_map.get(id).copied().flatten()),
            );
            let mut builder = SchemaBuilder::from(vector_results.schema().fields());
            builder.push(Arc::new(Field::new("_score", DataType::Float32, true)));
            let new_schema = Arc::new(builder.finish());
            let mut cols = vector_results.columns().to_vec();
            cols.push(Arc::new(scores));
            RecordBatch::try_new(new_schema, cols)?
        };

        // ------------------------------------------------------------------
        // 3. FTS-only rows: rows in FTS that are not in vector results
        // ------------------------------------------------------------------
        let vec_row_ids: UInt64Array =
            downcast_array(vector_results.column_by_name(ROW_ID).unwrap());
        let vec_id_set: HashSet<u64> = vec_row_ids.values().iter().copied().collect();

        let fts_only_mask = BooleanArray::from_iter(
            fts_row_ids
                .values()
                .iter()
                .map(|id| Some(!vec_id_set.contains(id))),
        );
        let fts_only = filter_record_batch(&fts_results, &fts_only_mask)?;

        // ------------------------------------------------------------------
        // 4. Add null _distance to FTS-only rows if absent
        // ------------------------------------------------------------------
        let fts_only = if fts_only.schema().column_with_name("_distance").is_some() {
            fts_only
        } else {
            let null_dist = new_null_array(&DataType::Float32, fts_only.num_rows());
            let mut builder = SchemaBuilder::from(fts_only.schema().fields());
            builder.push(Arc::new(Field::new("_distance", DataType::Float32, true)));
            let new_schema = Arc::new(builder.finish());
            let mut cols = fts_only.columns().to_vec();
            cols.push(Arc::new(null_dist));
            RecordBatch::try_new(new_schema, cols)?
        };

        // ------------------------------------------------------------------
        // 5. Reorder FTS-only columns to match vector schema, then concat
        // ------------------------------------------------------------------
        if fts_only.num_rows() == 0 {
            return Ok(vector_results);
        }

        let vec_schema = vector_results.schema();
        let fts_reordered = {
            let indices: std::result::Result<Vec<usize>, _> = vec_schema
                .fields()
                .iter()
                .map(|f| {
                    fts_only
                        .schema()
                        .index_of(f.name())
                        .map_err(|_| Error::InvalidInput {
                            message: format!(
                                "column '{}' missing from fts_only after padding",
                                f.name()
                            ),
                        })
                })
                .collect();
            let indices = indices?;
            let cols: Vec<_> = indices
                .iter()
                .map(|&i| fts_only.column(i).clone())
                .collect();
            RecordBatch::try_new(vec_schema.clone(), cols)?
        };

        Ok(concat_batches(
            &vec_schema,
            [vector_results, fts_reordered].iter(),
        )?)
    }
}

/// Deduplicate rows in a [`RecordBatch`] by `ROW_ID`, keeping the first
/// occurrence of each id.
fn dedup_by_row_id(batch: RecordBatch) -> Result<RecordBatch> {
    use std::collections::HashSet;

    if batch.num_rows() == 0 {
        return Ok(batch);
    }

    let row_ids: UInt64Array =
        downcast_array(
            batch
                .column_by_name(ROW_ID)
                .ok_or_else(|| Error::InvalidInput {
                    message: format!("column '{}' missing from batch for deduplication", ROW_ID),
                })?,
        );
    let mut seen = HashSet::new();
    let mask = BooleanArray::from_iter(row_ids.values().iter().map(|id| Some(seen.insert(*id))));
    Ok(filter_record_batch(&batch, &mask)?)
}

impl Default for RRFReranker {
    fn default() -> Self {
        Self {
            k: 60.0,
            return_score: ReturnScore::Relevance,
        }
    }
}

#[async_trait]
impl Reranker for RRFReranker {
    async fn rerank_hybrid(
        &self,
        _query: &str,
        vector_results: RecordBatch,
        fts_results: RecordBatch,
    ) -> Result<RecordBatch> {
        // Deduplicate inputs by ROW_ID so that each row is ranked exactly once
        // and appears at most once in the merged output.
        let vector_results = dedup_by_row_id(vector_results)?;
        let fts_results = dedup_by_row_id(fts_results)?;

        let vector_ids = vector_results
            .column_by_name(ROW_ID)
            .ok_or(Error::InvalidInput {
                message: format!(
                    "expected column {} not found in vector_results. found columns {:?}",
                    ROW_ID,
                    vector_results
                        .schema()
                        .fields()
                        .iter()
                        .map(|f| f.name())
                        .collect::<Vec<_>>()
                ),
            })?;
        let fts_ids = fts_results
            .column_by_name(ROW_ID)
            .ok_or(Error::InvalidInput {
                message: format!(
                    "expected column {} not found in fts_results. found columns {:?}",
                    ROW_ID,
                    fts_results
                        .schema()
                        .fields()
                        .iter()
                        .map(|f| f.name())
                        .collect::<Vec<_>>()
                ),
            })?;

        let vector_ids: UInt64Array = downcast_array(&vector_ids);
        let fts_ids: UInt64Array = downcast_array(&fts_ids);

        let mut rrf_score_map = BTreeMap::new();
        let mut update_score_map = |(i, result_id)| {
            let score = 1.0 / (i as f32 + self.k);
            rrf_score_map
                .entry(result_id)
                .and_modify(|e| *e += score)
                .or_insert(score);
        };
        vector_ids
            .values()
            .iter()
            .enumerate()
            .for_each(&mut update_score_map);
        fts_ids
            .values()
            .iter()
            .enumerate()
            .for_each(&mut update_score_map);

        // For ReturnScore::All, merge while preserving all score columns.
        // For ReturnScore::Relevance, use the original merge (FTS schema as base).
        let combined_results = if self.return_score == ReturnScore::All {
            self.merge_results_all_scores(vector_results, fts_results)?
        } else {
            self.merge_results(vector_results, fts_results)?
        };

        let combined_row_ids: UInt64Array =
            downcast_array(combined_results.column_by_name(ROW_ID).unwrap());
        let relevance_scores = Float32Array::from_iter_values(
            combined_row_ids
                .values()
                .iter()
                .map(|row_id| rrf_score_map.get(row_id).unwrap())
                .copied(),
        );

        // keep track of indices sorted by the relevance column
        let sort_indices = sort_to_indices(
            &relevance_scores,
            Some(SortOptions {
                descending: true,
                ..Default::default()
            }),
            None,
        )
        .unwrap();

        // add relevance scores to columns
        let mut columns = combined_results.columns().to_vec();
        columns.push(Arc::new(relevance_scores));

        // sort by the relevance scores
        let columns = columns
            .iter()
            .map(|c| take(c, &sort_indices, None).unwrap())
            .collect();

        // add relevance score to schema
        let mut fields = combined_results.schema().fields().to_vec();
        fields.push(Arc::new(Field::new(
            RELEVANCE_SCORE,
            DataType::Float32,
            false,
        )));
        let schema = Schema::new(fields);

        let combined_results = RecordBatch::try_new(Arc::new(schema), columns)?;

        Ok(combined_results)
    }
}

#[cfg(test)]
pub mod test {
    use super::*;
    use arrow_array::{Float32Array, StringArray};

    #[tokio::test]
    async fn test_rrf_reranker() {
        let schema = Arc::new(Schema::new(vec![
            Field::new("name", DataType::Utf8, false),
            Field::new(ROW_ID, DataType::UInt64, false),
        ]));

        let vec_results = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(StringArray::from(vec!["foo", "bar", "baz", "bean", "dog"])),
                Arc::new(UInt64Array::from(vec![1, 4, 2, 5, 3])),
            ],
        )
        .unwrap();

        let fts_results = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(StringArray::from(vec!["bar", "bean", "dog"])),
                Arc::new(UInt64Array::from(vec![4, 5, 3])),
            ],
        )
        .unwrap();

        // scores should be calculated as:
        // - foo = 1/1        = 1.0
        // - bar = 1/2 + 1/1  = 1.5
        // - baz = 1/3        = 0.333
        // - bean = 1/4 + 1/2 = 0.75
        // - dog = 1/5 + 1/3  = 0.533
        // then we should get the result ranked in descending order

        let reranker = RRFReranker::new(1.0);

        let result = reranker
            .rerank_hybrid("", vec_results, fts_results)
            .await
            .unwrap();

        assert_eq!(3, result.schema().fields().len());
        assert_eq!("name", result.schema().fields().first().unwrap().name());
        assert_eq!(ROW_ID, result.schema().fields().get(1).unwrap().name());
        assert_eq!(
            RELEVANCE_SCORE,
            result.schema().fields().get(2).unwrap().name()
        );

        let names: StringArray = downcast_array(result.column(0));
        assert_eq!(
            names.iter().map(|e| e.unwrap()).collect::<Vec<_>>(),
            vec!["bar", "foo", "bean", "dog", "baz"]
        );

        let ids: UInt64Array = downcast_array(result.column(1));
        assert_eq!(
            ids.iter().map(|e| e.unwrap()).collect::<Vec<_>>(),
            vec![4, 1, 5, 3, 2]
        );

        let scores: Float32Array = downcast_array(result.column(2));
        assert_eq!(
            scores.iter().map(|e| e.unwrap()).collect::<Vec<_>>(),
            vec![1.5, 1.0, 0.75, 1.0 / 5.0 + 1.0 / 3.0, 1.0 / 3.0]
        );
    }

    /// `ReturnScore::Relevance` (default) preserves the original merge behavior:
    /// `_score` (FTS) is kept, `_distance` is not present (FTS schema is used as
    /// the merge base so the vector `_distance` column is dropped naturally).
    #[tokio::test]
    async fn test_rrf_return_score_relevance_preserves_original_behavior() {
        let vec_schema = Arc::new(Schema::new(vec![
            Field::new("name", DataType::Utf8, false),
            Field::new(ROW_ID, DataType::UInt64, false),
            Field::new("_distance", DataType::Float32, true),
        ]));
        let fts_schema = Arc::new(Schema::new(vec![
            Field::new("name", DataType::Utf8, false),
            Field::new(ROW_ID, DataType::UInt64, false),
            Field::new("_score", DataType::Float32, true),
        ]));

        let vec_results = RecordBatch::try_new(
            vec_schema,
            vec![
                Arc::new(StringArray::from(vec!["foo", "bar"])),
                Arc::new(UInt64Array::from(vec![1u64, 2u64])),
                Arc::new(Float32Array::from(vec![0.1f32, 0.2f32])),
            ],
        )
        .unwrap();

        let fts_results = RecordBatch::try_new(
            fts_schema,
            vec![
                Arc::new(StringArray::from(vec!["bar", "baz"])),
                Arc::new(UInt64Array::from(vec![2u64, 3u64])),
                Arc::new(Float32Array::from(vec![0.9f32, 0.8f32])),
            ],
        )
        .unwrap();

        // default constructor → ReturnScore::Relevance = no extra processing
        let reranker = RRFReranker::new(1.0);
        let result = reranker
            .rerank_hybrid("", vec_results, fts_results)
            .await
            .unwrap();

        let schema = result.schema();
        let field_names: Vec<&str> = schema.fields().iter().map(|f| f.name().as_str()).collect();

        // original behavior: _score survives, _distance is dropped by merge
        assert!(
            field_names.contains(&"_score"),
            "_score should be present: {:?}",
            field_names
        );
        assert!(
            !field_names.contains(&"_distance"),
            "_distance should not be present (original behavior): {:?}",
            field_names
        );
        assert!(
            field_names.contains(&RELEVANCE_SCORE),
            "_relevance_score should be present: {:?}",
            field_names
        );
    }

    /// `ReturnScore::All` retains both `_distance` (vector) and `_score` (FTS).
    /// For rows in both result sets, both scores must be non-null.
    /// For vector-only rows, `_score` is null; for FTS-only rows, `_distance` is null.
    #[tokio::test]
    async fn test_rrf_return_score_all_keeps_both_raw_scores() {
        let vec_schema = Arc::new(Schema::new(vec![
            Field::new("name", DataType::Utf8, false),
            Field::new(ROW_ID, DataType::UInt64, false),
            Field::new("_distance", DataType::Float32, true),
        ]));
        let fts_schema = Arc::new(Schema::new(vec![
            Field::new("name", DataType::Utf8, false),
            Field::new(ROW_ID, DataType::UInt64, false),
            Field::new("_score", DataType::Float32, true),
        ]));

        // id=1: vector-only, id=2: both, id=3: FTS-only
        let vec_results = RecordBatch::try_new(
            vec_schema,
            vec![
                Arc::new(StringArray::from(vec!["foo", "bar"])),
                Arc::new(UInt64Array::from(vec![1u64, 2u64])),
                Arc::new(Float32Array::from(vec![0.1f32, 0.2f32])),
            ],
        )
        .unwrap();

        let fts_results = RecordBatch::try_new(
            fts_schema,
            vec![
                Arc::new(StringArray::from(vec!["bar", "baz"])),
                Arc::new(UInt64Array::from(vec![2u64, 3u64])),
                Arc::new(Float32Array::from(vec![0.9f32, 0.8f32])),
            ],
        )
        .unwrap();

        let reranker = RRFReranker::new_with_score(1.0, ReturnScore::All);
        let result = reranker
            .rerank_hybrid("", vec_results, fts_results)
            .await
            .unwrap();

        let schema = result.schema();
        let field_names: Vec<&str> = schema.fields().iter().map(|f| f.name().as_str()).collect();

        assert!(field_names.contains(&"_distance"), "{:?}", field_names);
        assert!(field_names.contains(&"_score"), "{:?}", field_names);
        assert!(field_names.contains(&RELEVANCE_SCORE), "{:?}", field_names);

        // Verify per-row score values:
        // result is sorted by _relevance_score descending.
        // id=2 appears in both  → _distance=0.2, _score=0.9
        // id=1 vector-only      → _distance=0.1, _score=null
        // id=3 FTS-only         → _distance=null, _score=0.8
        let row_ids: UInt64Array = downcast_array(result.column_by_name(ROW_ID).unwrap());
        let distances: Float32Array = downcast_array(result.column_by_name("_distance").unwrap());
        let scores: Float32Array = downcast_array(result.column_by_name("_score").unwrap());

        let rows: Vec<(u64, Option<f32>, Option<f32>)> = (0..result.num_rows())
            .map(|i| {
                (
                    row_ids.value(i),
                    if distances.is_null(i) {
                        None
                    } else {
                        Some(distances.value(i))
                    },
                    if scores.is_null(i) {
                        None
                    } else {
                        Some(scores.value(i))
                    },
                )
            })
            .collect();

        // id=2 must have both scores
        let row2 = rows.iter().find(|(id, _, _)| *id == 2).unwrap();
        assert!(row2.1.is_some(), "id=2 _distance should be non-null");
        assert!(
            row2.2.is_some(),
            "id=2 _score should be non-null (was in FTS)"
        );

        // id=1 (vector-only) must have _distance, no _score
        let row1 = rows.iter().find(|(id, _, _)| *id == 1).unwrap();
        assert!(row1.1.is_some(), "id=1 _distance should be non-null");
        assert!(row1.2.is_none(), "id=1 _score should be null (vector-only)");

        // id=3 (FTS-only) must have _score, no _distance
        let row3 = rows.iter().find(|(id, _, _)| *id == 3).unwrap();
        assert!(row3.1.is_none(), "id=3 _distance should be null (FTS-only)");
        assert!(row3.2.is_some(), "id=3 _score should be non-null");
    }

    /// Duplicate ROW_IDs in vector results must be deduplicated so that each
    /// row appears exactly once in the output and is ranked only once.
    #[tokio::test]
    async fn test_rrf_deduplicates_vector_results() {
        let vec_schema = Arc::new(Schema::new(vec![
            Field::new("name", DataType::Utf8, false),
            Field::new(ROW_ID, DataType::UInt64, false),
            Field::new("_distance", DataType::Float32, true),
        ]));
        let fts_schema = Arc::new(Schema::new(vec![
            Field::new("name", DataType::Utf8, false),
            Field::new(ROW_ID, DataType::UInt64, false),
            Field::new("_score", DataType::Float32, true),
        ]));

        // id=1 appears twice in vector results
        let vec_results = RecordBatch::try_new(
            vec_schema,
            vec![
                Arc::new(StringArray::from(vec!["foo", "bar", "foo"])),
                Arc::new(UInt64Array::from(vec![1u64, 2u64, 1u64])),
                Arc::new(Float32Array::from(vec![0.1f32, 0.2f32, 0.3f32])),
            ],
        )
        .unwrap();

        let fts_results = RecordBatch::try_new(
            fts_schema,
            vec![
                Arc::new(StringArray::from(vec!["bar"])),
                Arc::new(UInt64Array::from(vec![2u64])),
                Arc::new(Float32Array::from(vec![0.9f32])),
            ],
        )
        .unwrap();

        let reranker = RRFReranker::new_with_score(1.0, ReturnScore::All);
        let result = reranker
            .rerank_hybrid("", vec_results, fts_results)
            .await
            .unwrap();

        let row_ids: UInt64Array = downcast_array(result.column_by_name(ROW_ID).unwrap());
        let ids: Vec<u64> = row_ids.values().to_vec();

        // Each id must appear exactly once
        assert_eq!(ids.len(), 2, "expected 2 unique rows, got {:?}", ids);
        assert!(ids.contains(&1), "id=1 should be present");
        assert!(ids.contains(&2), "id=2 should be present");
    }
}
