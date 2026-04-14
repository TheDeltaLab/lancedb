// SPDX-License-Identifier: Apache-2.0
// SPDX-FileCopyrightText: Copyright The LanceDB Authors

import {
  Connection,
  LocalConnection,
  cleanseStorageOptions,
} from "./connection";

import {
  ConnectionOptions,
  Connection as LanceDbConnection,
  ProfilingOptions,
  Session,
  initProfiling as nativeInitProfiling,
} from "./native.js";
import { getTraceparent } from "./query";

export {
  AddColumnsSql,
  ConnectionOptions,
  IndexStatistics,
  IndexConfig,
  OptimizeStats,
  CompactionStats,
  RemovalStats,
  TableStatistics,
  FragmentStatistics,
  FragmentSummaryStats,
  Tags,
  TagContents,
  MergeResult,
  AddResult,
  AddColumnsResult,
  AlterColumnsResult,
  DeleteResult,
  DropColumnsResult,
  UpdateResult,
  SplitCalculatedOptions,
  SplitRandomOptions,
  SplitHashOptions,
  SplitSequentialOptions,
  ShuffleOptions,
} from "./native.js";

export {
  makeArrowTable,
  MakeArrowTableOptions,
  Data,
  VectorColumnOptions,
} from "./arrow";

export {
  Connection,
  CreateTableOptions,
  TableNamesOptions,
  OpenTableOptions,
} from "./connection";

export { Session } from "./native.js";

export {
  ExecutableQuery,
  Query,
  QueryBase,
  VectorQuery,
  TakeQuery,
  QueryExecutionOptions,
  FullTextSearchOptions,
  RecordBatchIterator,
  FullTextQuery,
  MatchQuery,
  PhraseQuery,
  BoostQuery,
  MultiMatchQuery,
  BooleanQuery,
  FullTextQueryType,
  Operator,
  Occur,
  withTraceparent,
} from "./query";

export {
  Index,
  IndexOptions,
  IvfPqOptions,
  IvfRqOptions,
  IvfFlatOptions,
  HnswPqOptions,
  HnswSqOptions,
  FtsOptions,
} from "./indices";

export {
  Table,
  AddDataOptions,
  UpdateOptions,
  OptimizeOptions,
  Version,
  ColumnAlteration,
} from "./table";

export { MergeInsertBuilder, WriteExecutionOptions } from "./merge";

export * as embedding from "./embedding";
export { permutationBuilder, PermutationBuilder } from "./permutation";
export * as rerankers from "./rerankers";
export {
  SchemaLike,
  TableLike,
  FieldLike,
  RecordBatchLike,
  DataLike,
  IntoVector,
  MultiVector,
} from "./arrow";
export { IntoSql, packBits } from "./util";

/**
 * Initialize OTLP profiling for traces, metrics, and logs.
 *
 * Sets up the OpenTelemetry pipeline to export telemetry data to
 * an OTLP-compatible collector (e.g. Grafana Alloy, OpenTelemetry Collector).
 *
 * Can be called at any time — the global subscriber installed at module
 * load supports hot-reloading, so there is no "must call before any
 * LanceDB operations" constraint.
 *
 * Requires the native library to be built with the `profiling-otlp` feature.
 *
 * @param options - Optional configuration for the OTLP endpoint.
 *
 * @example
 * ```ts
 * import { initProfiling } from "lancedb";
 *
 * initProfiling({
 *   otlpEndpoint: "http://localhost:4318",
 *   serviceName: "my-app",
 * });
 * ```
 */
export function initProfiling(options?: ProfilingOptions): void {
  nativeInitProfiling(options ?? undefined);
}
export type { ProfilingOptions } from "./native.js";

/**
 * Connect to a LanceDB instance at the given URI.
 *
 * Accepted formats:
 *
 * - `/path/to/database` - local database
 * - `s3://bucket/path/to/database` or `gs://bucket/path/to/database` - database on cloud storage
 * @param {string} uri - The uri of the database.
 * @see {@link ConnectionOptions} for more details on the URI format.
 * @param  options - The options to use when connecting to the database
 * @example
 * ```ts
 * const conn = await connect("/path/to/database");
 * ```
 * @example
 * ```ts
 * const conn = await connect(
 *   "s3://bucket/path/to/database",
 *   {storageOptions: {timeout: "60s"}
 * });
 * ```
 */
export async function connect(
  uri: string,
  options?: Partial<ConnectionOptions>,
  session?: Session,
): Promise<Connection>;
/**
 * Connect to a LanceDB instance at the given URI.
 *
 * Accepted formats:
 *
 * - `/path/to/database` - local database
 * - `s3://bucket/path/to/database` or `gs://bucket/path/to/database` - database on cloud storage
 * @param  options - The options to use when connecting to the database
 * @see {@link ConnectionOptions} for more details on the URI format.
 * @example
 * ```ts
 * const conn = await connect({
 *   uri: "/path/to/database",
 *   storageOptions: {timeout: "60s"}
 * });
 * ```
 *
 * @example
 * ```ts
 * const session = Session.default();
 * const conn = await connect({
 *   uri: "/path/to/database",
 *   session: session
 * });
 * ```
 */
export async function connect(
  options: Partial<ConnectionOptions> & { uri: string },
): Promise<Connection>;
export async function connect(
  uriOrOptions: string | (Partial<ConnectionOptions> & { uri: string }),
  optionsOrSession?: Partial<ConnectionOptions> | Session,
  _session?: Session,
): Promise<Connection> {
  let uri: string | undefined;
  let finalOptions: Partial<ConnectionOptions> = {};

  if (typeof uriOrOptions !== "string") {
    // First overload: connect(options)
    const { uri: uri_, ...opts } = uriOrOptions;
    uri = uri_;
    finalOptions = opts;
  } else {
    // Second overload: connect(uri, options?, session?)
    uri = uriOrOptions;

    // Handle optionsOrSession parameter
    if (optionsOrSession && "inner" in optionsOrSession) {
      // Second param is session, so no options provided
      finalOptions = {};
    } else {
      // Second param is options
      finalOptions = (optionsOrSession as Partial<ConnectionOptions>) || {};
    }
  }

  if (!uri) {
    throw new Error("uri is required");
  }

  finalOptions = (finalOptions as ConnectionOptions) ?? {};
  (<ConnectionOptions>finalOptions).storageOptions = cleanseStorageOptions(
    (<ConnectionOptions>finalOptions).storageOptions,
  );

  const nativeConn = await LanceDbConnection.new(
    uri,
    finalOptions,
    await getTraceparent(),
  );
  return new LocalConnection(nativeConn);
}
