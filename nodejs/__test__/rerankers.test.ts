// SPDX-License-Identifier: Apache-2.0
// SPDX-FileCopyrightText: Copyright The LanceDB Authors

import { spawn } from "node:child_process";
import * as path from "node:path";
import { RecordBatch } from "apache-arrow";
import * as tmp from "tmp";
import { Connection, Index, Table, connect, makeArrowTable } from "../lancedb";
import { RRFReranker } from "../lancedb/rerankers";

describe("rerankers", function () {
  let tmpDir: tmp.DirResult;
  let conn: Connection;
  let table: Table;

  beforeEach(async () => {
    tmpDir = tmp.dirSync({ unsafeCleanup: true });
    conn = await connect(tmpDir.name);
    table = await conn.createTable("mytable", [
      { vector: [0.1, 0.1], text: "dog" },
      { vector: [0.2, 0.2], text: "cat" },
    ]);
    await table.createIndex("text", {
      config: Index.fts(),
      replace: true,
    });
  });

  it("will query with the custom reranker", async function () {
    const expectedResult = [
      {
        text: "albert",
        // biome-ignore lint/style/useNamingConvention: this is the lance field name
        _relevance_score: 0.99,
      },
    ];
    class MyCustomReranker {
      async rerankHybrid(
        _query: string,
        _vecResults: RecordBatch,
        _ftsResults: RecordBatch,
      ): Promise<RecordBatch> {
        // no reranker logic, just return some static data
        const table = makeArrowTable(expectedResult);
        return table.batches[0];
      }
    }

    let result = await table
      .query()
      .nearestTo([0.1, 0.1])
      .fullTextSearch("dog")
      .rerank(new MyCustomReranker())
      .select(["text"])
      .limit(5)
      .toArray();

    result = JSON.parse(JSON.stringify(result)); // convert StructRow to Object
    expect(result).toEqual([
      {
        text: "albert",
      },
    ]);
  });

  it("will query with RRFReranker", async function () {
    // smoke test to see if the Rust wrapping Typescript is wired up correctly
    const result = await table
      .query()
      .nearestTo([0.1, 0.1])
      .fullTextSearch("dog")
      .rerank(await RRFReranker.create())
      .select(["text"])
      .limit(5)
      .toArray();

    expect(result).toHaveLength(2);
  });

  it("will query with RRFReranker returnScore='all'", async function () {
    // When returnScore="all", both _distance (vector) and _score (FTS) are
    // retained alongside _relevance_score. Rows that only appear in one result
    // set will have null for the score column from the other set.
    const reranker = await RRFReranker.create({ k: 60, returnScore: "all" });
    const result = await table
      .query()
      .nearestTo([0.1, 0.1])
      .fullTextSearch("dog")
      .rerank(reranker)
      .limit(5)
      .toArray();

    expect(result).toHaveLength(2);
    const keys = Object.keys(result[0] as object);
    expect(keys).toContain("_distance");
    expect(keys).toContain("_score");
    expect(keys).toContain("_relevance_score");
  });

  it("will query with RRFReranker returnScore='relevance' preserves original behavior", async function () {
    // When returnScore="relevance" (default), the original merge behavior is
    // preserved: _score (FTS) is kept, _distance is not present because the
    // merge uses the FTS schema as its base.
    const reranker = await RRFReranker.create({ returnScore: "relevance" });
    const result = await table
      .query()
      .nearestTo([0.1, 0.1])
      .fullTextSearch("dog")
      .rerank(reranker)
      .limit(5)
      .toArray();

    expect(result).toHaveLength(2);
    const keys = Object.keys(result[0] as object);
    expect(keys).not.toContain("_distance");
    expect(keys).toContain("_relevance_score");
  });

  it("will query with RRFReranker returnScore='all' and select including all scores", async function () {
    const reranker = await RRFReranker.create({ k: 60, returnScore: "all" });
    const result = await table
      .query()
      .nearestTo([0.1, 0.1])
      .fullTextSearch("dog")
      .rerank(reranker)
      .select(["text", "_distance", "_score", "_relevance_score"])
      .limit(5)
      .toArray();

    expect(result).toHaveLength(2);
    const keys = Object.keys(result[0] as object);
    expect(keys).toContain("text");
    expect(keys).toContain("_distance");
    expect(keys).toContain("_score");
    expect(keys).toContain("_relevance_score");
    expect(keys).not.toContain("vector");
  });

  it("will query with RRFReranker returnScore='all' and select only _distance", async function () {
    const reranker = await RRFReranker.create({ k: 60, returnScore: "all" });
    const result = await table
      .query()
      .nearestTo([0.1, 0.1])
      .fullTextSearch("dog")
      .rerank(reranker)
      .select(["text", "_distance"])
      .limit(5)
      .toArray();

    expect(result).toHaveLength(2);
    const keys = Object.keys(result[0] as object);
    expect(keys).toContain("text");
    expect(keys).toContain("_distance");
    expect(keys).not.toContain("_score");
    expect(keys).not.toContain("_relevance_score");
    expect(keys).not.toContain("vector");
  });

  it("will query with RRFReranker returnScore='all' and select only _score", async function () {
    const reranker = await RRFReranker.create({ k: 60, returnScore: "all" });
    const result = await table
      .query()
      .nearestTo([0.1, 0.1])
      .fullTextSearch("dog")
      .rerank(reranker)
      .select(["text", "_score"])
      .limit(5)
      .toArray();

    expect(result).toHaveLength(2);
    const keys = Object.keys(result[0] as object);
    expect(keys).toContain("text");
    expect(keys).toContain("_score");
    expect(keys).not.toContain("_distance");
    expect(keys).not.toContain("_relevance_score");
    expect(keys).not.toContain("vector");
  });

  it("will query with RRFReranker returnScore='all' and select only _relevance_score", async function () {
    const reranker = await RRFReranker.create({ k: 60, returnScore: "all" });
    const result = await table
      .query()
      .nearestTo([0.1, 0.1])
      .fullTextSearch("dog")
      .rerank(reranker)
      .select(["text", "_relevance_score"])
      .limit(5)
      .toArray();

    expect(result).toHaveLength(2);
    const keys = Object.keys(result[0] as object);
    expect(keys).toContain("text");
    expect(keys).toContain("_relevance_score");
    expect(keys).not.toContain("_distance");
    expect(keys).not.toContain("_score");
    expect(keys).not.toContain("vector");
  });

  it("will query with RRFReranker returnScore='all' and select without scores", async function () {
    const reranker = await RRFReranker.create({ k: 60, returnScore: "all" });
    const result = await table
      .query()
      .nearestTo([0.1, 0.1])
      .fullTextSearch("dog")
      .rerank(reranker)
      .select(["text"])
      .limit(5)
      .toArray();

    expect(result).toHaveLength(2);
    const keys = Object.keys(result[0] as object);
    expect(keys).toContain("text");
    // score columns not in select, should not appear
    expect(keys).not.toContain("_distance");
    expect(keys).not.toContain("_score");
    expect(keys).not.toContain("_relevance_score");
    expect(keys).not.toContain("vector");
  });

  it("does not keep process alive after rerank query", async function () {
    const script = `
import * as lancedb from "./dist/index.js";
import * as os from "node:os";
import * as path from "node:path";
import * as fs from "node:fs/promises";

const dir = await fs.mkdtemp(path.join(os.tmpdir(), "lancedb-rerank-exit-"));
const db = await lancedb.connect(dir);
const table = await db.createTable("test", [{ text: "hello", vector: [1, 2, 3] }], {
  mode: "overwrite",
});
await table.createIndex("text", { config: lancedb.Index.fts() });
await table.waitForIndex(["text_idx"], 30);

const reranker = await lancedb.rerankers.RRFReranker.create();
await table
  .query()
  .nearestTo([1, 2, 3])
  .fullTextSearch("hello")
  .rerank(reranker)
  .toArray();

table.close();
db.close();
`;

    await new Promise<void>((resolve, reject) => {
      const child = spawn(
        process.execPath,
        ["--input-type=module", "-e", script],
        {
          cwd: path.resolve(__dirname, ".."),
          stdio: ["ignore", "pipe", "pipe"],
        },
      );

      let stdout = "";
      let stderr = "";

      child.stdout.on("data", (chunk) => {
        stdout += chunk.toString();
      });

      child.stderr.on("data", (chunk) => {
        stderr += chunk.toString();
      });

      const timeout = setTimeout(() => {
        child.kill();
        reject(
          new Error(
            `child process did not exit in time\nstdout:\n${stdout}\nstderr:\n${stderr}`,
          ),
        );
      }, 20_000);

      child.on("error", (err) => {
        clearTimeout(timeout);
        reject(err);
      });

      child.on("exit", (code, signal) => {
        clearTimeout(timeout);
        if (signal !== null) {
          reject(
            new Error(
              `child process exited with signal ${signal}\nstdout:\n${stdout}\nstderr:\n${stderr}`,
            ),
          );
          return;
        }

        if (code !== 0) {
          reject(
            new Error(
              `child process exited with code ${code}\nstdout:\n${stdout}\nstderr:\n${stderr}`,
            ),
          );
          return;
        }

        resolve();
      });
    });
  });
});
