# verify-pipe: Structured Verification Pipeline — Implementation Plan

## Context

The `fe-verify` crate has a working skeleton with 5 runners (ESLint, Biome, tsc, Jest, Vitest), tool detection, a cascading pipeline, and type definitions. The critical gap: **all three pipeline steps discard tool output** — they check exit codes but hardcode `errors: Vec::new()`. The pipeline runs tools but returns empty diagnostics.

This plan completes `fe_verify` into a production-ready MCP tool that returns structured JSON with parsed errors, fix hints, and affected-only test selection — replacing the 3-6 raw shell commands agents currently run with a single tool call.

---

## Phase 1: Output Parsers (Steps 1-5)

### Step 1: Create parser module + TypeScript parser

**Files to create:**
- `crates/fe-verify/src/parsers/mod.rs`
- `crates/fe-verify/src/parsers/typescript.rs`

**File to modify:**
- `crates/fe-verify/src/lib.rs` — add `pub mod parsers;`

**TypeScript parser design:**
- tsc with `--noEmit --pretty false` outputs: `file(line,col): error TSxxxx: message`
- Use `std::sync::LazyLock<Regex>` for the pattern (compiled once)
- Multi-line continuation errors (indented lines) are skipped — first line has all critical info
- Returns `StepResult` with populated `DiagnosticItem` vec

```rust
static TSC_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(.+?)\((\d+),(\d+)\):\s+(error|warning)\s+(TS\d+):\s+(.+)$").unwrap()
});

pub fn parse_tsc_output(stdout: &str) -> Result<StepResult, VerifyError> { ... }
```

**Unit tests (inline):** clean output, 3 errors, multi-line errors, Windows paths, empty input

**Verify:** `cargo test -p fe-verify -- parsers::typescript`

---

### Step 2: ESLint JSON parser

**File:** `crates/fe-verify/src/parsers/eslint.rs`

ESLint `--format json` produces an array of `{filePath, messages[], errorCount, warningCount}`.

```rust
#[derive(Deserialize)]
struct ESLintFileResult {
    #[serde(rename = "filePath")]
    file_path: String,
    messages: Vec<ESLintMessage>,
    #[serde(rename = "errorCount")]
    error_count: usize,
    #[serde(rename = "warningCount")]
    warning_count: usize,
}

pub fn parse_eslint_output(stdout: &str, project_root: &Path) -> Result<StepResult, VerifyError>
```

Key details:
- `severity`: 1=warning, 2=error (ESLint convention)
- `ruleId` can be `null` for fatal parse errors → `Option<String>`
- File paths converted to project-relative via `strip_prefix`

**Unit tests:** clean `[]`, errors+warnings across files, null ruleId parse error, invalid JSON

**Verify:** `cargo test -p fe-verify -- parsers::eslint`

---

### Step 3: Jest JSON parser

**File:** `crates/fe-verify/src/parsers/jest.rs`

Jest `--json` produces `{numTotalTests, numPassedTests, numFailedTests, testResults[{name, assertionResults[]}]}`.

Critical edge case: Jest may prefix JSON with console.log output. Parser uses `extract_json()` to find the first valid `{...}` JSON object.

```rust
pub fn parse_jest_output(stdout: &str, project_root: &Path) -> Result<TestStepResult, VerifyError>
```

- `ancestorTitles` joined with " > " for full test name
- Failure messages truncated to 500 chars (stack traces can be huge)
- File paths converted to relative

**Unit tests:** all-pass, failures with ancestor titles, prefixed JSON, invalid JSON

**Verify:** `cargo test -p fe-verify -- parsers::jest`

---

### Step 4: Vitest parser (thin delegate to Jest)

**File:** `crates/fe-verify/src/parsers/vitest.rs`

Vitest `--reporter json` is deliberately Jest-compatible. Reuse the Jest parser with error tool-name remapping.

```rust
pub fn parse_vitest_output(stdout: &str, project_root: &Path) -> Result<TestStepResult, VerifyError> {
    super::jest::parse_jest_output(stdout, project_root)
        .map_err(|e| match e {
            VerifyError::ParseError { message, .. } => VerifyError::ParseError {
                tool: "vitest".into(), message,
            },
            other => other,
        })
}
```

**Unit test:** 1 test verifying delegation works

**Verify:** `cargo test -p fe-verify -- parsers::vitest`

---

### Step 5: Biome JSON parser

**File:** `crates/fe-verify/src/parsers/biome.rs`

Biome `--reporter json` produces `{diagnostics[{category, severity, description, location{path{file}, span, sourceCode}}]}`.

Challenge: Biome reports byte-offset `span`, not line/column. Parser computes line/col from span + sourceCode when available, falls back to `(0, 0)`.

```rust
fn compute_line_col(span: Option<&(usize, usize)>, source: Option<&str>) -> (usize, usize)
```

**Unit tests:** clean output, errors with locations, missing source fallback

**Verify:** `cargo test -p fe-verify -- parsers::biome`

---

## Phase 2: Hint Generation (Step 6)

### Step 6: Hint system for fix suggestions

**File:** `crates/fe-verify/src/parsers/hints.rs`

Maps error codes/rules to actionable suggestions that point to `fe_doctor`:

```rust
pub fn generate_lint_hint(rule: Option<&str>, message: &str) -> Option<String>
pub fn generate_tsc_hint(code: &str, message: &str) -> Option<String>
```

Coverage for the most common errors:
- **React hooks:** `react-hooks/exhaustive-deps`, `react-hooks/rules-of-hooks`
- **TypeScript:** `TS2304` (cannot find name), `TS2345` (type mismatch), `TS2322` (assignment), `TS2339` (property missing), `TS7006` (implicit any), `TS2307` (module not found), `TS18048` (possibly undefined)
- **Lint:** `no-unused-vars`, `import/no-unresolved`, `@typescript-eslint/*`, auto-fixable formatting rules
- **Biome:** `lint/suspicious/noDoubleEquals`, `lint/*/noExplicitAny`
- **Default fallback:** "Call fe_doctor with this error for structured fix instructions."

**Unit tests:** specific rules produce expected hints, unknown rules get default

**Verify:** `cargo test -p fe-verify -- parsers::hints`

---

## Phase 3: Pipeline Integration (Steps 7-8)

### Step 7: Wire parsers into pipeline.rs

**File to modify:** `crates/fe-verify/src/pipeline.rs`

Replace the 3 TODO sites with parser dispatch:

```rust
// Step 1: Lint — dispatch by runner name
let tool_name = linter.name().to_string();
let result = linter.run(project_root, affected_files).await?;
summary.lint = parse_lint_output(&tool_name, &result.stdout, result.exit_code, project_root)?;

// Step 2: TypeCheck — always parse when exit_code != 0
let types_result = if result.exit_code != 0 {
    parsers::typescript::parse_tsc_output(&result.stdout)?
} else {
    StepResult::pass()
};

// Step 3: Tests — dispatch by runner name
summary.tests = parse_test_output(&tool_name, &result.stdout, result.exit_code, project_root)?;
```

Dispatch functions route to correct parser based on `runner.name()`:
- `"eslint"` → `parsers::eslint::parse_eslint_output()`
- `"biome"` → `parsers::biome::parse_biome_output()`
- `"jest"` → `parsers::jest::parse_jest_output()`
- `"vitest"` → `parsers::vitest::parse_vitest_output()`

**Also modify:** `crates/fe-verify/src/types.rs`
- Add `pub status: String` field to `VerificationSummary` (with `Default` giving `""`)
- Add `pub fn compute_status(&mut self)` that sets "pass"/"fail" from step results
- Pipeline calls `summary.compute_status()` before returning

**Also add** to pipeline: `pub fn with_runners(linter, type_checker, test_runner) -> Self` constructor for testing.

**Verify:** `cargo test -p fe-verify`

---

### Step 8: Integration test with mock runners

**File to create:** `crates/fe-verify/tests/pipeline_integration.rs`

Uses a `MockRunner` implementing `VerificationRunner` that returns pre-canned output:

Test scenarios:
1. **All pass** — lint clean + tsc clean + tests pass → `status: "pass"`
2. **Lint fail cascade** — lint errors → types skipped, tests skipped
3. **Type fail cascade** — lint pass, tsc errors → tests skipped
4. **Test failures** — lint pass, tsc pass, 2 test failures → `status: "fail"`
5. **No tools detected** — empty pipeline → all steps skipped
6. **JSON serialization matches target format** — serialize `VerificationSummary` and verify structure

**Also modify:** `crates/fe-verify/Cargo.toml` — add `proptest = "1"` and `async-trait = "0.1"` to `[dev-dependencies]`

**Property-based test:** TSC parser never panics on arbitrary input; error count matches diagnostic count for generated inputs.

**Verify:** `cargo test -p fe-verify --test pipeline_integration`

---

## Phase 4: MCP Tool Exposure (Steps 9-10)

### Step 9: MCP tool wrapper

**Files to create:**
- `crates/fe-mcp-server/src/context.rs` — `ProjectContext { root, detected_tools }`
- `crates/fe-mcp-server/src/tools/mod.rs` — `Tool` trait, `ToolRegistry`, `register_all()`
- `crates/fe-mcp-server/src/tools/verify.rs` — `VerifyTool` implementing `Tool` trait

**Tool input schema:**
```json
{
  "type": "object",
  "properties": {
    "files": { "type": "array", "items": {"type": "string"}, "description": "Files to verify (relative). Omit for all." },
    "checks": { "type": "array", "items": {"enum": ["lint","types","tests","all"]}, "default": ["all"] },
    "fix": { "type": "boolean", "default": false, "description": "Auto-fix lint issues." }
  }
}
```

**Tool description** (critical — this is what the LLM reads to decide when to call it):
> "Verify frontend code changes: runs lint (ESLint/Biome), typecheck (tsc), and affected tests in one call. Returns structured JSON — NOT terminal output. Use this INSTEAD of running 'npm run lint', 'npx tsc', or 'npm test' separately."

**File to modify:** `crates/fe-mcp-server/Cargo.toml` — add `async-trait = "0.1"`

**Verify:** `cargo build -p fe-tools`

---

### Step 10: MCP server wiring + E2E proof

**Files to modify:**
- `crates/fe-mcp-server/src/main.rs` — wire `ProjectContext::discover()` and `ToolRegistry`
- `crates/fe-mcp-server/src/server.rs` — implement MCP JSON-RPC handler over stdio

MCP server approach: Start with hand-rolled JSON-RPC (read stdin line-by-line, dispatch `tools/list` and `tools/call`, write JSON-RPC response to stdout). This is ~50 lines and avoids an external SDK dependency for the POC. Upgrade to `rmcp` in a follow-up.

**E2E verification:**
```bash
# List tools
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | cargo run --bin fe-tools -- serve --project-root /path/to/real/project

# Call fe_verify
echo '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"fe_verify","arguments":{"files":["src/App.tsx"]}}}' | cargo run --bin fe-tools -- serve --project-root /path/to/real/project
```

---

## File Manifest

### New files (12)
| File | Purpose |
|------|---------|
| `crates/fe-verify/src/parsers/mod.rs` | Parser module root (`pub mod eslint; biome; typescript; jest; vitest; hints;`) |
| `crates/fe-verify/src/parsers/eslint.rs` | ESLint `--format json` parser |
| `crates/fe-verify/src/parsers/biome.rs` | Biome `--reporter json` parser |
| `crates/fe-verify/src/parsers/typescript.rs` | tsc `--pretty false` regex parser |
| `crates/fe-verify/src/parsers/jest.rs` | Jest `--json` parser |
| `crates/fe-verify/src/parsers/vitest.rs` | Vitest parser (delegates to Jest) |
| `crates/fe-verify/src/parsers/hints.rs` | Error code → fix suggestion mapping |
| `crates/fe-verify/tests/pipeline_integration.rs` | Full pipeline integration tests with mock runners |
| `crates/fe-mcp-server/src/context.rs` | `ProjectContext` for MCP server |
| `crates/fe-mcp-server/src/tools/mod.rs` | `Tool` trait + `ToolRegistry` |
| `crates/fe-mcp-server/src/tools/verify.rs` | `VerifyTool` MCP wrapper |

### Files to modify (6)
| File | Change |
|------|--------|
| `crates/fe-verify/src/lib.rs` | Add `pub mod parsers;` |
| `crates/fe-verify/src/types.rs` | Add `status` field to `VerificationSummary`, add `compute_status()` |
| `crates/fe-verify/src/pipeline.rs` | Replace 3 TODOs with parser calls; add `with_runners()` constructor |
| `crates/fe-verify/Cargo.toml` | Add `proptest = "1"` to `[dev-dependencies]` |
| `crates/fe-mcp-server/src/main.rs` | Wire ProjectContext + ToolRegistry |
| `crates/fe-mcp-server/src/server.rs` | Implement MCP JSON-RPC over stdio |

### Existing code reused (no changes needed)
| File | What we reuse |
|------|--------------|
| `crates/fe-verify/src/detection.rs` | `detect_tools()`, `DetectedTools`, `LinterKind`, `TestRunnerKind` |
| `crates/fe-verify/src/runners/*.rs` | All 5 runners (ESLint, Biome, tsc, Jest, Vitest) |
| `crates/fe-verify/src/error.rs` | `VerifyError` enum |
| `crates/fe-common/src/project.rs` | `find_project_root()` |
| `crates/fe-common/src/fs_utils.rs` | `resolve_within_root()` |

---

## Testing Summary

| Test Type | Count | What's Tested |
|-----------|-------|---------------|
| **Parser unit tests** (inline `#[cfg(test)]`) | ~25 | Each parser with real sample output: clean, errors, edge cases, invalid input |
| **Hint unit tests** | ~8 | Specific rules → expected suggestions, unknown rules → default |
| **Pipeline integration** (mock runners) | 6 | Cascading (lint-fail, type-fail), all-pass, all-skip, JSON serialization |
| **Property-based** (proptest) | 2 | TSC parser never panics; error count matches diagnostic count |
| **E2E** (`#[ignore]`, needs Node.js) | 1 | Full pipeline against real broken-app fixture |
| **Total** | **~42** | |

## Verification (how to prove it works end-to-end)

```bash
# 1. All parser unit tests pass
cargo test -p fe-verify -- parsers

# 2. Pipeline integration tests pass
cargo test -p fe-verify --test pipeline_integration

# 3. Full crate compiles with no warnings
cargo build -p fe-verify -p fe-tools

# 4. MCP server starts and responds to tools/list
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | cargo run --bin fe-tools -- serve

# 5. Serialized output matches target format (checked in integration test)
cargo test -p fe-verify --test pipeline_integration -- json_format
```

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Biome JSON format varies between versions | All fields `Option<T>`; fallback `(0,0)` for line/col |
| Jest prefixes JSON with console output | `extract_json()` scans for first valid `{...}` |
| tsc multi-line errors | Regex anchored to `^`; continuation lines don't match |
| rmcp SDK immaturity | Start with hand-rolled JSON-RPC (~50 lines); upgrade later |
| Large test suites → huge JSON | Truncate failure messages to 500 chars |
