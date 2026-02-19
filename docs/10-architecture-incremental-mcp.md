# Architecture: Incremental MCP Tool Development

> How to ship 2-3 tools as a proof of concept, then keep adding tools without touching core infrastructure.

---

## Design Principle

**One tool = one file. Adding a tool never requires changing the server, the router, or any other tool.**

The MCP server is a thin shell. Tools are self-contained modules that register themselves into a shared registry. Shared infrastructure (AST parsing, process running, project detection) is extracted only when a second tool needs it — not pre-built speculatively.

---

## Project Structure

```
fe-tools/
├── Cargo.toml
├── src/
│   ├── main.rs                  # Entry point: parse CLI args, init server
│   ├── server.rs                # MCP JSON-RPC handler, stdio transport
│   ├── registry.rs              # Tool registry + request routing
│   ├── types.rs                 # Shared result types (VerifyResult, SkeletonNode, etc.)
│   │
│   ├── tools/                   # Each tool is a self-contained module
│   │   ├── mod.rs               # Tool trait definition + register_all()
│   │   ├── verify.rs            # fe_verify   ← POC tool 1
│   │   ├── skeleton.rs          # fe_skeleton ← POC tool 2
│   │   ├── doctor.rs            # fe_doctor   ← POC tool 3
│   │   │
│   │   │  # --- Added incrementally after POC ---
│   │   ├── impact.rs            # fe_impact
│   │   ├── scaffold.rs          # fe_scaffold
│   │   ├── surgeon.rs           # fe_surgeon
│   │   ├── diff.rs              # fe_diff
│   │   ├── batch.rs             # fe_batch
│   │   └── budget.rs            # fe_budget
│   │
│   └── shared/                  # Infrastructure extracted when 2+ tools need it
│       ├── mod.rs
│       ├── project.rs           # Project discovery (package.json, tsconfig, framework)
│       ├── runner.rs            # Subprocess runner (lint, tsc, test commands)
│       ├── ast.rs               # tree-sitter parsing (extracted when skeleton + surgeon both need it)
│       ├── token_counter.rs     # Token estimation (extracted when skeleton + budget both need it)
│       └── output_parser.rs     # Parse tsc/eslint/jest output into structured errors
│
├── tests/
│   ├── fixtures/                # Tiny real frontend projects for integration tests
│   │   ├── next-app/            # Next.js 14, ~5 components, TS, Tailwind
│   │   ├── vite-react/          # Vite + React, ~5 components, TS
│   │   └── broken-app/          # Intentionally broken project (lint errors, type errors, failing tests)
│   ├── verify_test.rs
│   ├── skeleton_test.rs
│   └── doctor_test.rs
│
└── README.md
```

---

## Core Interface: The `Tool` Trait

Every tool implements this trait. The server doesn't know or care what any tool does internally.

```rust
use async_trait::async_trait;
use serde_json::Value;

/// The result a tool returns to the MCP client (the agent).
pub struct ToolResult {
    pub content: Value,       // Structured JSON the LLM reads
    pub is_error: bool,       // MCP isError flag
}

/// Shared project knowledge, initialized once at server startup.
pub struct ProjectContext {
    pub root: PathBuf,
    pub framework: Framework,          // React | Next | Vue | Svelte | Astro
    pub package_manager: PackageManager, // npm | pnpm | yarn | bun
    pub tsconfig_path: Option<PathBuf>,
    pub lint_tool: LintTool,           // ESLint | Biome
    pub test_runner: TestRunner,       // Jest | Vitest | Playwright
}

#[async_trait]
pub trait Tool: Send + Sync {
    /// MCP tool name. E.g., "fe_verify".
    fn name(&self) -> &str;

    /// MCP tool description. The LLM reads this to decide when to call the tool.
    /// This is the most important string in the entire system.
    fn description(&self) -> &str;

    /// JSON Schema for the tool's inputSchema (what params the LLM can pass).
    fn input_schema(&self) -> Value;

    /// Execute the tool. Receives the LLM's params + shared project context.
    async fn execute(&self, params: Value, ctx: &ProjectContext) -> Result<ToolResult>;
}
```

### Why This Interface Is Sufficient

| Concern | How the trait handles it |
|---------|------------------------|
| **Discovery** | `name()` + `description()` + `input_schema()` map directly to MCP `tools/list` response fields. |
| **Execution** | `execute()` receives arbitrary JSON params, returns arbitrary JSON. No coupling between tools. |
| **Shared state** | `ProjectContext` is the only shared dependency — and it's read-only after init. |
| **Composability** | Tools can call each other directly via function calls (same process), not MCP round-trips. E.g., `fe_batch` calls `verify::execute()` internally. |
| **Testing** | Each tool can be tested in isolation by constructing a `ProjectContext` pointing at a fixture directory. |

---

## Tool Registry & Routing

```rust
// registry.rs

pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self { tools: HashMap::new() }
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    /// MCP tools/list response — returns all registered tools.
    pub fn list(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(|t| ToolDefinition {
            name: t.name().to_string(),
            description: t.description().to_string(),
            input_schema: t.input_schema(),
        }).collect()
    }

    /// MCP tools/call routing — dispatch to the right tool.
    pub async fn call(
        &self,
        name: &str,
        params: Value,
        ctx: &ProjectContext,
    ) -> Result<ToolResult> {
        let tool = self.tools.get(name)
            .ok_or_else(|| anyhow!("Unknown tool: {}", name))?;
        tool.execute(params, ctx).await
    }
}
```

### Registration — Where Incremental Addition Happens

```rust
// tools/mod.rs

pub fn register_all(registry: &mut ToolRegistry) {
    // --- POC (ship first) ---
    registry.register(Box::new(verify::VerifyTool::new()));
    registry.register(Box::new(skeleton::SkeletonTool::new()));
    registry.register(Box::new(doctor::DoctorTool::new()));

    // --- Wave 2 (add after POC proves out) ---
    // registry.register(Box::new(impact::ImpactTool::new()));
    // registry.register(Box::new(scaffold::ScaffoldTool::new()));

    // --- Wave 3 (add after shared/ast.rs is mature) ---
    // registry.register(Box::new(surgeon::SurgeonTool::new()));
    // registry.register(Box::new(batch::BatchTool::new()));

    // --- Wave 4 ---
    // registry.register(Box::new(diff::DiffTool::new()));
    // registry.register(Box::new(budget::BudgetTool::new()));
}
```

**Adding a tool is:**
1. Create `tools/new_tool.rs`, implement the `Tool` trait.
2. Add one `registry.register(...)` line in `tools/mod.rs`.
3. Done.

### Optional: Feature Flags for Experimental Tools

```rust
pub fn register_all(registry: &mut ToolRegistry, config: &ServerConfig) {
    // Always available
    registry.register(Box::new(verify::VerifyTool::new()));
    registry.register(Box::new(skeleton::SkeletonTool::new()));
    registry.register(Box::new(doctor::DoctorTool::new()));

    if config.enable_experimental {
        registry.register(Box::new(surgeon::SurgeonTool::new()));
    }
}
```

This lets users opt into unstable tools via CLI flag (`fe-tools serve --experimental`) without risking POC stability.

---

## POC Tool Selection: verify → skeleton → doctor

### Why These Three

| Tool | Rationale |
|------|-----------|
| **`fe_verify`** | Highest call frequency — every agent task ends with verification. Simplest to implement (shell out to lint/tsc/test, parse output to structured JSON). Immediate, demonstrable value over raw terminal output. |
| **`fe_skeleton`** | Proves the "understand codebase without reading every file" value prop. Requires tree-sitter, which forces building `shared/ast.rs` early — needed by surgeon, impact, diff later. Sets up the shared infra investment. |
| **`fe_doctor`** | Completes the verify→doctor tool chain. When verify returns errors, doctor diagnoses them with structured fix instructions. Demonstrates **tool chaining** — the hardest thing to prove works with LLM agents. |

### What These Three Cover

The most common agent loop:

```
understand (skeleton) → write code → verify → diagnose (doctor) → fix → verify
```

This is the loop an agent runs on >80% of frontend tasks. If the POC proves this loop works well, the remaining tools (impact, surgeon, batch, etc.) are extensions that make specific parts of the loop faster.

### What Each POC Tool Needs from `shared/`

```
                  shared/project.rs   shared/runner.rs   shared/ast.rs   shared/output_parser.rs
                  (project detect)    (run commands)     (tree-sitter)   (parse tsc/eslint/jest)
                  ─────────────────   ────────────────   ─────────────   ───────────────────────
fe_verify              ✓                    ✓                              ✓
fe_skeleton            ✓                                      ✓
fe_doctor              ✓                                                   ✓
```

This means the POC forces building exactly 4 shared modules — all of which are reused by every later tool.

---

## Shared Infrastructure: Build Order

Only build shared modules when a second tool needs them. For the POC:

### 1. `shared/project.rs` — Built First (All 3 POC Tools Need It)

Discovers project configuration at startup:

```rust
pub struct ProjectContext {
    pub root: PathBuf,
    pub framework: Framework,
    pub package_manager: PackageManager,
    pub tsconfig_path: Option<PathBuf>,
    pub lint_tool: LintTool,
    pub test_runner: TestRunner,
    pub src_dir: PathBuf,              // "src/" or "app/" etc.
}

pub enum Framework { React, NextJs, Vue, Nuxt, Svelte, SvelteKit, Astro, Unknown }
pub enum PackageManager { Npm, Pnpm, Yarn, Bun }
pub enum LintTool { ESLint, Biome, None }
pub enum TestRunner { Jest, Vitest, Playwright, None }
```

Detection logic: read `package.json` dependencies to determine framework, check for lock files to determine package manager, check for config files (`biome.json`, `.eslintrc.*`, `vitest.config.*`, `jest.config.*`).

### 2. `shared/runner.rs` — Built for verify

Runs subprocesses and captures output:

```rust
pub struct CommandResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub duration_ms: u64,
}

pub async fn run_command(cmd: &str, args: &[&str], cwd: &Path) -> Result<CommandResult>;
```

### 3. `shared/output_parser.rs` — Built for verify, Reused by doctor

Parses raw CLI output into structured errors:

```rust
pub struct ParsedError {
    pub tool: String,         // "eslint", "tsc", "jest"
    pub code: Option<String>, // "TS2345", "react-hooks/exhaustive-deps"
    pub message: String,
    pub file: PathBuf,
    pub line: usize,
    pub column: usize,
    pub severity: Severity,   // Error | Warning | Info
}

pub fn parse_tsc_output(raw: &str) -> Vec<ParsedError>;
pub fn parse_eslint_output(raw: &str) -> Vec<ParsedError>;
pub fn parse_jest_output(raw: &str) -> TestResult;
```

### 4. `shared/ast.rs` — Built for skeleton

Tree-sitter integration. Initially just extracts exports and signatures:

```rust
pub struct FileSymbols {
    pub exports: Vec<ExportedSymbol>,
    pub imports: Vec<ImportStatement>,
}

pub struct ExportedSymbol {
    pub name: String,
    pub kind: SymbolKind,        // Function | Component | Hook | Type | Const
    pub signature: String,       // "({ userId }: Props) => JSX.Element"
    pub line: usize,
}

pub fn extract_symbols(source: &str, language: Language) -> Result<FileSymbols>;
```

This starts minimal for skeleton. When surgeon arrives later, it grows to support mutations (add_import, rename_symbol, etc.). The interface is designed so skeleton's read-only usage doesn't change when mutation methods are added.

---

## Tool Chaining: How Responses Guide the Agent

The most important architectural decision beyond the trait itself: **tool responses include hints that teach the LLM which tool to call next.**

### verify → doctor chain

```jsonc
// fe_verify returns:
{
  "status": "fail",
  "lint": {
    "status": "fail",
    "errors": [
      {
        "rule": "react-hooks/exhaustive-deps",
        "message": "Missing dependency 'userId' in useEffect",
        "file": "src/components/UserProfile.tsx",
        "line": 12,
        "suggestion": "Call fe_doctor with this error for structured fix instructions"
      }
    ]
  }
}

// fe_doctor returns:
{
  "diagnosis": "useEffect missing dependency",
  "root_cause": "userId is referenced inside effect but not in dependency array",
  "fix": {
    "description": "Add userId to useEffect dependency array",
    "file": "src/components/UserProfile.tsx",
    "line": 12,
    "operation": "Add 'userId' to the dependency array at line 12"
    // When fe_surgeon exists, this becomes:
    // "surgeon_op": { "op": "add_hook_dependency", "hook": "useEffect", "dependency": "userId" }
  }
}
```

### Design rule: Every error response includes a `suggestion` field pointing to the next tool in the chain.

This pattern extends naturally as tools are added:
- **doctor → surgeon**: "Apply this fix using fe_surgeon"
- **impact → batch**: "Use fe_batch to apply changes to all affected files"
- **skeleton → budget**: "Call fe_budget for optimized context allocation"

---

## Incremental Shipping Plan

### Wave 1: POC (Prove the concept works)

**Tools:** `fe_verify`, `fe_skeleton`, `fe_doctor`
**Shared infra:** `project.rs`, `runner.rs`, `output_parser.rs`, `ast.rs` (read-only)
**Test with:** Claude Code on a real Next.js project
**Success metric:** Agent calls our tools instead of raw `npm run lint` / `npx tsc`. Verify→doctor chain fires correctly.

### Wave 2: Blast Radius + Patterns (Make the agent smarter before it edits)

**Tools:** `fe_impact`, `fe_scaffold`
**New shared infra:** Dependency graph builder (extends `ast.rs` with import resolution)
**Success metric:** Agent checks impact before modifying shared components. Agent uses scaffold patterns to produce correct-first-try code.

### Wave 3: Mechanical Edits (Let the tool do the typing)

**Tools:** `fe_surgeon`, `fe_batch`
**New shared infra:** AST mutation engine (extends `ast.rs` with write capabilities), file transaction/rollback
**Success metric:** Rename refactors complete in 1 tool call instead of 12 file edits. Batch rollback works on verify failure.

### Wave 4: Optimization (Token efficiency)

**Tools:** `fe_diff`, `fe_budget`
**New shared infra:** Token counter, semantic diff engine
**Success metric:** Measurable token reduction vs. baseline on the 3 simulation scenarios from the integration doc.

---

## Key Architectural Decisions to Make Before Coding

| Decision | Options | Leaning | Needs |
|----------|---------|---------|-------|
| **Language** | Rust vs TypeScript | Rust (performance, tree-sitter native bindings, single binary distribution) | Confirm team comfort with Rust async + serde |
| **MCP transport** | stdio vs SSE | stdio (simpler, all agents support it, no port management) | — |
| **MCP SDK** | `mcp-rust-sdk` vs hand-rolled JSON-RPC | Evaluate `mcp-rust-sdk` maturity; hand-roll if too immature | Spike: build hello-world MCP server both ways |
| **Tree-sitter bindings** | `tree-sitter` crate vs `tree-sitter-cli` subprocess | `tree-sitter` crate (in-process, faster) | — |
| **Output parsing** | Regex vs JSON reporter flags | JSON reporter where available (`eslint -f json`, `tsc` has no JSON — regex needed) | Verify which tools support `--format json` |
| **Test fixtures** | Real mini-projects vs synthetic | Real mini-projects (Next.js, Vite+React) checked into repo | Create them during POC setup |

---

## What "Done" Looks Like for the POC

1. `fe-tools serve` starts an MCP server over stdio.
2. Claude Code (or any MCP client) connects, sees 3 tools in `tools/list`.
3. Agent runs a frontend task on a real project:
   - Calls `fe_skeleton` to understand the codebase → gets structured component tree.
   - Writes code.
   - Calls `fe_verify` → gets structured lint/type/test results (not terminal noise).
   - On error, calls `fe_doctor` → gets structured diagnosis with fix instructions.
   - Fixes and re-verifies.
4. Total tool calls reduced vs. baseline. Agent never shells out to lint/tsc/test directly.
