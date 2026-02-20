#!/usr/bin/env node

/**
 * Quick smoke test for the WASM module.
 * Run: node test-wasm.mjs
 */

import { createRequire } from "module";
const require = createRequire(import.meta.url);

const wasm = require("./packages/mcp-server/wasm/ast_surgeon_wasm.js");

// Test 1: rename_symbol
console.log("=== Test 1: rename_symbol ===");
const renameReq = {
  content: "const foo = 1;\nconsole.log(foo);\n",
  language: "typescript",
  operations: [{ op: "rename_symbol", from: "foo", to: "bar" }],
  dry_run: false,
};
const renameResult = JSON.parse(wasm.process_file(JSON.stringify(renameReq)));
console.log("Status:", renameResult.status);
console.log("Content:", renameResult.content);
console.log("Changes:", renameResult.changes.length);
console.assert(renameResult.status === "applied");
console.assert(renameResult.content.includes("bar"));
console.assert(!renameResult.content.includes("foo"));

// Test 2: add_import
console.log("\n=== Test 2: add_import ===");
const importReq = {
  content: "import { useState } from 'react';\n\nconst App = () => {};\n",
  language: "typescript",
  operations: [
    {
      op: "add_import",
      source: "react",
      specifiers: ["useEffect"],
    },
  ],
  dry_run: false,
};
const importResult = JSON.parse(wasm.process_file(JSON.stringify(importReq)));
console.log("Status:", importResult.status);
console.log("Content:", importResult.content);
console.assert(importResult.status === "applied");
console.assert(importResult.content.includes("useEffect"));
console.assert(importResult.content.includes("useState"));

// Test 3: dry_run
console.log("\n=== Test 3: dry_run ===");
const dryRunReq = {
  content: "function fetchData(url: string) { return fetch(url); }\n",
  language: "typescript",
  operations: [{ op: "make_async", function_name: "fetchData" }],
  dry_run: true,
};
const dryRunResult = JSON.parse(wasm.process_file(JSON.stringify(dryRunReq)));
console.log("Status:", dryRunResult.status);
console.log("Edit count:", dryRunResult.edit_count);
console.assert(dryRunResult.status === "preview");
console.assert(dryRunResult.content === null);
console.assert(dryRunResult.edit_count > 0);

// Test 4: batch processing
console.log("\n=== Test 4: batch processing ===");
const batchReq = {
  files: [
    {
      path: "src/App.tsx",
      content:
        "import { useState } from 'react';\n\nexport function App() {\n  const [count, setCount] = useState(0);\n  return <div>{count}</div>;\n}\n",
      language: "tsx",
      operations: [
        { op: "rename_symbol", from: "count", to: "value" },
      ],
    },
    {
      path: "src/utils.ts",
      content: "export function formatDate(date: Date) {\n  return date.toISOString();\n}\n",
      language: "typescript",
      operations: [
        { op: "make_async", function_name: "formatDate" },
      ],
    },
  ],
  dry_run: false,
};
const batchResult = JSON.parse(wasm.process_batch(JSON.stringify(batchReq)));
console.log("Status:", batchResult.status);
console.log("Total edits:", batchResult.total_edits);
console.log("Files processed:", batchResult.results.length);
console.assert(batchResult.status === "applied");
console.assert(batchResult.results.length === 2);
console.assert(batchResult.results[0].content.includes("value"));
console.assert(batchResult.results[1].content.includes("async"));

// Test 5: error handling
console.log("\n=== Test 5: error handling ===");
const errorReq = {
  content: "const x = 1;\n",
  language: "typescript",
  operations: [
    { op: "rename_symbol", from: "nonexistent", to: "y" },
  ],
};
const errorResult = JSON.parse(wasm.process_file(JSON.stringify(errorReq)));
console.log("Status:", errorResult.status);
console.log("Error:", errorResult.operation_errors[0]?.message);
console.assert(errorResult.status === "error");
console.assert(errorResult.operation_errors.length > 0);

console.log("\n=== All smoke tests passed! ===");
