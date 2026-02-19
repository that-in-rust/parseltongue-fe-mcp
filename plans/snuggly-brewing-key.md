# fe.batch — Atomic Multi-File Edit Tool Implementation Plan

## Context

Parseltongue Frontend MCP is a suite of 9 MCP tools for AI coding agents. Currently only specification docs exist (`09-competitive-landscape.md`, `09-integration-and-gtm.md`) — no implementation code. `fe.batch` is identified as the **strongest market gap**: no existing tool (Claude Code, Cursor, Copilot, Aider, Codex) provides true transactional semantics for multi-file edits. This plan builds the complete `fe.batch` tool from scratch in Rust, exposed as an MCP server.

---

## Architecture

### Cargo Workspace Layout

```
parseltongue-fe-mcp/
├── Cargo.toml                          # Workspace root
├── crates/
│   ├── fe-mcp-server/                  # Binary: MCP server entrypoint (stdio transport)
│   │   └── src/ { main.rs, server.rs }
│   ├── fe-batch/                       # Library: transaction engine
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── types.rs                # BatchInput, BatchResult, EditOperation, etc.
│   │       ├── error.rs                # BatchError enum (thiserror)
│   │       ├── edit_set.rs             # Input validation (paths, duplicates, traversal)
│   │       ├── file_ops.rs             # atomic_write(), FileBackupSet, restore_all()
│   │       ├── staging.rs              # StagingArea (tempdir shadow directory)
│   │       ├── transaction.rs          # Transaction<State> typestate engine
│   │       ├── verification.rs         # Wire to fe-verify pipeline
│   │       └── cross_validation.rs     # Import/export consistency (tree-sitter)
│   ├── fe-verify/                      # Library: lint/types/tests pipeline
│   │   └── src/
│   │       ├── lib.rs, types.rs, error.rs
│   │       ├── detection.rs            # Auto-detect ESLint/Biome/tsc/Jest/Vitest
│   │       ├── pipeline.rs             # Cascading: lint → types → tests (early termination)
│   │       └── runners/ { eslint.rs, biome.rs, typescript.rs, jest.rs, vitest.rs }
│   └── fe-common/                      # Library: shared utilities
│       └── src/ { lib.rs, project.rs, config.rs, fs_utils.rs }
```

### Core Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Atomic writes | `tempfile::NamedTempFile` in same dir → `persist()` (rename) | `rename()` is atomic on POSIX/NTFS within same filesystem |
| Multi-file atomicity | Backup set + rollback on failure | Software transactional memory — sufficient for dev tooling |
| Verification | Spawn external processes with JSON flags | Users already have tools installed; avoids binary bloat |
| Transaction lifecycle | Typestate pattern (`Pending→Staged→Applied→Committed/RolledBack`) | Compile-time enforcement of correct state transitions |
| Cross-file validation | `tree-sitter` import/export extraction | Fast pre-check before expensive tsc; catches 80% of issues |

### Transaction Lifecycle

```
BatchInput → Transaction<Pending>
  │  validate paths, check existence, no duplicates
  ▼
Transaction<Staged>
  │  write to shadow tempdir, cross-file validation
  ▼
Transaction<Applied>
  │  backup originals, atomic_write to working dir
  ├──verify()──→ PASS → commit() → Transaction<Committed>
  │                                   (discard backups, return success)
  └──verify()──→ FAIL → rollback() → Transaction<RolledBack>
                                       (restore from backups, delete created files)
```

---

## Implementation Phases

### Phase 1: Core Transaction Engine (Week 1-2)

**Goal:** Atomic all-or-nothing file operations without verification.

**Files to create:**
- `Cargo.toml` (workspace)
- `crates/fe-common/` — `project.rs` (find project root via package.json/tsconfig.json), `fs_utils.rs` (path canonicalization, `is_within_root()`)
- `crates/fe-batch/src/types.rs` — `BatchInput`, `BatchResult`, `EditOperation`, `CreateOperation`, `BatchStatus`
- `crates/fe-batch/src/error.rs` — `BatchError` enum with variants: `FileNotFound`, `FileAlreadyExists`, `PathTraversal`, `DuplicatePath`, `EmptyTransaction`, `WriteError`, `BackupError`, `RollbackError`
- `crates/fe-batch/src/edit_set.rs` — validation: files exist for edits, don't exist for creates, paths within root, no duplicates, exactly one of content/operations per edit
- `crates/fe-batch/src/file_ops.rs` — `atomic_write()` (NamedTempFile → sync_all → persist), `FileBackupSet` (backup, record_creation, restore_all, discard)
- `crates/fe-batch/src/staging.rs` — `StagingArea` wrapping `TempDir`, mirrors project structure
- `crates/fe-batch/src/transaction.rs` — `Transaction<State>` with PhantomData typestate: `new()→stage()→apply()→commit()/rollback()`

**Key detail — `atomic_write()`:**
```rust
// Create temp file in SAME directory as target (ensures same-filesystem rename)
let temp = NamedTempFile::new_in(target.parent())?;
temp.write_all(content)?;
temp.as_file().sync_all()?;
temp.persist(target)?;  // atomic rename
```

**Key detail — `FileBackupSet::restore_all()`:**
- Restore in REVERSE order of application
- First: delete created files + empty ancestor dirs
- Then: copy backup content back to original paths, restore permissions

### Phase 2: Verification Pipeline (Week 2-3)

**Goal:** Detect project tooling, run lint/types/tests, parse structured output.

**Files to create:**
- `crates/fe-verify/src/detection.rs` — scan for config files: `biome.json`→Biome, `.eslintrc.*`/`eslint.config.*`→ESLint, `tsconfig.json`→tsc, `vitest.config.*`→Vitest, `jest.config.*`→Jest. Fallback to `node_modules/.bin/` and `which()`.
- `crates/fe-verify/src/runners/*.rs` — each runner implements `VerificationRunner` trait:
  - `async fn run(&self, project_root, files) → RunnerOutput`
  - Parse JSON output into unified diagnostic types
- `crates/fe-verify/src/pipeline.rs` — cascading pipeline with early termination:
  - lint errors → skip types and tests
  - type errors → skip tests
  - Returns `VerificationSummary { lint, types, tests }`

**JSON output flags per tool:**

| Tool | Flag | Output |
|------|------|--------|
| ESLint | `--format json` | `[{filePath, messages: [{line, column, message, ruleId, severity}]}]` |
| Biome | `--reporter json` | `{diagnostics: [{file, severity, message, category}]}` |
| tsc | `--pretty false` | Regex-parsed: `file(line,col): error TSxxxx: message` |
| Jest | `--json --forceExit` | `{numTotalTests, numPassedTests, testResults: [...]}` |
| Vitest | `--reporter json` | Jest-compatible format |

### Phase 3: MCP Server Integration (Week 3-4)

**Goal:** Expose `fe_batch` as a callable MCP tool via stdio transport.

**Files to create:**
- `crates/fe-mcp-server/src/main.rs` — `clap` CLI: `fe-tools serve --project-root . --framework auto`
- `crates/fe-mcp-server/src/server.rs` — `FeMcpServer` struct with `#[tool_box]` impl, `#[tool(name = "fe_batch")]` handler

**MCP handler flow:**
```
input JSON → Transaction::new() → stage() → apply() → verify()
  → PASS: commit() → return {status: "success", verification: {...}}
  → FAIL + rollback_on_failure: rollback() → return {status: "rolled_back", ...}
  → FAIL + !rollback_on_failure: commit() → return {status: "verification_failed", ...}
```

**Key dependencies:** `rmcp` (MCP SDK with `server`, `macros`, `transport-io` features), `schemars` (JSON Schema from types), `clap` (CLI)

### Phase 4: Cross-File Validation (Week 4-5)

**Goal:** Fast pre-check before applying — catch broken imports/exports without running tsc.

**File:** `crates/fe-batch/src/cross_validation.rs`

**Approach:** Use `tree-sitter` + `tree-sitter-typescript` to parse staged files, extract import/export declarations, build lightweight symbol table, cross-reference. Catches the 80% case (broken imports, missing exports) in milliseconds vs seconds for full tsc.

**Behind feature flag:** `cross-validation = ["tree-sitter", "tree-sitter-typescript"]`

### Phase 5: AST Operations Integration (Week 5-6)

**Goal:** Support `operations` field in EditOperation (connection to fe.surgeon).

**Integration point:** During `stage()`, if an edit has `operations` instead of `content`:
1. Read original file
2. Apply AST operations via `fe_surgeon::apply_operations()`
3. Stage the resulting content

**Behind feature flag:** `ast-operations = ["fe-surgeon"]`

---

## Proof of Concept — 3 Deterministic Scenarios

### Scenario A: Atomicity on I/O Failure
```
Setup:  3 TypeScript files, make middle one read-only (chmod 0444)
Action: fe_batch edits all 3
Expect: First file rolled back, third never touched
Result: { status: "error", rolled_back: true }
```

### Scenario B: Verification-Triggered Rollback
```
Setup:  React component + consumer file, tsconfig.json, TypeScript installed
Action: fe_batch renames a prop in component but NOT in consumer (verify: true)
Expect: tsc fails on consumer, ALL changes rolled back
Result: { status: "rolled_back", verification: { types: { status: "fail" } } }
```

### Scenario C: Successful Atomic Create
```
Setup:  Next.js project with ESLint + tsc + Vitest
Action: fe_batch creates ProductCard.tsx + test + story (verify: true)
Expect: All 3 created, all verification passes
Result: { status: "success", files_created: 3, verification: { lint: "pass", types: "pass", tests: { ran: N, passed: N } } }
```

---

## Testing Strategy

### Unit Tests (per module, ~40 tests)

**edit_set.rs (8 tests):**
- `test_validate_edits_with_existing_files` — happy path
- `test_validate_edits_file_not_found` — error
- `test_validate_creates_file_already_exists` — error
- `test_validate_path_traversal_blocked` — `../../../etc/passwd` rejected
- `test_validate_duplicate_paths_rejected`
- `test_validate_ambiguous_edit_rejected` — both content + operations
- `test_validate_empty_transaction_rejected`
- `test_validate_creates_with_nested_dirs`

**file_ops.rs (9 tests):**
- `test_atomic_write_creates_file`
- `test_atomic_write_replaces_existing`
- `test_atomic_write_preserves_on_failure`
- `test_backup_and_restore_single_file`
- `test_backup_and_restore_multiple_files`
- `test_backup_preserves_permissions`
- `test_record_creation_and_rollback` — created file deleted
- `test_rollback_removes_empty_parent_dirs`
- `test_discard_cleans_up_backup_dir`

**staging.rs (4 tests):**
- `test_staging_area_create_and_read`
- `test_staging_area_multiple_files`
- `test_staging_area_nested_paths`
- `test_staging_area_cleanup_on_drop`

**transaction.rs (7 tests):**
- `test_transaction_new_validates_input`
- `test_transaction_stage_writes_to_staging`
- `test_transaction_apply_creates_backups`
- `test_transaction_apply_writes_to_working_dir`
- `test_transaction_commit_discards_backups`
- `test_transaction_rollback_restores_files`
- `test_transaction_rollback_removes_created_files`

### Integration Tests (`crates/fe-batch/tests/`, ~8 tests)

- `test_edit_three_files_atomically` — happy path
- `test_create_three_files_atomically`
- `test_mixed_edits_and_creates`
- `test_rollback_after_partial_apply_failure` — read-only file mid-batch
- `test_rollback_preserves_original_content_exactly` — byte-for-byte
- `test_rollback_after_create_removes_files_and_dirs`
- `test_unicode_filenames`
- `test_large_file_handling` — 10MB file

### Property-Based Tests (`proptest`, ~3 strategies)

```rust
// 1. Rollback ALWAYS restores original state
prop_rollback_always_restores_original_state(file_contents, edits)
  → apply edits, rollback, assert byte-for-byte match

// 2. Committed state ALWAYS matches staged content
prop_committed_state_matches_staged_content(edits)
  → apply, commit, assert content matches

// 3. No partial state on failure
prop_no_partial_state_on_failure(file_count, fail_at_index)
  → make one file read-only, assert ALL original or ALL new (never mixed)
```

### Failure Injection Tests (~4 tests, mock runners)

- `test_lint_failure_triggers_rollback`
- `test_type_check_failure_triggers_rollback`
- `test_test_failure_triggers_rollback`
- `test_verification_timeout_triggers_rollback`

### E2E Tests (gated behind `--ignored`, require Node.js)

- `e2e_react_component_creation_with_real_verification`
- `e2e_type_error_causes_rollback`

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Process killed mid-apply | Deterministic backup dir path (keyed on txn ID); detect stale backups on next run |
| Disk full during backup | Check available space before starting; `backup_file()` fails early |
| Verification tool crashes | Non-zero exit + unparseable output = verification failure → rollback |
| Symlinks escape project root | `canonicalize()` all paths, verify resolved path is within root |
| Cross-filesystem rename | Detect via `stat().st_dev` comparison; warn or reject |
| macOS case-insensitive FS | Path validation accounts for `Foo.tsx` == `foo.tsx` |
| Windows file locking | Clear error messages when editor holds a file open |
| File modified between backup and write | Record mtime at backup, check before commit, warn if changed |

---

## Key Dependencies

```toml
[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
thiserror = "2"
tempfile = "3"
tracing = "0.1"

# fe-mcp-server
rmcp = { version = "0.15", features = ["server", "macros", "transport-io"] }
schemars = "1"
clap = { version = "4", features = ["derive"] }

# fe-verify
which = "7"
async-trait = "0.1"
regex = "1"

# fe-batch (optional features)
tree-sitter = { version = "0.24", optional = true }
tree-sitter-typescript = { version = "0.23", optional = true }

# dev-dependencies
proptest = "1"
assert_fs = "1"
predicates = "3"
```

---

## Verification Plan

1. **After Phase 1:** `cargo test -p fe-batch` — all unit + integration + property tests pass
2. **After Phase 2:** `cargo test -p fe-verify` — runner tests with mock JSON output
3. **After Phase 3:** Manual test with Claude Code MCP config — call `fe_batch` from Claude Code, verify structured JSON response
4. **After Phase 4:** `cargo test -p fe-batch --features cross-validation` — import/export consistency tests
5. **After Phase 5:** Full E2E — `cargo test -- --ignored` with a real React project
