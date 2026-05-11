[**@delta-ai/lancedb**](../README.md) • **Docs**

***

[@delta-ai/lancedb](../globals.md) / ConnectionOptions

# Interface: ConnectionOptions

## Properties

### manifestEnabled?

```ts
optional manifestEnabled: boolean;
```

(For LanceDB OSS only): use directory namespace manifests as the source
of truth for table metadata. Existing directory-listed root tables are
migrated into the manifest on access.

***

### namespaceClientProperties?

```ts
optional namespaceClientProperties: Record<string, string>;
```

(For LanceDB OSS only): extra properties for the backing namespace
client used by manifest-enabled native connections.

***

### readConsistencyInterval?

```ts
optional readConsistencyInterval: number;
```

(For LanceDB OSS only): The interval, in seconds, at which to check for
updates to the table from other processes. If None, then consistency is not
checked. For performance reasons, this is the default. For strong
consistency, set this to zero seconds. Then every read will check for
updates from other processes. As a compromise, you can set this to a
non-zero value for eventual consistency. If more than that interval
has passed since the last check, then the table will be checked for updates.
Note: this consistency only applies to read operations. Write operations are
always consistent.

***

### session?

```ts
optional session: Session;
```

(For LanceDB OSS only): the session to use for this connection. Holds
shared caches and other session-specific state.

***

### storageOptions?

```ts
optional storageOptions: Record<string, string>;
```

Configuration for object storage.

The available options are described at https://docs.lancedb.com/storage/
