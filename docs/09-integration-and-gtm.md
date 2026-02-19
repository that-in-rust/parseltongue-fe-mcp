# Frontend Agent Tools: MCP Integration, Token Simulations & GTM

> Building MCP-based tools for frontend coding agents (Claude Code, Cursor, OpenCode, Codex, Cline). Scoped specifically to React/Next.js/Vue/Svelte/CSS workflows. With hard token numbers.

---

## Part 1: What We're Building — 9 MCP Tools for Frontend Agents

Each tool is an MCP server that any coding agent can call. The agent doesn't know or care how the tool works internally. It calls the tool, gets structured JSON back.

| MCP Tool | What It Does (Frontend Context) |
|----------|-------------------------------|
| `fe.verify` | Lint + typecheck + test changed React/Vue/Svelte files. Returns structured JSON, not terminal noise. |
| `fe.skeleton` | Token-optimized map of a frontend codebase: component tree, hook signatures, route structure, API layer. |
| `fe.impact` | "If you change this component/hook, what pages, tests, and Storybook stories break?" |
| `fe.scaffold` | Given a component's prop types, generate constraints: required props, event handlers, render cases, available hooks. |
| `fe.doctor` | Intercept tsc/ESLint/build errors → structured diagnosis with specific fix instructions. |
| `fe.diff` | Semantic change description: "Component became async", "Prop renamed", "Hook dependency changed." |
| `fe.surgeon` | Structured AST operations: add prop, wrap in provider, extract component, add hook call. |
| `fe.batch` | Atomic multi-file edits with rollback. For changes that span component + test + story + route. |
| `fe.budget` | Allocates a token budget across the codebase: full source for target, signatures for neighbors, skeleton for everything else. |

---

## Part 2: Token Consumption Simulations

### Simulation Setup

All simulations use a **real-world mid-size Next.js 14 app**:
- 180 components, 45 hooks, 30 API routes, 22 pages
- ~95,000 tokens if you read every file raw
- TypeScript, Tailwind CSS, React Server Components

Agent model: Claude Sonnet (for cost calculations: $3/M input, $15/M output)

---

### Scenario 1: "Add a loading skeleton to the UserProfile component"

**WITHOUT our tools (standard agent flow):**

```
Step                                          Input Tokens   Output Tokens
─────────────────────────────────────────────────────────────────────────
1. Agent reads src/components/UserProfile.tsx      620            —
2. Agent reads src/types/user.ts                   340            —
3. Agent reads src/hooks/useUser.ts                280            —
4. Agent reads src/components/Skeleton.tsx          180            —
   (guesses this exists, gets lucky)
5. Agent generates modified UserProfile.tsx          —           480
6. Agent writes file                                50            30
7. Agent runs: npm run lint                         50            30
8. Agent reads lint output (ANSI-colored mess)     380            —
   "ESLint: 'Skeleton' is defined but never used
    in src/components/UserProfile.tsx line 3..."
9. Agent reasons about lint error                    —           150
10. Agent reads file again to understand context    620            —
11. Agent generates fix                              —           280
12. Agent writes fix                                50            30
13. Agent runs: npx tsc --noEmit                    50            30
14. Agent reads tsc output                         240            —
15. Agent runs: npm test -- UserProfile             50            30
16. Agent reads test output                        450            —
    (Jest verbose output with PASS/FAIL banners)
17. Agent confirms success                           —            40
─────────────────────────────────────────────────────────────────────────
TOTALS                                           3,360         1,100
GRAND TOTAL: 4,460 tokens
COST: ~$0.027
ROUND TRIPS: 17 tool calls
```

**WITH our MCP tools:**

```
Step                                          Input Tokens   Output Tokens
─────────────────────────────────────────────────────────────────────────
1. Agent calls fe.budget({                          —             80
     task: "Add loading skeleton to UserProfile",
     budget: 2000
   })
   → Returns: optimized context package              850           —
     (UserProfile full source, useUser signature,
      Skeleton component signature, prop types,
      existing loading patterns in codebase)

2. Agent calls fe.scaffold({                         —             40
     component: "UserProfile",
     file: "src/components/UserProfile.tsx"
   })
   → Returns: prop constraints, available hooks       120           —
     loading states in codebase, test patterns

3. Agent generates code                               —           420
   (better first attempt because it has
    constraints and patterns upfront)

4. Agent calls fe.batch({                             —            80
     edits: [{file: "UserProfile.tsx", content: "..."}],
     verify: true
   })
   → Tool applies edit + runs verify internally
   → Returns:                                        110           —
     {status: "pass", lint: "pass",
      types: "pass", tests: {ran: 2, passed: 2}}
─────────────────────────────────────────────────────────────────────────
TOTALS                                            1,080           620
GRAND TOTAL: 1,700 tokens
COST: ~$0.012
ROUND TRIPS: 4 tool calls
```

**Result: 62% fewer tokens. 76% fewer round trips. 55% cheaper.**

---

### Scenario 2: "Rename the `useAuth` hook to `useSession` and update all consumers"

This is the scenario where the tools shine the most — a cross-cutting refactor.

**WITHOUT our tools:**

```
Step                                          Input Tokens   Output Tokens
─────────────────────────────────────────────────────────────────────────
1. Agent greps for "useAuth"                        50            30
2. Agent reads grep results                        280            —
   (12 files reference useAuth)
3. Agent reads each of 12 files to understand       —             —
   how useAuth is used:
   - src/hooks/useAuth.ts                          320            —
   - src/components/LoginForm.tsx                  580            —
   - src/components/Navbar.tsx                     440            —
   - src/components/ProtectedRoute.tsx             360            —
   - src/pages/dashboard.tsx                       680            —
   - src/pages/settings.tsx                        520            —
   - src/pages/profile.tsx                         460            —
   - src/middleware.ts                             340            —
   - src/lib/api-client.ts                         280            —
   - src/__tests__/useAuth.test.ts                 620            —
   - src/__tests__/LoginForm.test.tsx              540            —
   - src/__tests__/Navbar.test.tsx                 380            —
4. Agent generates 12 modified files                 —          4,800
5. Agent writes 12 files (12 tool calls)           600           360
6. Agent runs lint                                  50            30
7. Agent reads lint output                         480            —
8. Agent runs tsc                                   50            30
9. Agent reads tsc output (3 type errors)          520            —
   (missed a re-export in src/index.ts)
10. Agent reads src/index.ts                       200            —
11. Agent fixes re-export                            —           120
12. Agent writes fix                                50            30
13. Agent re-runs tsc                               50            30
14. Agent reads output                             180            —
15. Agent runs tests                                50            30
16. Agent reads test output (2 failures)           680            —
17. Agent reads failing test files                 800            —
18. Agent generates fixes                            —           340
19. Agent writes fixes                             100            60
20. Agent re-runs tests                             50            30
21. Agent reads output: all pass                   180            —
─────────────────────────────────────────────────────────────────────────
TOTALS                                           8,140         5,920
GRAND TOTAL: 14,060 tokens
COST: ~$0.113
ROUND TRIPS: 21+ tool calls
WALL CLOCK: ~3-5 minutes of agent time
```

**WITH our MCP tools:**

```
Step                                          Input Tokens   Output Tokens
─────────────────────────────────────────────────────────────────────────
1. Agent calls fe.impact({                           —            60
     symbol: "useAuth",
     file: "src/hooks/useAuth.ts",
     change_type: "rename",
     new_name: "useSession"
   })
   → Returns: complete blast radius                  280           —
     {
       direct_consumers: [12 files with line numbers],
       re_exports: ["src/index.ts:14"],
       test_files: ["useAuth.test.ts", "LoginForm.test.tsx",
                     "Navbar.test.tsx"],
       type_references: ["AuthContext", "AuthState"],
       safe_rename: true  // no dynamic references
     }

2. Agent calls fe.surgeon({                          —           100
     operations: [
       {op: "rename_symbol", from: "useAuth", to: "useSession",
        scope: "project"},
       {op: "rename_file",
        from: "src/hooks/useAuth.ts",
        to: "src/hooks/useSession.ts"},
       {op: "rename_file",
        from: "src/__tests__/useAuth.test.ts",
        to: "src/__tests__/useSession.test.ts"},
       {op: "update_imports", old_path: "useAuth", new_path: "useSession"},
       {op: "update_re_exports", file: "src/index.ts",
        old: "useAuth", new: "useSession"}
     ]
   })
   → Tool applies ALL renames atomically             60            —
     across 14 files (12 consumers + 2 test renames)

3. Agent calls fe.verify({                           —            40
     scope: "affected"
   })
   → Runs lint, tsc, tests on ONLY affected files
   → Returns:                                        120           —
     {status: "pass", lint: "pass", types: "pass",
      tests: {ran: 8, passed: 8},
      files_modified: 14}

4. Agent calls fe.diff({scope: "uncommitted"})       —            30
   → Returns semantic summary:                       90            —
     {changes: [
       {type: "symbol_rename", from: "useAuth",
        to: "useSession", files_affected: 14},
       {type: "file_rename", from: "useAuth.ts",
        to: "useSession.ts"}
     ]}
─────────────────────────────────────────────────────────────────────────
TOTALS                                              550           230
GRAND TOTAL: 780 tokens
COST: ~$0.005
ROUND TRIPS: 4 tool calls
WALL CLOCK: ~15 seconds
```

**Result: 94% fewer tokens. 81% fewer round trips. 96% cheaper.**

The rename scenario is where `fe.surgeon` + `fe.impact` eliminate almost all overhead. The agent never reads any file. It never generates modified source code character by character. It issues semantic operations, and the tools do the mechanical work.

---

### Scenario 3: "Build a new ProductCard component with props, tests, and Storybook story"

A greenfield component creation — the most common frontend task.

**WITHOUT our tools:**

```
Step                                          Input Tokens   Output Tokens
─────────────────────────────────────────────────────────────────────────
1. Agent reads existing card components              —             —
   to match patterns:
   - src/components/UserCard.tsx                   460            —
   - src/components/OrderCard.tsx                  380            —
2. Agent reads types                               220            —
   - src/types/product.ts
3. Agent reads existing test for pattern match     540            —
   - src/__tests__/UserCard.test.tsx
4. Agent reads existing story for pattern match    380            —
   - src/stories/UserCard.stories.tsx
5. Agent generates ProductCard.tsx                   —           620
6. Agent generates ProductCard.test.tsx              —           540
7. Agent generates ProductCard.stories.tsx           —           380
8. Agent writes 3 files                            150            90
9. Agent runs lint                                  50            30
10. Agent reads lint output (1 warning)            280            —
11. Agent generates fix                              —           120
12. Agent writes fix                                50            30
13. Agent runs tsc                                  50            30
14. Agent reads tsc output                         180            —
15. Agent runs tests                                50            30
16. Agent reads test output                        320            —
─────────────────────────────────────────────────────────────────────────
TOTALS                                           3,110         1,870
GRAND TOTAL: 4,980 tokens
COST: ~$0.037
ROUND TRIPS: 16 tool calls
```

**WITH our MCP tools:**

```
Step                                          Input Tokens   Output Tokens
─────────────────────────────────────────────────────────────────────────
1. Agent calls fe.scaffold({                         —            60
     task: "create_component",
     name: "ProductCard",
     type_file: "src/types/product.ts",
     match_pattern_from: "src/components/UserCard.tsx"
   })
   → Returns:                                       320            —
     {
       prop_types: {product: {name, price, image, ...}},
       patterns: {
         component_template: "functional with destructured props",
         test_template: "render + snapshot + interaction",
         story_template: "default + variants (compact, large)",
         css_pattern: "tailwind utility classes",
         import_style: "named exports"
       },
       conventions: {
         file_location: "src/components/ProductCard.tsx",
         test_location: "src/__tests__/ProductCard.test.tsx",
         story_location: "src/stories/ProductCard.stories.tsx"
       }
     }

2. Agent generates 3 files                           —          1,200
   (higher quality first attempt because scaffold
    provided exact patterns, types, conventions)

3. Agent calls fe.batch({                             —            80
     creates: [
       {file: "src/components/ProductCard.tsx", content: "..."},
       {file: "src/__tests__/ProductCard.test.tsx", content: "..."},
       {file: "src/stories/ProductCard.stories.tsx", content: "..."}
     ],
     verify: true
   })
   → Creates all 3 files + runs verification
   → Returns:                                        130           —
     {status: "pass", lint: "pass", types: "pass",
      tests: {ran: 3, passed: 3},
      files_created: 3}
─────────────────────────────────────────────────────────────────────────
TOTALS                                              450          1,340
GRAND TOTAL: 1,790 tokens
COST: ~$0.022
ROUND TRIPS: 3 tool calls
```

**Result: 64% fewer tokens. 81% fewer round trips. 41% cheaper.**

---

### Simulation Summary

| Scenario | Without Tools | With Tools | Token Reduction | Cost Reduction | Round Trip Reduction |
|----------|--------------|------------|-----------------|----------------|---------------------|
| Add loading skeleton | 4,460 | 1,700 | **62%** | 55% | 76% (17→4) |
| Rename hook across codebase | 14,060 | 780 | **94%** | 96% | 81% (21→4) |
| Create new component + test + story | 4,980 | 1,790 | **64%** | 41% | 81% (16→3) |
| **Weighted average (typical session)** | | | **~70%** | **~65%** | **~80%** |

**At scale:** A frontend developer using Claude Code with Sonnet spends ~$50-150/month on API tokens. A **70% reduction** means **$35-105/month saved per developer**. A 20-person frontend team saves **$700-2,100/month** — which more than justifies a $20/dev/month tool subscription.

---

## Part 3: System Prompt Strategy — Making the LLM Pick Up Our Tools

This is the most critical section. MCP tools are useless if the agent doesn't call them. The agent decides what to call based on **three inputs**:

1. **Tool descriptions** (from MCP `tools/list` response)
2. **System prompt instructions** (from CLAUDE.md, .cursorrules, opencode agents)
3. **In-context examples** (from conversation history)

We need to nail all three.

### 3.1 MCP Tool Descriptions (What the Agent Sees)

These are the actual MCP tool definitions. The `description` field is what the LLM reads to decide whether to call the tool. Every word matters.

```jsonc
// ═══════════════════════════════════════════════════
// fe.verify — THE MOST IMPORTANT TOOL (agents call this the most)
// ═══════════════════════════════════════════════════
{
  "name": "fe_verify",
  "description": "Verify frontend code changes: runs lint (ESLint/Biome), typecheck (tsc), and affected tests in one call. Returns structured JSON — NOT terminal output. Use this INSTEAD of running 'npm run lint', 'npx tsc', or 'npm test' separately. Returns fix suggestions you can apply directly. Always call this after writing or modifying any .tsx/.ts/.css file.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "files": {
        "type": "array",
        "items": {"type": "string"},
        "description": "Files to verify. Omit to verify all files changed since last commit."
      },
      "checks": {
        "type": "array",
        "items": {"enum": ["lint", "types", "tests", "all"]},
        "default": ["all"],
        "description": "Which checks to run."
      },
      "fix": {
        "type": "boolean",
        "default": false,
        "description": "Auto-fix lint issues where possible."
      }
    }
  }
}

// ═══════════════════════════════════════════════════
// fe.skeleton — CODEBASE UNDERSTANDING
// ═══════════════════════════════════════════════════
{
  "name": "fe_skeleton",
  "description": "Get a token-optimized map of the frontend codebase. Returns component tree, hook signatures, route structure, and API layer WITHOUT reading every file. Use this FIRST when you need to understand the project structure, find where a component lives, or discover what hooks/utilities are available. Massively cheaper than reading files individually — a 180-component codebase fits in ~500 tokens instead of ~50,000.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "scope": {
        "enum": ["full", "components", "hooks", "routes", "api", "types", "styles"],
        "default": "full",
        "description": "Which part of the codebase to map."
      },
      "depth": {
        "enum": ["tree", "signatures", "full"],
        "default": "signatures",
        "description": "tree=file names only, signatures=exports+types, full=implementation."
      },
      "filter": {
        "type": "string",
        "description": "Filter by pattern. E.g. 'User*' shows only User-related files."
      }
    }
  }
}

// ═══════════════════════════════════════════════════
// fe.impact — BLAST RADIUS
// ═══════════════════════════════════════════════════
{
  "name": "fe_impact",
  "description": "Before modifying a component, hook, or utility — call this to see what will break. Returns every file that imports the target, every test that covers it, and every page that renders it. Essential before renaming, changing props, or modifying return types. Prevents the common mistake of changing a shared component without updating consumers.",
  "inputSchema": {
    "type": "object",
    "required": ["symbol", "file"],
    "properties": {
      "symbol": {
        "type": "string",
        "description": "The exported symbol to analyze. E.g. 'UserProfile', 'useAuth', 'fetchUser'."
      },
      "file": {
        "type": "string",
        "description": "The file containing the symbol."
      },
      "change_type": {
        "enum": ["modify", "rename", "delete", "change_props", "change_return_type"],
        "description": "What kind of change you plan to make. Affects what dependencies are flagged."
      }
    }
  }
}

// ═══════════════════════════════════════════════════
// fe.scaffold — COMPONENT/HOOK CONSTRAINTS
// ═══════════════════════════════════════════════════
{
  "name": "fe_scaffold",
  "description": "Get constraints and patterns for writing a new component, hook, or modifying an existing one. Returns: prop types with descriptions, required vs optional props, existing patterns in the codebase (naming, file structure, test patterns, CSS approach), available hooks and utilities, and render cases to handle. Call this BEFORE writing component code to get it right on the first try.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "task": {
        "enum": ["create_component", "create_hook", "modify_component", "add_props", "create_page"],
        "description": "What you're building."
      },
      "name": {
        "type": "string",
        "description": "Name of the component/hook."
      },
      "type_file": {
        "type": "string",
        "description": "Path to file containing relevant types."
      },
      "match_pattern_from": {
        "type": "string",
        "description": "Path to a similar existing component to match patterns from."
      }
    }
  }
}

// ═══════════════════════════════════════════════════
// fe.doctor — ERROR DIAGNOSIS
// ═══════════════════════════════════════════════════
{
  "name": "fe_doctor",
  "description": "Diagnose a build/lint/type error and get structured fix instructions. Pass the error message or code and file path. Returns: root cause, exact fix location, and specific operations to resolve it. Especially useful for cryptic TypeScript errors, React hydration mismatches, and Next.js build failures. Use this instead of trying to reason about error messages yourself.",
  "inputSchema": {
    "type": "object",
    "required": ["error"],
    "properties": {
      "error": {
        "type": "string",
        "description": "The error message, tsc error code, or ESLint rule name."
      },
      "file": {
        "type": "string",
        "description": "File where the error occurred."
      },
      "line": {
        "type": "number",
        "description": "Line number of the error."
      }
    }
  }
}

// ═══════════════════════════════════════════════════
// fe.surgeon — AST OPERATIONS
// ═══════════════════════════════════════════════════
{
  "name": "fe_surgeon",
  "description": "Apply structured code operations instead of rewriting entire files. Operations: rename_symbol, add_prop, remove_prop, add_import, remove_import, wrap_in_component (e.g. wrap in Suspense/ErrorBoundary), extract_component, add_hook_call, rename_file, update_imports. Faster and safer than generating modified source text — no syntax errors possible.",
  "inputSchema": {
    "type": "object",
    "required": ["operations"],
    "properties": {
      "operations": {
        "type": "array",
        "items": {
          "type": "object",
          "properties": {
            "op": {"type": "string"},
            "file": {"type": "string"},
            "target": {"type": "string"},
            "args": {"type": "object"}
          }
        },
        "description": "Array of AST operations to apply."
      },
      "dry_run": {
        "type": "boolean",
        "default": false,
        "description": "Preview changes without applying."
      }
    }
  }
}

// ═══════════════════════════════════════════════════
// fe.diff — SEMANTIC CHANGES
// ═══════════════════════════════════════════════════
{
  "name": "fe_diff",
  "description": "Get a semantic summary of code changes (not line-level diff). Returns what changed in terms of components, props, hooks, types — not added/removed lines. Use after completing a task to verify the changes are correct, or when reviewing what changed.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "scope": {
        "enum": ["uncommitted", "last_commit", "branch"],
        "default": "uncommitted"
      },
      "format": {
        "enum": ["summary", "detailed"],
        "default": "summary"
      }
    }
  }
}

// ═══════════════════════════════════════════════════
// fe.batch — ATOMIC MULTI-FILE EDITS
// ═══════════════════════════════════════════════════
{
  "name": "fe_batch",
  "description": "Apply changes to multiple files atomically. If any file fails verification (lint/types/tests), ALL changes are rolled back. Use for coordinated changes: component + test + story, or renaming across multiple files. Includes built-in verification — no need to call fe_verify separately.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "edits": {
        "type": "array",
        "items": {
          "type": "object",
          "properties": {
            "file": {"type": "string"},
            "content": {"type": "string"},
            "operations": {"type": "array", "description": "AST ops instead of full content"}
          }
        }
      },
      "creates": {
        "type": "array",
        "items": {
          "type": "object",
          "properties": {
            "file": {"type": "string"},
            "content": {"type": "string"}
          }
        }
      },
      "verify": {"type": "boolean", "default": true},
      "rollback_on_failure": {"type": "boolean", "default": true}
    }
  }
}

// ═══════════════════════════════════════════════════
// fe.budget — CONTEXT ALLOCATION
// ═══════════════════════════════════════════════════
{
  "name": "fe_budget",
  "description": "Get an optimized context package for a task. Instead of reading files one by one (wasting tokens on irrelevant code), describe your task and get back exactly the code context you need — target files in full, related files as signatures, and project structure as a skeleton. Dramatically reduces token usage for complex tasks.",
  "inputSchema": {
    "type": "object",
    "required": ["task"],
    "properties": {
      "task": {
        "type": "string",
        "description": "Natural language description of what you need to do."
      },
      "budget": {
        "type": "number",
        "default": 4000,
        "description": "Maximum tokens to allocate for context."
      },
      "focus_files": {
        "type": "array",
        "items": {"type": "string"},
        "description": "Files you know you'll need."
      }
    }
  }
}
```

### Key Description Patterns That Make LLMs Call Tools

After analyzing how Claude, GPT, and Gemini decide when to use tools, these patterns in descriptions increase invocation rates:

| Pattern | Example | Why It Works |
|---------|---------|-------------|
| **"Use INSTEAD of X"** | "Use this INSTEAD of running npm run lint" | Directly competes with the agent's default behavior. The agent recognizes it should prefer this over bash. |
| **"Call this BEFORE/AFTER Y"** | "Call this BEFORE modifying any shared component" | Creates a trigger condition. The agent learns to check before acting. |
| **"Returns structured JSON"** | "Returns structured JSON — NOT terminal output" | Agents prefer structured data. Telling them it's structured makes them prefer the tool over bash. |
| **"FIRST/ALWAYS"** | "Use this FIRST when you need to understand the project" | Priority signal. The agent learns ordering. |
| **Concrete examples** | "E.g. 'UserProfile', 'useAuth', 'fetchUser'" | Reduces ambiguity about what inputs look like. |
| **Problem it prevents** | "Prevents the common mistake of changing a shared component without updating consumers" | The agent recognizes the failure mode and wants to avoid it. |

---

### 3.2 System Prompts — The Full Prompt Per Agent

#### Claude Code (CLAUDE.md)

```markdown
# Frontend Agent Tools (fe.*)

You have access to frontend-specific MCP tools. These are MANDATORY for frontend work.

## Required Workflow

### Before ANY task:
1. Call `fe_skeleton` to understand the codebase structure (NOT `find` or `ls`)
2. Call `fe_budget` with your task description to get optimized context
3. Read ONLY what `fe_budget` tells you to read

### Before modifying a shared component or hook:
1. Call `fe_impact` to see the blast radius
2. Plan changes for ALL affected files before starting

### When creating new components:
1. Call `fe_scaffold` to get patterns, conventions, and type constraints
2. Create component + test + story together using `fe_batch`

### When making cross-file changes (renames, prop changes):
1. Use `fe_surgeon` for mechanical operations (rename, add import, etc.)
2. Use `fe_batch` to apply all changes atomically

### After ANY code change:
1. Call `fe_verify` — do NOT run `npm run lint`, `npx tsc`, or `npm test` directly
2. If `fe_verify` returns errors, call `fe_doctor` with the error before trying to fix manually
3. If `fe_doctor` suggests operations, apply them via `fe_surgeon`

### When done with a task:
1. Call `fe_diff` to review what changed semantically

## Why These Rules

- `fe_skeleton` + `fe_budget` replace reading files — saves 90% of input tokens
- `fe_verify` replaces separate lint/tsc/test commands — one call instead of six
- `fe_impact` prevents breaking changes you didn't know about
- `fe_surgeon` applies changes mechanically — no syntax errors possible
- `fe_batch` ensures multi-file changes either all succeed or all rollback
```

#### Cursor (.cursorrules)

```markdown
# Frontend Agent Tools

When MCP tool `fe_*` is available, follow these rules for all frontend work:

## Component Work
- Before creating: `fe_scaffold` → get patterns and constraints
- Before modifying shared components: `fe_impact` → see what breaks
- Create component + test + story: `fe_batch` → atomic creation

## Verification
- After ALL code changes: `fe_verify` (not manual lint/tsc/test commands)
- On errors: `fe_doctor` → structured diagnosis with fix suggestions

## Understanding Code
- Start tasks with: `fe_budget` → optimized context package
- Explore codebase with: `fe_skeleton` → component tree and signatures
- Never read more than 3 files manually — use fe_budget instead

## Refactoring
- Renames/prop changes: `fe_surgeon` → AST operations, not text rewriting
- Multi-file: `fe_batch` → atomic with rollback
- Pre-check: `fe_impact` → blast radius analysis
```

#### OpenCode (Custom Agent Profile)

```jsonc
// .opencode/agents/frontend.json
{
  "name": "frontend",
  "description": "Frontend development agent with optimized tooling",
  "system_prompt": "You are a frontend development specialist. You have access to the fe.* MCP tools optimized for React/Next.js/Vue/Svelte work.\n\nCRITICAL RULES:\n1. NEVER run npm/npx commands for lint, typecheck, or test. Use fe_verify instead.\n2. NEVER read more than 3 files manually. Use fe_budget to get optimized context.\n3. ALWAYS call fe_impact before modifying any exported component, hook, or utility.\n4. ALWAYS use fe_batch for multi-file changes.\n5. For renames and mechanical refactors, use fe_surgeon instead of rewriting files.\n6. On any error, call fe_doctor before attempting manual fixes.\n\nYour goal: produce correct frontend code in the minimum number of tool calls and tokens.",
  "tools": [
    "fe_verify", "fe_skeleton", "fe_impact", "fe_scaffold",
    "fe_doctor", "fe_diff", "fe_surgeon", "fe_batch", "fe_budget",
    "bash", "file_read", "file_write"
  ]
}
```

#### Cline (VS Code Settings)

```jsonc
// .vscode/cline_instructions.md
// (Cline reads this as custom instructions)

## Frontend Agent Tools

You have `fe_*` tools available via MCP. Use them:

- `fe_verify` INSTEAD of terminal lint/tsc/test commands
- `fe_skeleton` INSTEAD of exploring files one by one
- `fe_impact` BEFORE touching any shared code
- `fe_scaffold` BEFORE creating new components
- `fe_surgeon` for renames and mechanical refactors
- `fe_batch` for multi-file changes (atomic with rollback)
- `fe_budget` to get optimized context for complex tasks
- `fe_doctor` to diagnose errors structurally

Priority: fe_budget → fe_scaffold/fe_impact → write code → fe_batch → fe_verify → fe_diff
```

---

### 3.3 Prompt Engineering: Teaching Tool Chains

The system prompt alone isn't enough. We also need the tool descriptions to **chain naturally**. When `fe_verify` returns an error, the response should make it obvious to call `fe_doctor`:

```jsonc
// fe_verify response when there's an error:
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
        "hint": "Call fe_doctor with this error for a detailed fix suggestion"
        //       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
        //       This hint teaches the agent to chain tools
      }
    ]
  }
}

// fe_doctor response includes operations for fe_surgeon:
{
  "diagnosis": "useEffect missing dependency",
  "root_cause": "userId is read inside effect but not in deps array",
  "fix": {
    "description": "Add userId to dependency array",
    "operation": {
      "op": "add_hook_dependency",
      "file": "src/components/UserProfile.tsx",
      "hook": "useEffect",
      "line": 12,
      "dependency": "userId"
    },
    "hint": "Apply this fix using fe_surgeon"
    //       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    //       Chains to the next tool
  }
}
```

This **hint chaining** pattern trains the agent to follow the tool pipeline without explicit system prompt instructions for every possible scenario.

---

## Part 4: Frontend-Specific GTM

### Target Persona

**Primary:** Frontend developers using AI coding agents for React/Next.js/Vue work.

**Quantified market:**
- ~15M frontend developers worldwide (2025 StackOverflow estimate)
- ~40% use AI coding tools (2026 GitHub survey)
- = ~6M potential users
- Willingness to pay for AI tooling: high (Cursor has 1M+ users at $20/mo)

### Frontend-Specific Positioning

> **For frontend developers whose AI agents build React, Next.js, Vue, and Svelte apps** — our MCP tools give your agent structured understanding of component trees, prop types, hook dependencies, and route structures. Your agent stops guessing and starts knowing. **70% fewer tokens. 80% fewer round trips. Zero broken builds.**

### Why Frontend First (Not Full-Stack)

| Reason | Detail |
|--------|--------|
| **Component architecture is graph-shaped** | Components import components. Props flow down. Events flow up. This graph structure is exactly what `fe.impact` and `fe.skeleton` exploit. Backend code is flatter. |
| **Frontend has the most boilerplate** | Component + test + story + types for every feature. `fe.scaffold` and `fe.batch` eliminate this boilerplate tax. |
| **TypeScript adoption is highest in frontend** | >80% of React projects use TS. This means `fe.verify` (type checking) and `fe.scaffold` (type constraints) provide maximum value. |
| **Frontend refactors are the most painful** | Renaming a prop ripples through 20 files. `fe.surgeon` + `fe.impact` solve this categorically. Backend renames are typically 2-3 files. |
| **AI agents struggle most with frontend** | JSX, CSS, component state, hooks rules, hydration — these are the areas where agents make the most errors. Our tools directly address these. |
| **Largest AI coding tool user base** | Cursor, the most popular AI coding tool, is used primarily for frontend work. |

### Launch Strategy — Frontend Communities

```
WEEK 1-2: SEED
─────────────
Target: Early adopter frontend devs who use Claude Code / Cursor daily

Channels:
  - Twitter/X: Demo video "Claude Code + fe.verify = no more broken builds"
  - Reddit: r/reactjs, r/nextjs, r/webdev
  - Hacker News: "Show HN: MCP tools that cut AI agent token usage 70% for frontend"
  - Discord: Reactiflux, Next.js, Vue Land

Content:
  - 2-minute demo: side-by-side of agent with/without tools on a React task
  - Blog: "The token tax your AI agent pays on every frontend task"

WEEK 3-4: EXPAND
─────────────────
Target: Frontend team leads exploring AI coding at work

Channels:
  - Dev.to: Integration guides for each agent
  - YouTube: "5 min setup with Cursor" + "5 min setup with Claude Code"
  - cursor.directory/mcp: Submit for listing

Content:
  - Benchmark post: token usage across 5 real frontend scenarios
  - Template repos: Next.js starter + Vite+React starter + Nuxt starter
    pre-configured with our MCP tools

WEEK 5-8: ESTABLISH
────────────────────
Target: Frontend conference and newsletter audiences

Channels:
  - This Week in React newsletter (sponsor/feature)
  - Bytes.dev newsletter
  - Frontend-specific podcasts (Syntax.fm, JS Party)
  - React Summit / Next.js Conf CFPs

Content:
  - "How we built an MCP server that understands React component trees"
  - Open source the benchmark suite so others can verify our claims
  - Case study: real team, real numbers
```

### Pricing (Frontend-Specific)

```
FREE (Individual)
  ✓ All 9 tools, unlimited usage
  ✓ React, Next.js, Vue, Svelte, Astro support
  ✓ Works with all agents (Claude Code, Cursor, OpenCode, etc.)
  ✓ Community support (Discord + GitHub)

TEAM — $19/dev/month
  ✓ Everything in Free
  ✓ Shared team config (enforce consistent patterns across all devs' agents)
  ✓ Token usage analytics ("your team saved X tokens this week")
  ✓ Custom component conventions (enforce your design system patterns)
  ✓ Priority support (Slack)
  ✓ CI/CD integration (verify agent-generated PRs)

ENTERPRISE — Custom
  ✓ Everything in Team
  ✓ SSO / SAML
  ✓ Audit trail (which agent modified which component)
  ✓ Custom design system integration (fe.scaffold knows your DS)
  ✓ Private MCP registry
  ✓ SLA + dedicated support
```

---

## Part 5: MCP Config — Copy-Paste Setup for Each Agent

### Claude Code

```jsonc
// .claude/settings.json
{
  "mcpServers": {
    "frontend-tools": {
      "command": "fe-tools",
      "args": ["serve", "--framework", "react", "--project-root", "."]
    }
  }
}
```

Plus: drop the CLAUDE.md system prompt from Part 3.2 into `.claude/CLAUDE.md`.

### Cursor

```jsonc
// .cursor/mcp.json
{
  "mcpServers": {
    "frontend-tools": {
      "command": "fe-tools",
      "args": ["serve", "--framework", "react", "--project-root", "."]
    }
  }
}
```

Plus: add `.cursorrules` from Part 3.2.

### OpenCode

```jsonc
// opencode.json
{
  "mcp": {
    "frontend-tools": {
      "type": "local",
      "command": "fe-tools",
      "args": ["serve", "--framework", "react", "--project-root", "."]
    }
  }
}
```

Plus: add the custom agent profile from Part 3.2 into `.opencode/agents/frontend.json`.

### Codex CLI

```jsonc
// .codex/config.json
{
  "mcpServers": {
    "frontend-tools": {
      "type": "stdio",
      "command": "fe-tools",
      "args": ["serve", "--framework", "react", "--project-root", "."]
    }
  }
}
```

### Cline

```jsonc
// VS Code settings.json
{
  "cline.mcpServers": {
    "frontend-tools": {
      "command": "fe-tools",
      "args": ["serve", "--framework", "react", "--project-root", "${workspaceFolder}"]
    }
  }
}
```

### One-Command Init (for all agents)

```bash
# Auto-detects agent + framework, generates all config files
npx fe-tools init

# Or specify:
npx fe-tools init --agent claude-code --framework nextjs
npx fe-tools init --agent cursor --framework vue
npx fe-tools init --agent opencode --framework svelte
```

This creates:
- MCP config for the detected agent
- System prompt / rules file with tool usage instructions
- `fe-tools.config.json` with project-specific settings (framework, test runner, lint config)

---

## Part 6: Success Metrics

### Phase 1 (Month 0-3) — Does it work?

| Metric | Target |
|--------|--------|
| GitHub stars | 3,000 |
| Weekly installs | 500 |
| Token reduction (measured) | >50% on benchmark scenarios |
| Agent tool invocations/day (opt-in telemetry) | 1,000+ |
| Supported frameworks | React, Next.js, Vue, Svelte |

### Phase 2 (Month 3-6) — Does it grow?

| Metric | Target |
|--------|--------|
| GitHub stars | 10,000 |
| Weekly installs | 5,000 |
| Community PRs | 50+ |
| Listed in cursor.directory | Yes |
| Featured in Claude Code MCP docs | Yes |
| Blog posts by external devs | 10+ |

### Phase 3 (Month 6-12) — Does it monetize?

| Metric | Target |
|--------|--------|
| Team tier subscribers | 200 teams |
| MRR | $40,000 |
| Enterprise pipeline | 10 conversations |
| Agent compatibility | 8+ agents |
| Framework coverage | React, Next.js, Vue, Nuxt, Svelte, SvelteKit, Astro, Remix |

---

## Sources

- [Claude Code MCP Documentation](https://code.claude.com/docs/en/mcp)
- [Cursor MCP Setup Guide](https://claudefa.st/blog/tools/mcp-extensions/cursor-mcp-setup)
- [OpenCode MCP Servers Documentation](https://opencode.ai/docs/mcp-servers/)
- [OpenCode Agents Documentation](https://opencode.ai/docs/agents/)
- [Codex CLI MCP Documentation](https://developers.openai.com/codex/mcp/)
- [Cline GitHub](https://github.com/cline/cline)
- [Scott Spence: Configuring MCP Tools in Claude Code](https://scottspence.com/posts/configuring-mcp-tools-in-claude-code)
- [FastMCP: Claude Code Integration](https://gofastmcp.com/integrations/claude-code)
- [Product Marketing Alliance: Open Source to PLG](https://www.productmarketingalliance.com/developer-marketing/open-source-to-plg/)
- [Catchy Agency: What 202 Open Source Developers Taught Us About Tool Adoption](https://www.catchyagency.com/post/what-202-open-source-developers-taught-us-about-tool-adoption)
- [a16z: Open Source — From Community to Commercialization](https://a16z.com/open-source-from-community-to-commercialization/)
- [Factory.ai: The Context Window Problem](https://factory.ai/news/context-window-problem)
- [Anthropic: 2026 Agentic Coding Trends Report](https://resources.anthropic.com/hubfs/2026%20Agentic%20Coding%20Trends%20Report.pdf)
- [Tembo: 2026 Guide to Coding CLI Tools — 15 Agents Compared](https://www.tembo.io/blog/coding-cli-tools-comparison)
