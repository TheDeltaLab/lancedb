[**@thedeltalab/lancedb**](../../../README.md) • **Docs**

***

[@thedeltalab/lancedb](../../../globals.md) / [rerankers](../README.md) / RRFReranker

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

#### create(k)

```ts
static create(k?): Promise<RRFReranker>
```

Create with a specific `k` value (default `60`, `returnScore` defaults to `"relevance"`).

##### Parameters

* **k?**: `number`

##### Returns

`Promise`&lt;[`RRFReranker`](RRFReranker.md)&gt;

#### create(options)

```ts
static create(options): Promise<RRFReranker>
```

Create with an options object.

##### Parameters

* **options**: [`RRFRerankerOptions`](../interfaces/RRFRerankerOptions.md)

##### Returns

`Promise`&lt;[`RRFReranker`](RRFReranker.md)&gt;
