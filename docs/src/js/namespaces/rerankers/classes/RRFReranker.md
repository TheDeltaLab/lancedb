[**@lancedb/lancedb**](../../../README.md) • **Docs**

***

[@lancedb/lancedb](../../../globals.md) / [rerankers](../README.md) / RRFReranker

# Class: RRFReranker

Reranks the results using the Reciprocal Rank Fusion (RRF) algorithm.

## Param

Constant used in the RRF formula (default `60`). Experiments
  indicate that `k = 60` was near-optimal, but the choice is not critical.
  See paper: https://plg.uwaterloo.ca/~gvcormac/cormacksigir09-rrf.pdf

## Param

Controls which score columns appear in the output.
  - `"relevance"` (default): Only the `_relevance_score` column is kept;
    the raw `_distance` and `_score` columns are dropped.
  - `"all"`: All score columns are retained alongside `_relevance_score`,
    which is useful for debugging.

## Methods

### rerankHybrid()

```ts
rerankHybrid(
   query,
   vecResults,
   ftsResults): Promise<RecordBatch<any>>
```

#### Parameters

* **query**: `string`

* **vecResults**: `RecordBatch`&lt;`any`&gt;

* **ftsResults**: `RecordBatch`&lt;`any`&gt;

#### Returns

`Promise`&lt;`RecordBatch`&lt;`any`&gt;&gt;

***

### create()

```ts
static create(k, returnScore): Promise<RRFReranker>
```

#### Parameters

* **k**: `number` = `60`

* **returnScore**: `"all"` \| `"relevance"` = `"relevance"`

#### Returns

`Promise`&lt;[`RRFReranker`](RRFReranker.md)&gt;
