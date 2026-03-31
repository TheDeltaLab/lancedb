#!/usr/bin/env node
// SPDX-License-Identifier: Apache-2.0
// SPDX-FileCopyrightText: Copyright The LanceDB Authors
// A zero-dependency mock OpenAI embeddings API endpoint for testing purposes.

const http = require("http");

const port = parseInt(process.argv[2] || "8000", 10);

const server = http.createServer((req, res) => {
  if (req.method === "POST") {
    let body = "";
    req.on("data", (chunk) => (body += chunk));
    req.on("end", () => {
      const data = JSON.parse(body);
      const numInputs =
        typeof data.input === "string" ? 1 : data.input.length;
      const model = data.model || "text-embedding-ada-002";

      const embeddings = [];
      for (let i = 0; i < numInputs; i++) {
        embeddings.push({
          object: "embedding",
          embedding: new Array(1536).fill(0.1),
          index: i,
        });
      }

      const response = JSON.stringify({
        object: "list",
        data: embeddings,
        model,
        usage: { prompt_tokens: 0, total_tokens: 0 },
      });

      res.writeHead(200, { "Content-Type": "application/json" });
      res.end(response);
    });
  }
});

server.listen(port, "0.0.0.0", () => {
  console.log(`Mock OpenAI server started on port ${port}`);
});
