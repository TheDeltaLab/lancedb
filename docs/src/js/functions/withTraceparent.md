[**@delta-ai/lancedb**](../README.md) • **Docs**

***

[@delta-ai/lancedb](../globals.md) / withTraceparent

# Function: withTraceparent()

```ts
function withTraceparent<T>(traceparent, fn): T
```

Run a function with a specific W3C traceparent string.

Any LanceDB operations called within `fn` will use this traceparent
to link Rust-side tracing spans to the caller's trace.

## Type Parameters

• **T**

## Parameters

* **traceparent**: `undefined` \| `string`

* **fn**

## Returns

`T`

## Example

```ts
import { withTraceparent } from "@delta-ai/lancedb";

// Inside a Hono / Express / Koa handler:
const tp = req.headers.get("traceparent");
const results = await withTraceparent(tp, () => table.query().limit(10).toArray());
```
