[**@delta-ai/lancedb**](../../../README.md) • **Docs**

***

[@delta-ai/lancedb](../../../globals.md) / [embedding](../README.md) / getRegistry

# Function: getRegistry()

```ts
function getRegistry(): EmbeddingFunctionRegistry
```

Utility function to get the global instance of the registry

## Returns

[`EmbeddingFunctionRegistry`](../classes/EmbeddingFunctionRegistry.md)

`EmbeddingFunctionRegistry` The global instance of the registry

## Example

```ts
const registry = getRegistry();
const openai = registry.get("openai").create();
