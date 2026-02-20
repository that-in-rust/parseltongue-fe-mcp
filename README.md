# fe-tools

MCP server that gives AI coding agents structured understanding of frontend codebases. One connection, three tools — replacing dozens of raw terminal commands with structured JSON.

**Works with:** Claude Code, Cursor, Windsurf, OpenCode, Cline, and any MCP-compatible client.

## Tools

### `fe_verify` — Lint + Typecheck + Tests in One Call

Runs ESLint/Biome, tsc, and Jest/Vitest on your changed files. Returns structured JSON instead of terminal noise. Auto-detects which tools your project uses.

```json
// Agent calls:
{"name": "fe_verify", "arguments": {"files": ["src/App.tsx"]}}

// Returns:
{
  "status": "fail",
  "lint": {
    "status": "fail",
    "error_count": 1,
    "warning_count": 0,
    "errors": [
      {
        "file": "src/App.tsx",
        "line": 3,
        "column": 7,
        "message": "'x' is defined but never used.",
        "rule": "no-unused-vars",
        "severity": "error",
        "suggestion": "Call fe_doctor with this error for a structured fix"
      }
    ]
  },
  "types": {"status": "skipped: Skipped due to lint errors"},
  "tests": {"status": "skipped: Skipped due to lint errors"},
  "suggestion": "Call fe_doctor with the errors above for structured fix suggestions"
}
```

Omit `files` to auto-verify all files changed since last commit.

### `fe_batch` — Atomic Multi-File Edits

Creates or edits multiple files in one transaction. If verification fails, everything rolls back — no half-applied changes.

```json
// Agent calls:
{
  "name": "fe_batch",
  "arguments": {
    "creates": [
      {"file": "src/components/Card.tsx", "content": "..."},
      {"file": "src/__tests__/Card.test.tsx", "content": "..."}
    ],
    "verify": true,
    "rollback_on_failure": true
  }
}
```

### `fe_surgeon` — AST-Level Code Operations

Structured code transformations instead of rewriting entire files. No syntax errors possible.

```json
// Agent calls:
{
  "name": "fe_surgeon",
  "arguments": {
    "operations": [
      {"op": "rename_symbol", "file": "src/hooks/useAuth.ts", "from": "useAuth", "to": "useSession"},
      {"op": "add_import", "file": "src/App.tsx", "source": "react", "specifiers": ["useEffect"], "type_only": false},
      {"op": "make_async", "file": "src/utils/api.ts", "function_name": "fetchUser"}
    ],
    "dry_run": false
  }
}
```

**Supported operations:** `rename_symbol`, `add_import`, `remove_import`, `update_import_paths`, `add_parameter`, `remove_parameter`, `make_async`, `wrap_in_block`, `extract_to_variable`.

## Installation

### Prebuilt Binaries

Download from [GitHub Releases](https://github.com/happycoder0011/fe-tools/releases):

```bash
# macOS (Apple Silicon)
curl -L https://github.com/happycoder0011/fe-tools/releases/latest/download/fe-tools-aarch64-apple-darwin.tar.gz | tar xz
sudo mv fe-tools /usr/local/bin/

# macOS (Intel)
curl -L https://github.com/happycoder0011/fe-tools/releases/latest/download/fe-tools-x86_64-apple-darwin.tar.gz | tar xz
sudo mv fe-tools /usr/local/bin/

# Linux (x86_64)
curl -L https://github.com/happycoder0011/fe-tools/releases/latest/download/fe-tools-x86_64-unknown-linux-gnu.tar.gz | tar xz
sudo mv fe-tools /usr/local/bin/
```

### Build from Source

Requires Rust 1.75+:

```bash
git clone https://github.com/happycoder0011/fe-tools.git
cd fe-tools
cargo build --release
# Binary at target/release/fe-tools
```

## Configuration

### Claude Code

Add to your project's `.claude/settings.json`:

```json
{
  "mcpServers": {
    "fe-tools": {
      "command": "fe-tools",
      "args": ["serve", "--project-root", "."]
    }
  }
}
```

### Cursor

Add to your project's `.cursor/mcp.json`:

```json
{
  "mcpServers": {
    "fe-tools": {
      "command": "fe-tools",
      "args": ["serve", "--project-root", "."]
    }
  }
}
```

### OpenCode

Add to `opencode.json`:

```json
{
  "mcp": {
    "fe-tools": {
      "type": "local",
      "command": "fe-tools",
      "args": ["serve", "--project-root", "."]
    }
  }
}
```

## What Gets Auto-Detected

fe-tools inspects your project at startup and configures itself:

| Category | Detected From | Tools Supported |
|----------|--------------|-----------------|
| **Linter** | `eslint.config.*`, `.eslintrc.*`, `biome.json` | ESLint, Biome |
| **Type checker** | `tsconfig.json` | TypeScript (tsc) |
| **Test runner** | `vitest.config.*`, `jest.config.*` | Vitest, Jest |
| **Language** | File extensions | .ts, .tsx, .js, .jsx, .vue, .svelte, .css |

## How It Works

fe-tools is a single MCP server process that communicates over stdio using JSON-RPC 2.0. When an agent connects:

1. **`tools/list`** — Returns all 3 tools with descriptions and input schemas
2. **`tools/call`** — Routes to the right tool handler, returns structured JSON

The agent never shells out to `npm run lint` or `npx tsc` directly. It calls `fe_verify` once and gets structured, parseable results.

### Cascading Verification

`fe_verify` runs checks in order: **lint -> types -> tests**. If lint fails, it skips type checking. If types fail, it skips tests. Every error includes a `suggestion` field pointing the agent to the next action.

### Atomic Batching

`fe_batch` uses a transaction model: stage changes to a temp directory, apply to working directory (with backups), verify, then commit or rollback. No partial state.

### AST Operations

`fe_surgeon` uses [tree-sitter](https://tree-sitter.github.io/) for parsing. Operations compute text edits against the original AST, merge them, and apply in a single pass. The result is re-parsed and validated — if the operation would produce invalid syntax, it fails instead of writing broken code.

## Project Structure

```
fe-tools/
├── crates/
│   ├── fe-mcp-server/     # MCP binary: stdio server, tool registry
│   ├── fe-verify/          # Lint/type/test pipeline + output parsers
│   ├── fe-batch/           # Atomic multi-file transactions
│   └── fe-common/          # Shared: project detection, git, fs utils
├── ast-surgeon/
│   └── crates/
│       ├── ast-surgeon-core/   # Tree-sitter AST operations engine
│       └── ast-surgeon-lang/   # Language detection + TS/JS/CSS support
└── docs/                   # Architecture and planning documents
```

## License

MIT
