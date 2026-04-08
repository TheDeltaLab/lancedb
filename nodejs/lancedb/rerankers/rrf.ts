// SPDX-License-Identifier: Apache-2.0
// SPDX-FileCopyrightText: Copyright The LanceDB Authors

import { RecordBatch } from "apache-arrow";
import { fromBufferToRecordBatch, fromRecordBatchToBuffer } from "../arrow";
import { RrfReranker as NativeRRFReranker } from "../native";

/**
 * Options for {@link RRFReranker.create}.
 */
export interface RRFRerankerOptions {
  /** Constant used in the RRF formula (default `60`). */
  k?: number;
  /**
   * Controls which score columns appear in the output.
   * - `"relevance"` (default): only `_relevance_score` is kept.
   * - `"all"`: `_distance`, `_score`, and `_relevance_score` are all retained.
   */
  returnScore?: "relevance" | "all";
}

/**
 * Reranks the results using the Reciprocal Rank Fusion (RRF) algorithm.
 *
 * @param k - Constant used in the RRF formula (default `60`). Experiments
 *   indicate that `k = 60` was near-optimal, but the choice is not critical.
 *   See paper: https://plg.uwaterloo.ca/~gvcormac/cormacksigir09-rrf.pdf
 * @param returnScore - Controls which score columns appear in the output.
 *   - `"relevance"` (default): Only the `_relevance_score` column is kept;
 *     the raw `_distance` and `_score` columns are dropped.
 *   - `"all"`: All score columns are retained alongside `_relevance_score`,
 *     which is useful for debugging.
 *
 * @hideconstructor
 */
export class RRFReranker {
  private inner: NativeRRFReranker;

  /** @ignore */
  constructor(inner: NativeRRFReranker) {
    this.inner = inner;
  }

  /** Create with a specific `k` value (default `60`, `returnScore` defaults to `"relevance"`). */
  public static async create(k?: number): Promise<RRFReranker>;
  /** Create with an options object. */
  public static async create(options: RRFRerankerOptions): Promise<RRFReranker>;
  public static async create(
    kOrOptions?: number | RRFRerankerOptions,
  ): Promise<RRFReranker> {
    let k = 60;
    let score: "relevance" | "all" = "relevance";

    if (typeof kOrOptions === "object" && kOrOptions !== null) {
      k = kOrOptions.k ?? 60;
      score = kOrOptions.returnScore ?? "relevance";
    } else if (kOrOptions !== undefined) {
      k = kOrOptions;
    }

    return new RRFReranker(
      await NativeRRFReranker.tryNew(new Float32Array([k]), score),
    );
  }

  async rerankHybrid(
    query: string,
    vecResults: RecordBatch,
    ftsResults: RecordBatch,
  ): Promise<RecordBatch> {
    const buffer = await this.inner.rerankHybrid(
      query,
      await fromRecordBatchToBuffer(vecResults),
      await fromRecordBatchToBuffer(ftsResults),
    );
    const recordBatch = await fromBufferToRecordBatch(buffer);

    return recordBatch as RecordBatch;
  }
}
