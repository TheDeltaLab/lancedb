[**@delta-ai/lancedb**](../../../README.md) • **Docs**

***

[@delta-ai/lancedb](../../../globals.md) / [rerankers](../README.md) / RRFRerankerOptions

# Interface: RRFRerankerOptions

Options for [RRFReranker.create](../classes/RRFReranker.md#create).

## Properties

### k?

```ts
optional k: number;
```

Constant used in the RRF formula (default `60`).

***

### returnScore?

```ts
optional returnScore: "all" | "relevance";
```

Controls which score columns appear in the output.
- `"relevance"` (default): only `_relevance_score` is kept.
- `"all"`: `_distance`, `_score`, and `_relevance_score` are all retained.
