# ast-surgeon: Implementation Plan

## Context

**Problem:** Every AI coding tool (Claude Code, Cursor, Copilot, Aider) edits code via text -- `old_string/new_string`, search-and-replace, or full-file rewrites. This is error-prone, token-expensive, and format-destructive. No tool provides a **formal operation vocabulary** (`rename_symbol`, `add_import`, `wrap_in_component`) that an LLM can emit as structured JSON, with the guarantee that the output always parses correctly.

**Solution:** ast-surgeon -- a Rust core (compiled to WASM) with a TypeScript MCP server layer. The LLM emits structured operations; the tool executes them as AST transformations via tree-sitter. "Correct by construction" -- output always parses.

**Architecture:** Rust core + tree-sitter (language-agnostic) compiled to WASM via wasm-pack. TypeScript MCP server wraps the WASM module and handles file I/O. Scope: ast-surgeon only (standalone, pluggable into the broader fe-tools suite later).

---

## Architecture Overview

```
  MCP Client (Claude Code, Cursor, etc.)
          |  stdio transport
  ┌───────┴───────────┐
  │  TypeScript MCP   │  file I/O, file discovery, tool schema,
  │  Server           │  dry_run, hint chaining
  └───────┬───────────┘
          |  JSON strings across WASM boundary
  ┌───────┴───────────┐
  │  Rust Core (WASM) │  tree-sitter parse, query, edit computation,
  │  via wasm-pack    │  validation, formatting preservation
  └───────────────────┘
```

**WASM Boundary:** Strings in (JSON operations + file contents), strings out (modified file contents + diagnostics). Stateless per call. No shared memory.

---

## Rust Crate Structure

```
ast-surgeon/
  Cargo.toml                          # workspace root
  crates/
    ast-surgeon-core/                 # Pure computation (language-agnostic)
      src/
        lib.rs                        # Public API: execute_operations()
        edit.rs                       # TextEdit, EditSet, reverse-order application
        format.rs                     # Indentation inference, comment attachment
        validate.rs                   # Re-parse verification (zero ERROR nodes)
        operations/
          mod.rs                      # Operation enum + Executable trait
          rename_symbol.rs
          imports.rs                  # add_import, remove_import, update_paths
          props.rs                    # add_prop, remove_prop (JSX)
          wrap.rs                     # wrap_in_block, wrap_jsx
          extract.rs                  # extract_component, extract_to_function
          hooks.rs                    # add_hook_call, add_hook_dependency
          signature.rs               # add_parameter, remove_parameter, make_async
    ast-surgeon-lang/                 # Language-specific query patterns
      src/
        lib.rs
        registry.rs                   # Language detection + grammar lookup
        typescript.rs                 # TS/TSX queries (imports, exports, JSX, hooks)
        javascript.rs
        css.rs
        vue.rs                        # Vue SFC region splitting
        svelte.rs                     # Svelte region splitting
        queries/                      # Bundled .scm query files
    ast-surgeon-wasm/                 # WASM boundary (thin shell)
      src/
        lib.rs                        # #[wasm_bindgen]: process_file, process_batch, dry_run
        protocol.rs                   # JSON request/response serde types
  packages/
    mcp-server/                       # TypeScript npm package
      src/
        index.ts                      # MCP server entry (stdio transport)
        wasm-bridge.ts                # WASM loading + typed wrappers
        handler.ts                    # Request orchestration, file I/O, dry_run
        tool-schema.ts                # fe_surgeon MCP tool definition
        file-discovery.ts             # Cross-file: find files by import graph
        hints.ts                      # Tool chaining hints in responses
        errors.ts                     # Structured error types
      package.json
  tests/
    fixtures/                         # Before/after test file pairs
    property/                         # proptest soundness tests
    integration/                      # MCP protocol tests
    corpus/                           # Real-world project tests
```

### Key Dependencies

| Crate | Purpose |
|-------|---------|
| `tree-sitter` | CST parsing (Rust API) |
| `tree-sitter-typescript` | TS + TSX grammars (compiled into WASM) |
| `tree-sitter-javascript` | JS + JSX grammars |
| `tree-sitter-css` | CSS grammar |
| `wasm-bindgen` | Rust-to-JS bridge |
| `serde` / `serde_json` | JSON serialization across WASM boundary |
| `thiserror` | Error types |
| `proptest` | Property-based testing |
| `criterion` | Benchmarks |

**WASM size budget:** ~3MB uncompressed (~1MB gzipped). Tier 1 grammars (TS, TSX, JS, CSS) bundled. Vue/Svelte handled by TS server splitting SFCs into regions.

---

## Text Editing Model

The core invariant: **edits are byte-range replacements on the original source text, applied in reverse order.**

```rust
struct TextEdit {
    start: usize,       // byte offset (inclusive)
    end: usize,         // byte offset (exclusive)
    replacement: String,
    label: String,       // for diagnostics
    priority: i32,       // for ordering adjacent insertions
}

struct EditSet {
    edits: Vec<TextEdit>,  // sorted by (start, end, priority) ascending
}
```

**Why reverse-order works:** Edits sorted ascending, iterated in reverse. The last edit (highest offset) is applied first -- it only changes bytes at the end, so earlier offsets remain valid. Each subsequent edit operates on still-valid byte ranges.

**Composing multiple operations on same file:** All operations compute edits against the **original** parse tree. All edits merge into a single EditSet. `EditSet::new()` detects overlapping edits and returns `EditConflict` if two operations target the same byte range.

**Formatting preservation:** Because we replace only targeted byte ranges, all untouched bytes (whitespace, comments, trailing commas, unusual formatting) are preserved byte-identically. For **new** code insertion, we infer indentation by examining the `sibling_prefix` (exact whitespace string from the adjacent line).

---

## Operation Vocabulary (22 Operations, 4 Tiers)

### Tier 1 -- Core (Universal, all languages)

| # | Operation | Parameters | Description |
|---|-----------|-----------|-------------|
| 1 | `rename_symbol` | `from`, `to`, `scope?` | Rename identifier at all reference sites. Skips string literals/comments. |
| 2 | `add_import` | `source`, `specifiers`, `type_only?` | Add import or merge into existing. Detects quote style, semicolons, ordering. |
| 3 | `remove_import` | `source`, `specifiers?` | Remove specifiers or entire import. Cleans up empty imports. |
| 4 | `update_import_paths` | `old_path`, `new_path`, `match_mode` | Update module paths after file moves. Handles dynamic imports + re-exports. |
| 5 | `add_parameter` | `function_name`, `param_name`, `type?`, `default?`, `position` | Add param to function. Handles arrow functions, destructuring, rest params. |
| 6 | `remove_parameter` | `function_name`, `param_name` | Remove param from function signature. |
| 7 | `make_async` | `function_name` | Add `async` keyword + wrap return type in `Promise<>`. |
| 8 | `extract_to_function` | `start_line`, `end_line`, `new_name`, `export?` | Extract statements into new function. Auto-computes params and return values. |

### Tier 2 -- Structural (Universal)

| # | Operation | Description |
|---|-----------|-------------|
| 9 | `wrap_in_block` | Wrap statements in `if`/`try-catch`/`for`/plain block. |
| 10 | `extract_to_variable` | Extract expression into a named `const`/`let`. |
| 11 | `inline_variable` | Replace variable with its initializer at all usage sites. |
| 12 | `move_to_file` | Move declaration to another file, update imports, optional re-export. |

### Tier 3 -- React/JSX

| # | Operation | Description |
|---|-----------|-------------|
| 13 | `add_prop` | Add prop to JSX usage or props type definition. Multi-line aware. |
| 14 | `remove_prop` | Remove prop from JSX element. |
| 15 | `wrap_in_component` | Wrap JSX in parent (Suspense, ErrorBoundary, etc.). Auto-imports wrapper. |
| 16 | `extract_component` | Extract JSX subtree into new component. Computes props from free variables. |
| 17 | `add_hook_call` | Add hook call to component. Ensures Rules of Hooks compliance. |
| 18 | `add_hook_dependency` | Add dependency to useEffect/useMemo/useCallback array. |

### Tier 4 -- Framework-Specific

| # | Operation | Description |
|---|-----------|-------------|
| 19 | `vue_add_to_setup` | Add statement to `<script setup>` or `setup()` function. |
| 20 | `vue_wrap_in_ref` | Convert variable to Vue reactive ref. Updates `.value` in script. |
| 21 | `svelte_add_reactive` | Add `$:` statement or `$derived()` rune. Detects Svelte 4 vs 5. |
| 22 | `svelte_add_store` | Create/subscribe to writable/readable/derived store. |

---

## Operation Execution Pipeline

```
  LLM emits JSON operations
       |
  [1. PARSE]     Parse source with tree-sitter → CST
       |
  [2. QUERY]     Find target nodes via tree-sitter S-expression queries
       |
  [3. COMPUTE]   Each operation computes Vec<TextEdit> from matched nodes
       |
  [4. MERGE]     Merge all edits for same file into single EditSet
       |
  [5. VALIDATE]  Check for overlaps, out-of-bounds
       |
  [6. APPLY]     Reverse-order string replacement
       |
  [7. VERIFY]    Re-parse result, check for ERROR/MISSING nodes
       |
  [8. RETURN]    New source + changes + diagnostics
```

**Guarantee:** If step 7 finds ERROR nodes, the operation returns an error and the original source is untouched. The LLM receives error details + suggestions.

---

## Cross-File Operations

**File discovery:** TypeScript MCP server (has filesystem access). Scans for files importing the target symbol via grep/import-following.

**Edit computation:** Rust/WASM (CPU-intensive). Server constructs a `BatchRequest` with all file contents, WASM processes each independently.

```
BatchRequest {
  files: [
    { path, content, language, operations }  // per file
  ]
}
→ WASM processes each file independently
→ BatchResponse {
  results: [ { path, content, changes } ]
  errors:  [ { path, error, op_index } ]
}
```

---

## MCP Tool Schema

```json
{
  "name": "fe_surgeon",
  "description": "Apply structured AST operations instead of rewriting files. Operations: rename_symbol, add_import, remove_import, add_prop, remove_prop, wrap_jsx, extract_component, add_hook_call, update_import_path, make_async. Faster and safer than text rewriting -- no syntax errors possible.",
  "inputSchema": {
    "type": "object",
    "required": ["operations"],
    "properties": {
      "operations": {
        "type": "array",
        "items": {
          "type": "object",
          "required": ["op"],
          "properties": {
            "op": { "type": "string", "enum": ["rename_symbol", "add_import", ...] },
            "file": { "type": "string" },
            "scope": { "type": "string", "enum": ["file", "project"], "default": "file" },
            "args": { "type": "object" }
          }
        }
      },
      "dry_run": { "type": "boolean", "default": false },
      "project_root": { "type": "string" }
    }
  }
}
```

**Response structure:**
```json
{
  "status": "applied|preview|partial|error",
  "files_modified": [{ "file": "...", "operations": [...], "edits_applied": N }],
  "files_created": [{ "file": "...", "reason": "..." }],
  "operations_applied": N,
  "operations_failed": N,
  "total_edits": N,
  "time_ms": N,
  "warnings": ["..."],
  "errors": [{ "operation_index": N, "code": "SYMBOL_NOT_FOUND", "message": "...", "suggestion": "..." }],
  "hint": "Call fe_verify to confirm lint/types/tests pass."
}
```

---

## Error Handling

| Error Code | When | LLM Action |
|------------|------|------------|
| `SYMBOL_NOT_FOUND` | Target not found in file | Correct name or use fe_skeleton to find it |
| `AMBIGUOUS_MATCH` | Multiple nodes match (returns locations) | Specify `occurrence` to disambiguate |
| `EDIT_CONFLICT` | Two ops target overlapping bytes | Split into separate calls |
| `INVALID_RESULT` | Output has syntax errors (should never happen) | Fall back to text edit; logged as bug |
| `SOURCE_HAS_ERRORS` | Input file already has syntax errors | Fix errors first or proceed with warning |
| `UNSUPPORTED_LANGUAGE` | Language not loaded | Use text-based editing |
| `INVALID_PARAMS` | Bad operation parameters | Fix parameters |

---

## PoC Milestones (4 Weeks)

Each milestone is independently demoable, testable, and retires a specific technical risk.

### Week 1: Foundation

**Milestone 1: Parse + Query (Days 1-3)**
- Rust crate with tree-sitter-typescript/tsx
- Parse any .ts/.tsx file, execute S-expression queries
- **Tests:** Parse 500-line TSX, query imports/hooks/JSX -- verify correct matches in <5ms
- **Risk retired:** tree-sitter queries are expressive enough for all operations
- **Acceptance:** CLI that prints query matches for any TS/TSX file

**Milestone 2: Text Edit Engine (Days 4-6)**
- `TextEdit`, `EditSet`, reverse-order application, overlap detection
- Implement `rename_symbol` (single file)
- **Tests:** Rename in 5-occurrence file, re-parse output -- zero ERROR nodes. Overlapping edits → error, not corruption
- **Risk retired:** Byte offset arithmetic is correct; edits never corrupt files
- **Acceptance:** CLI renames a symbol in a file; output is valid

### Week 2: Bridge

**Milestone 3: Formatting Preservation (Days 7-9)**
- Indentation inference from sibling nodes
- Comment attachment heuristics (leading/trailing)
- **Tests:** 10 "golden file" tests against real-world React components. Byte-identical untouched regions. Comments survive adjacent edits
- **Risk retired:** Edits don't mangle formatting
- **Acceptance:** Side-by-side diff showing ONLY targeted changes

**Milestone 4: WASM Compilation (Days 10-12)**
- `wasm-pack build --target nodejs` with tree-sitter grammars
- TypeScript wrapper that loads WASM module
- **Tests:** .wasm < 5MB. Parse + rename from Node.js in < 50ms. No memory leaks across 20 sequential files
- **Risk retired:** tree-sitter's C code compiles correctly to WASM; string boundary works
- **Acceptance:** `node test-wasm.mjs` processes a TSX file end-to-end

### Week 3: End-to-End

**Milestone 5: MCP Server (Days 13-16)**
- Full MCP server with `@modelcontextprotocol/sdk`
- `fe_surgeon` tool with complete schema
- dry_run support, structured error responses, hint chaining
- **Tests:** `tools/list` returns correct schema. `tools/call` with dry_run returns preview. Live call modifies file on disk
- **Risk retired:** Full MCP integration works; Claude Code can call the tool
- **Acceptance:** Configure in Claude Code, ask to rename a hook, observe success

**Milestone 6: Performance Validation (Days 17-18)**
- Benchmark suite (criterion for Rust, custom for WASM)
- **Targets:** Parse + single op < 50ms, parse + 5 ops < 80ms, 12 files < 500ms
- **Risk retired:** Performance is interactive-speed
- **Acceptance:** Benchmark table showing all targets met

### Week 4: Full Power

**Milestone 7: Cross-File Operations (Days 19-22)**
- File discovery in TS server (scan for symbol references)
- BatchRequest/BatchResponse protocol
- `rename_symbol` with `scope: "project"` across N files
- **Tests:** Rename across 12 files -- all modified correctly, all re-parse. String literals NOT renamed. Partial failure reported cleanly
- **Risk retired:** Multi-file operations are accurate and don't produce false positives
- **Acceptance:** Rename a hook across a project in one MCP call

**Milestone 8: Operation Composition (Days 23-25)**
- Multiple operations on same file in single call
- Edit merging with conflict detection
- **Tests:** 4 ops (rename + add_import + wrap_jsx + add_prop) on one file -- all compose. Conflicting ops → error, not corruption. Order independence
- **Risk retired:** Operations compose correctly; LLM can batch operations
- **Acceptance:** Single MCP call with 4 ops on one file, all succeed

---

## Testing Strategy (7 Layers)

### Layer 1: Soundness Property Tests (Nightly CI)
- **Property:** `forall valid_source, forall valid_operation: parse(apply(source, op)).error_count == 0`
- **Method:** Corpus-based mutation (80%) + grammar-guided generation (20%) via `proptest`
- **Volume:** 5,000 iterations per operation per language = 660,000 total
- **Duration:** ~55 min (8 cores)

### Layer 2: Fixture Tests (Every Commit)
- Before/after file pairs for every operation + every edge case
- **Coverage matrix:** empty files, CRLF, unicode, decorators, generics, async, deep nesting, large files
- **Volume:** 500+ fixture pairs (30+ per Tier 1 operation)
- **Duration:** ~30 sec

### Layer 3: Idempotency Tests (Every Commit)
- Operations that should be no-ops when applied twice (add existing import, make already-async function async)
- **Volume:** 1,000 iterations per idempotent operation
- **Duration:** ~3 min

### Layer 4: Commutativity Tests (Nightly CI)
- Independent operations produce same result regardless of order
- **Volume:** 2,000 iterations per independent pair (~100 pairs)
- **Duration:** ~17 min

### Layer 5: Real-World Corpus Tests (Weekly CI)
- **Projects:** next.js examples, shadcn/ui, cal.com, nuxt, elk, svelte examples, typescript-eslint
- **Method:** Parse every file, apply random valid operations, verify output parses
- **Volume:** ~5,900 files, ~30,000 operation applications
- **Duration:** ~25 min

### Layer 6: Performance Benchmarks (Every Commit)
- Criterion benchmarks for native Rust, separate suite for WASM
- Regression detection: fail build if any benchmark regresses >20%
- **Volume:** 66 benchmarks
- **Duration:** ~10 min

### Layer 7: MCP Integration Tests (Every Commit)
- Full MCP request/response cycle tests
- Schema validation, error format, concurrent operations, large payloads
- End-to-end scenarios (rename across project, extract component)
- **Volume:** 200+ test cases
- **Duration:** ~2 min

### Confidence Target
- Property tests: >99% confidence of <0.1% defect rate per operation-language pair
- Four independent test layers at 95%+ detection each → combined 99.999% detection rate
- Every historical bug permanently prevented via fixture regression tests

---

## Demo Scenarios (Proving Value)

### Demo 1: "Rename useAuth to useSession" (94% token reduction)
- 3 operations: `rename_symbol` + 2x `update_import_path`
- 27 edits across 8 files in 145ms
- Agent never reads any file, never generates modified source
- All comments/formatting byte-identical in untouched regions

### Demo 2: "Wrap ActivityFeed in Suspense + ErrorBoundary"
- 5 operations: 3x `add_import` + 2x `wrap_jsx`
- Imports merge intelligently (`Suspense` merges into existing `react` import)
- Correct indentation of nested wrappers

### Demo 3: "Extract JSX into new component" (hardest operation)
- 1 operation: `extract_component`
- Auto-computes props from free variables in extracted JSX
- Generates new file with props interface + moved imports
- Replaces original JSX with `<UserActivity activities={user?.activities} />`

---

## Files to Create/Modify

All new files (greenfield project):

| File | Purpose |
|------|---------|
| `Cargo.toml` | Workspace root |
| `crates/ast-surgeon-core/src/lib.rs` | Core API |
| `crates/ast-surgeon-core/src/edit.rs` | TextEdit + EditSet (the foundation) |
| `crates/ast-surgeon-core/src/format.rs` | Indentation inference |
| `crates/ast-surgeon-core/src/validate.rs` | Re-parse verification |
| `crates/ast-surgeon-core/src/operations/*.rs` | 22 operation implementations |
| `crates/ast-surgeon-lang/src/*.rs` | Language-specific queries |
| `crates/ast-surgeon-wasm/src/lib.rs` | WASM entry points |
| `packages/mcp-server/src/index.ts` | MCP server entry |
| `packages/mcp-server/src/handler.ts` | Request orchestration |
| `packages/mcp-server/src/wasm-bridge.ts` | WASM loading |
| `tests/fixtures/**` | 500+ before/after test pairs |

---

## Verification Plan

After each milestone, verify:
1. `cargo test` -- all Rust unit tests pass
2. `cargo test --target wasm32-wasi` -- WASM tests pass (from M4)
3. `npm test` -- TypeScript integration tests pass (from M5)
4. `cargo bench` -- performance targets met (from M6)
5. End-to-end: configure in Claude Code, perform a rename, verify file modified correctly (from M5)

After full implementation:
6. Run nightly CI (property tests + commutativity): all pass
7. Run weekly CI (real-world corpus): >99% success rate
8. Benchmark report shows all operations under 100ms (WASM, single file)
