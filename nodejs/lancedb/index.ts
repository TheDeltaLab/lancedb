// SPDX-License-Identifier: Apache-2.0
// SPDX-FileCopyrightText: Copyright The LanceDB Authors

import {
  Connection,
  LocalConnection,
  cleanseStorageOptions,
} from "./connection";

import {
  ConnectNamespaceOptions,
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
  ConnectNamespaceOptions,
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
  ListNamespacesOptions,
  CreateNamespaceOptions,
  DropNamespaceOptions,
  ListNamespacesResponse,
  CreateNamespaceResponse,
  DropNamespaceResponse,
  DescribeNamespaceResponse,
  RenameTableOptions,
} from "./connection";

export { Session } from "./native.js";

export {
  ExecutableQuery,
  Query,
  QueryBase,
  VectorQuery,
  TakeQuery,
  QueryExecutionOptions,
  ColumnOrdering,
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
  WriteProgress,
  LsmWriteSpec,
  ColumnAlteration,
} from "./table";

export { MergeInsertBuilder, WriteExecutionOptions } from "./merge";

export * as embedding from "./embedding";
export { permutationBuilder, PermutationBuilder } from "./permutation";
export { Scannable, ScannableOptions } from "./scannable";
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

/**
 * Configuration for the built-in directory namespace (`"dir"`).
 *
 * The directory namespace stores tables under a single root path (local
 * filesystem or object storage URI). See
 * {@link https://docs.lancedb.com/namespaces} for the documented surface;
 * less-common knobs live under {@link DirNamespaceConfig.extraProperties}.
 */
export interface DirNamespaceConfig {
  /** Root path or URI containing the LanceDB tables. */
  root: string;
  /**
   * Whether to maintain a namespace manifest at the root. Required for
   * child namespaces. Defaults to true on the impl side.
   */
  manifestEnabled?: boolean;
  /**
   * Additional raw properties passed verbatim to the namespace
   * implementation (e.g. `storage.*`, `credential_vendor.*`). Typed
   * fields above take precedence on key collision.
   */
  extraProperties?: Record<string, string>;
}

/**
 * Configuration for the built-in REST namespace (`"rest"`).
 *
 * The REST namespace talks to a remote catalog server over HTTP. See
 * {@link https://docs.lancedb.com/namespaces} for the documented surface;
 * less-common knobs (TLS, metrics) live under
 * {@link RestNamespaceConfig.extraProperties}.
 */
export interface RestNamespaceConfig {
  /** Catalog endpoint URL. */
  uri: string;
  /**
   * HTTP headers forwarded with each request. Keys are passed through
   * as-is (e.g. `"x-api-key"`, `"Authorization"`).
   */
  headers?: Record<string, string>;
  /**
   * Additional raw properties passed verbatim to the namespace
   * implementation (e.g. `tls.*`, `ops_metrics_enabled`, `delimiter`).
   * Typed fields above take precedence on key collision.
   */
  extraProperties?: Record<string, string>;
}

function dirConfigToProperties(
  config: DirNamespaceConfig,
): Record<string, string> {
  // Spread the whole input so that unknown keys (e.g. a raw `manifest_enabled`
  // passed via the dynamic-impl path) flow through instead of being dropped.
  // Typed transformations layer on top.
  const { manifestEnabled, extraProperties, ...rest } = config;
  const properties: Record<string, string> = {
    ...(extraProperties ?? {}),
    ...(rest as Record<string, string>),
  };
  if (manifestEnabled !== undefined) {
    properties.manifest_enabled = String(manifestEnabled);
  }
  return properties;
}

function restConfigToProperties(
  config: RestNamespaceConfig,
): Record<string, string> {
  const { headers, extraProperties, ...rest } = config;
  const properties: Record<string, string> = {
    ...(extraProperties ?? {}),
    ...(rest as Record<string, string>),
  };
  if (headers) {
    for (const [name, value] of Object.entries(headers)) {
      properties[`headers.${name}`] = value;
    }
  }
  return properties;
}

/**
 * Connect to a LanceDB database through a namespace.
 *
 * Unlike {@link connect}, which routes by URI scheme (local path vs.
 * `db://` cloud), `connectNamespace` always returns a namespace-backed
 * connection. The `implName` selects the namespace implementation:
 *
 * - `"dir"` — directory namespace, configured with {@link DirNamespaceConfig}.
 * - `"rest"` — remote REST catalog, configured with {@link RestNamespaceConfig}.
 * - Any other string — full module path for a custom implementation,
 *   configured with a free-form string-keyed `properties` map.
 *
 * @example Typed dir namespace
 * ```ts
 * const db = await connectNamespace("dir", { root: "/path/to/db" });
 * await db.createTable("users", [{ id: 1 }]);
 * ```
 *
 * @example Typed REST namespace with auth headers
 * ```ts
 * const db = await connectNamespace("rest", {
 *   uri: "https://catalog.example.com",
 *   headers: { "x-api-key": process.env.CATALOG_KEY ?? "" },
 * });
 * ```
 *
 * @example Custom implementation with raw properties
 * ```ts
 * const db = await connectNamespace("my.custom.Namespace", {
 *   endpoint: "...",
 * });
 * ```
 */
export function connectNamespace(
  implName: "dir",
  config: DirNamespaceConfig,
  options?: Partial<ConnectNamespaceOptions>,
): Promise<Connection>;
/**
 * Connect through the built-in REST namespace.
 *
 * Configured with {@link RestNamespaceConfig}. See the function-level
 * documentation above for the full surface, examples, and how this
 * relates to {@link connect}.
 *
 * @example
 * ```ts
 * const db = await connectNamespace("rest", {
 *   uri: "https://catalog.example.com",
 *   headers: { "x-api-key": process.env.CATALOG_KEY ?? "" },
 * });
 * ```
 */
export function connectNamespace(
  implName: "rest",
  config: RestNamespaceConfig,
  options?: Partial<ConnectNamespaceOptions>,
): Promise<Connection>;
/**
 * Connect through a custom namespace implementation by full module path,
 * configured with a free-form string-keyed `properties` map. Use the
 * typed overloads above for the built-in `"dir"` and `"rest"` impls.
 *
 * See the function-level documentation above for examples and how this
 * relates to {@link connect}.
 *
 * @example
 * ```ts
 * const db = await connectNamespace("my.custom.Namespace", {
 *   endpoint: "...",
 * });
 * ```
 */
export function connectNamespace(
  implName: string,
  properties: Record<string, string>,
  options?: Partial<ConnectNamespaceOptions>,
): Promise<Connection>;
export async function connectNamespace(
  implName: string,
  configOrProperties:
    | DirNamespaceConfig
    | RestNamespaceConfig
    | Record<string, string>,
  options?: Partial<ConnectNamespaceOptions>,
): Promise<Connection> {
  let properties: Record<string, string>;
  if (implName === "dir") {
    properties = dirConfigToProperties(
      configOrProperties as DirNamespaceConfig,
    );
  } else if (implName === "rest") {
    properties = restConfigToProperties(
      configOrProperties as RestNamespaceConfig,
    );
  } else {
    properties = configOrProperties as Record<string, string>;
  }

  const finalOptions: ConnectNamespaceOptions = (options ??
    {}) as ConnectNamespaceOptions;
  finalOptions.storageOptions = cleanseStorageOptions(
    finalOptions.storageOptions,
  );

  const nativeConn = await LanceDbConnection.newWithNamespace(
    implName,
    properties,
    finalOptions,
  );
  return new LocalConnection(nativeConn);
}
