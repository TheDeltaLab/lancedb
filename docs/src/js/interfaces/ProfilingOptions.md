[**@delta-ai/lancedb**](../README.md) • **Docs**

***

[@delta-ai/lancedb](../globals.md) / ProfilingOptions

# Interface: ProfilingOptions

Options for configuring OTLP profiling export.

## Properties

### otlpEndpoint?

```ts
optional otlpEndpoint: string;
```

The OTLP collector endpoint (e.g. "http://localhost:4318").
Falls back to OTEL_EXPORTER_OTLP_ENDPOINT env var.

***

### protocol?

```ts
optional protocol: string;
```

Transport protocol: "http" (default) or "grpc".

***

### serviceName?

```ts
optional serviceName: string;
```

The service name for telemetry (defaults to "lancedb").
Falls back to OTEL_SERVICE_NAME env var.
