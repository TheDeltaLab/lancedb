[**@delta-ai/lancedb**](../README.md) • **Docs**

***

[@delta-ai/lancedb](../globals.md) / initProfiling

# Function: initProfiling()

```ts
function initProfiling(options?): void
```

Initialize OTLP profiling for traces, metrics, and logs.

Sets up the OpenTelemetry pipeline to export telemetry data to
an OTLP-compatible collector (e.g. Grafana Alloy, OpenTelemetry Collector).

Can be called at any time — the global subscriber installed at module
load supports hot-reloading, so there is no "must call before any
LanceDB operations" constraint.

Requires the native library to be built with the `profiling-otlp` feature.

## Parameters

* **options?**: [`ProfilingOptions`](../interfaces/ProfilingOptions.md)
    Optional configuration for the OTLP endpoint.

## Returns

`void`

## Example

```ts
import { initProfiling } from "lancedb";

initProfiling({
  otlpEndpoint: "http://localhost:4318",
  serviceName: "my-app",
});
```
