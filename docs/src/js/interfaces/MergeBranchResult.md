[**@delta-ai/lancedb**](../README.md) • **Docs**

***

[@delta-ai/lancedb](../globals.md) / MergeBranchResult

# Interface: MergeBranchResult

Result of previewing or attempting a branch merge.

## Properties

### diff

```ts
diff: BranchDiff;
```

***

### mainVersionAfter?

```ts
optional mainVersionAfter: number;
```

***

### preview

```ts
preview: MergePreview;
```

***

### status

```ts
status:
  | "unknown"
  | "rejected"
  | "ready"
  | "notImplemented"
  | "merged";
```
