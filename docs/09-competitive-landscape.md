# Competitive Landscape: Agentic Tooling Hypothesis

> Research date: February 2026. Competitive analysis for the 9 tools proposed in [08-hypothesis-agentic-tooling.md](./08-hypothesis-agentic-tooling.md).

---

## Executive Verdict

| Tool | Market Status | Closest Competitor | Gap Severity | Risk Level |
|------|-------------|-------------------|-------------|------------|
| **ast-surgeon** | Pre-competitive | ast-grep + MCP (search only) | **Critical** -- no structured operation vocabulary exists | Low |
| **codebase-skeleton** | Fragmented | Aider RepoMap, Augment Context Engine | **Moderate** -- partial solutions exist, none unified | High |
| **verify-pipe** | Nascent | Semaphore MCP, Biome JSON reporter | **Critical** -- no cascading pipeline exists | Medium |
| **semantic-diff** | Pre-competitive | GumTree (academic), SemanticDiff (visual) | **Critical** -- no machine-readable semantic diffs | Low |
| **impact-graph** | Emerging | Greptile (internal), CodeGPT MCP, Nx affected | **High** -- function-level graphs exist but not for agents | Medium |
| **type-scaffold** | Novel | ETH PLDI 2025 (decoding-level only) | **Critical** -- no prompt-level type constraint tool | Low |
| **batch-edit** | Unaddressed | Claude Code checkpoints, Cursor undo | **Critical** -- no atomic multi-file transactions | Low |
| **context-budget** | Crowded | Augment Context Engine MCP (70%+ improvement) | **Low** -- strong competitors exist | High |
| **error-doctor** | Nascent | Copilot Autofix (security only), APR research | **High** -- no universal structured error translator | Medium |

---

## Tool 1: ast-surgeon — AST-Level Code Manipulation API

### What Exists Today

**ast-grep + MCP Server** (Rust, OSS)
- Structural code search/lint/rewrite using tree-sitter ASTs. Has an official MCP server and Claude Code agent skill.
- **Overlap: 40% -- search side only.** The LLM still writes YAML rewrite patterns as text, not structured AST operations.

**Codemod.com** (Seed-funded)
- Platform for AST-based code transformations (jscodeshift, ts-morph). Has MCP server. AI generates transformation *scripts*, achieving 54% accuracy after 4 iterations.
- **Overlap: 30%.** Migration-oriented, not per-edit agent interactions.

**Grit.io** ($7M seed, acquired by Honeycomb April 2025)
- Declarative AST transformations via GritQL. No longer standalone post-acquisition.

**ASTify** (Prototype, Python-only)
- Puts the LLM *inside* the AST pipeline. **Closest concept (45%)** but early-stage.

**Morph (MorphLLM)** (Product)
- Specialized "apply model" at 10,500 tok/sec for merging text edits. Uses hierarchical positional encodings for AST-like structure. Optimizes *application* of text edits, not the generation format.

**Aider Issue #3206** (Open discussion)
- Community request for AST operations instead of find-and-replace. Indicates recognized need, no implementation.

### What AI Coding Tools Use Internally

| Tool | AST for Context | AST for Edits |
|------|----------------|---------------|
| Cursor | tree-sitter chunking | No (text diffs + apply model) |
| Copilot | tree-sitter context extraction | No (text generation) |
| Aider | tree-sitter repo map + PageRank | No (search-and-replace) |
| Claude Code | N/A | No (old_string/new_string) |
| Sourcegraph Cody | tree-sitter for completion triggers | No (text diffs) |

### Research

- **TreeDiff** (Aug 2025): AST-guided diffusion LLM, validates that AST awareness improves generation quality
- **AST-T5** (ICML 2024): ~40% improvement with AST-guided fine-tuning
- **Constrained Decoding** (DOMINO/ICML 2024, IterGen/ICLR 2025): Could enforce valid AST operation output

### The Gap

**No tool defines a formal operation vocabulary** (`add_import`, `make_async`, `wrap_in_guard`) that an LLM can emit. Every existing tool has the LLM generate text that is then parsed, or generate transformation scripts. The "correct by construction" promise -- where the tool generates valid AST nodes and the LLM just specifies which ones -- has zero implementations.

---

## Tool 2: codebase-skeleton — Token-Optimized Codebase Representation

### What Exists Today

| Tool | Type | Multi-Resolution | Token Budget | Importance Ranking | Traction |
|------|------|-----------------|-------------|-------------------|----------|
| **Repomix** | Flat packer | No | Token count only | No | 21.8K GitHub stars |
| **Aider RepoMap** | Smart skeleton | Partial (signatures) | Yes (1K default) | PageRank | 30K+ stars (Aider) |
| **Cursor** | RAG/Embeddings | No (binary retrieve) | N/A | Embedding similarity | $29.3B valuation |
| **Greptile** | Graph + RAG | No (query-based) | N/A | Graph + embedding | $25M Series A |
| **Augment Code** | Context Engine | Partial | Implicit | Semantic | MCP, 70%+ improvement |
| **Kit (Cased)** | Toolkit | Partial (configurable) | No | Dependency analysis | OSS, 12+ languages |
| **LLMap** | Multi-stage search | Yes (3-stage) | Implicit | LLM-judged | OSS, Java/Python only |
| **Code Maps pattern** | Concept | Partial (signatures) | No | No | Blog posts, no product |
| **codebase-map (npm)** | AST indexer | Multiple formats | No | No | Early stage |

### The Gap

No tool produces a **single, coherent, multi-resolution document** flowing from directory tree (50 tokens) to file signatures (500 tokens) to targeted full source (2000 tokens). Aider gets closest but only at the signature level.

**Token-budget-aware generation with user-controlled zoom** is missing. No tool supports "give me the skeleton at 4K tokens, then expand `auth/` to full source."

### Risk Assessment

**HIGH RISK.** Augment Code's Context Engine MCP (launched Feb 2026) with 70%+ improvement benchmarks is a near-direct substitute. Factory.ai ($50M Series B, $300M valuation) also building context engineering infrastructure.

---

## Tool 3: verify-pipe — Structured Verification Pipeline

### What Exists Today

**No AI coding tool has a built-in structured verification pipeline.** All rely on ad-hoc shell command execution with the LLM parsing human-readable output.

| Tool | Lint Output | Type Output | Test Output | Cascading | Affected-Only | Fix Suggestions |
|------|-----------|------------|------------|-----------|-------------|----------------|
| **Biome** | JSON, SARIF | Partial (inference) | N/A | No | N/A | No |
| **ESLint MCP** | MCP | N/A | N/A | No | No | ESLint fixes only |
| **TS MCP Worker** | JSON | JSON | N/A | No | No | Patches |
| **Semaphore MCP** | CI-level | CI-level | JSON summaries | No | No | No |
| **Mega-Linter** | JSON, SARIF | Per-tool | Per-tool | No | No | No |
| **trunk.io** | SARIF, JSON | Per-tool | Per-tool | No | No | No |

### Self-Healing CI (Adjacent Market)

The "self-healing CI" trend validates the direction, but operates at CI platform level:
- **Nx Cloud**: Self-healing CI with affected-only testing for monorepos
- **Dagger**: LLM-powered pipeline repair with workspace functions as tools
- **Semaphore**: MCP-exposed pipelines with `--generate-mcp-summary` for JSON test summaries
- **Graphite** (acquired by Cursor Dec 2025, $52M Series B): AI code review with CI failure self-healing

### The Gap

**No tool provides a single invocation** that cascades lint -> typecheck -> test with:
- Structured JSON output (not prose)
- Affected-only test selection
- Early termination on parse failure
- Suggested AST fix operations per error
- Single tool call for the agent

---

## Tool 4: semantic-diff — Machine-Readable Change Representation

### What Exists Today

| Tool | AST Diff | Semantic Labels | Machine-Readable | Agent-Oriented | Languages | Status |
|------|---------|----------------|-----------------|---------------|-----------|--------|
| **SemanticDiff** | Yes | No | No (visual) | No | 10 | Product |
| **GumTree** | Yes (tree edit scripts) | No (low-level ops) | Yes (edit scripts) | No | 6 | Research |
| **ChangeDistiller** | Yes | Partial (taxonomy) | No | No | Java | Research |
| **Difftastic** | Yes (expression-level) | No | No (visual) | No | 50+ | OSS |
| **Diffast** | Yes | No | Yes (RDF/XML) | No | 5 | Research |

### Academic Work

- **RefactoringMiner** (ACM TOSEM 2024): Refactoring-aware AST diff with semantic detection of renames, extract method, etc.
- **SIGMADIFF** (NDSS 2024): Deep learning-based semantic matching for pseudocode
- **Comprehensive Review** (2025): 2022-2025 advances driven by LLM integration

### The Gap

**No tool produces high-level semantic change descriptions** ("function became async," "null check added") in machine-readable JSON designed for AI agent consumption. ChangeDistiller's change taxonomy is the closest conceptual predecessor. No tool combines semantic diff with impact computation.

---

## Tool 5: impact-graph — Blast Radius Computation

### What Exists Today

| Tool | Dependency Graph | Granularity | "What's Affected?" | Agent API | Status |
|------|-----------------|-------------|-------------------|-----------|--------|
| **Nx** | Project/file | File-level | Yes (`nx affected`) | No | Production |
| **Turborepo** | Package | Package-level | Yes (coarse) | No | Production |
| **Greptile** | Function/class | Function-level | During review | No (internal) | $25M Series A |
| **CodeGPT** | Function/class | Function-level | Via MCP | **Yes (MCP)** | Product |
| **Sourcegraph SCIP** | Symbol-level | Symbol-level | Find references | No | Enterprise |
| **Blast Radius (blast-radius.dev)** | Unknown | Unknown | Planned | No | Early stage |
| **Code-Graph-RAG** | Cross-file | Function-level | Via MCP | Yes (MCP) | OSS |
| **Augment Context Engine** | Semantic | Architecture-level | Retrieval-based | Yes (MCP) | Product |

### The Gap

**No tool provides a pre-computed, function-level dependency graph** exposed as a queryable API for AI agents that instantly answers "if I change function X, what files/functions/tests are affected?" with blast-radius computation.

CodeGPT's MCP server is closest. Greptile has the best graph but it's internal. Turbopack's incremental computation engine tracks function-level dependencies internally but exposes no public API.

---

## Tool 6: type-scaffold — Type-Driven Code Generation Constraints

### What Exists Today

**No tool explicitly generates type constraint scaffolds.**

| Tool | Type Awareness | Explicit Constraints | Error Cases | Available APIs | Model-Agnostic |
|------|---------------|---------------------|------------|---------------|---------------|
| **GitHub Copilot** | Implicit (context) | No | No | No | No |
| **Cursor** | LSP implicit | No | No | @-references | No |
| **Qodo** | Context-aware testing | Partial (test scaffold) | Partial | No | No |

### Research

**Type-Constrained Code Generation (ETH Zurich, PLDI 2025)** -- the most relevant work:
- Prefix automata enforce well-typedness during LLM decoding
- Compilation errors reduced by **more than half**, functional correctness +3.5-5.5%
- But requires custom inference pipelines and only works with open-weight models

**Key insight from ETH**: Only 3.5% of functional errors and 6% of compilation errors are syntactic. **Type errors are the dominant category of correctness failures.**

### The Gap

Type-scaffold occupies a unique position: **prompt-level type constraints** (model-agnostic, zero runtime overhead) vs. ETH's **decoding-level constraints** (open-weight only, significant latency). No product provides explicit type constraint scaffolds as LLM context.

---

## Tool 7: batch-edit — Atomic Multi-File Transactions

### What Exists Today

| Tool | Multi-File Edit | Atomic | Pre-Verification | Auto-Rollback | Transaction Grouping |
|------|---------------|--------|-----------------|--------------|---------------------|
| **Claude Code** | Parallel tool calls | No | No | Checkpoint (`/rewind`) | No |
| **Cursor** | Sequential | No | No | Checkpoint (per-prompt) | No |
| **Copilot Agent** | Iterative | No | Partial (lint) | Undo button | No |
| **Aider** | Architect/Editor | No | No | Git-based | No |
| **Codex** | Sandboxed | No | Compile+test | Manual | No |

### Industry Recognition

RedMonk 2025 survey: *"When agents can modify hundreds of files autonomously, the ability to undo becomes critical. Checkpoints need to capture conversation context, tool outputs, and intermediate states that traditional version control doesn't track."*

### The Gap

**No tool provides true transactional semantics for multi-file edits:**
1. Atomic commit (all-or-nothing)
2. Pre-flight verification (lint/typecheck/test before applying)
3. Cross-file consistency validation
4. Automatic rollback on verification failure
5. Logically related changes as a single unit

The database transaction analogy (BEGIN/COMMIT/ROLLBACK) has **not been applied to AI coding.** This is the strongest gap in the market.

---

## Tool 8: context-budget — Intelligent Context Allocation

### What Exists Today

**This is the most crowded space.**

| Competitor | Approach | Token Budget | Multi-Level Granularity | MCP | Funding |
|-----------|---------|-------------|------------------------|-----|---------|
| **Augment Code** | Semantic indexing (500K files) | Implicit | Partial | Yes | Significant |
| **Factory.ai** | Enterprise context unification | Custom compression | No | No | $50M Series B |
| **Sourcegraph Cody** | RSG + embeddings | Dynamic (4-6 snippets) | No | Yes | $245M raised |
| **Greptile** | AST + docstrings + graph | N/A | Via query | API | $25M Series A |
| **Claude Code** | Emergent (tool-calling) | Auto-compact at 95% | No | N/A | N/A |
| **Cursor** | Embeddings + @-refs | 272K window | No | N/A | $29.3B valuation |

### Research

- **cAST (EMNLP 2025)**: AST-based structural chunking, +5.5 points on RepoEval
- **CODEFILTER (COLM 2025)**: Labels chunks positive/neutral/negative, found many retrieved chunks *degrade* quality
- **Context Rot (Chroma)**: Increasing input tokens degrades LLM performance, validating selective context
- **HumanLayer ACE**: "Frequent Intentional Compaction" targeting 40-60% window utilization

### The Differentiator

The **multi-level granularity system** (full source / signatures only / types only / examples) is the novel contribution. No tool explicitly manages context at different fidelity levels based on distance from the edit target. Augment is closest but operates as a black-box retriever.

### Risk Assessment

**HIGH RISK.** Augment Code's Context Engine MCP with 70%+ improvement benchmarks is a near-direct substitute.

---

## Tool 9: error-doctor — Structured Error Diagnosis

### What Exists Today

| Tool | Error Parsing | Root Cause | AST Fix Suggestions | Cross-Tool | Agent-Oriented |
|------|-------------|-----------|--------------------|-----------|----|
| **Copilot Autofix** | SARIF/CodeQL | Security only | Code changes | No | No |
| **Raygun Robbie** | Production errors | Stack trace analysis | 3 suggestions | No | No |
| **JetBrains AI** | IDE diagnostics | Context-based | Intention actions | No | No |
| **ESLint API** | Lint errors | Rule-based | EditInfo objects | ESLint only | Partial |
| **Aider** | tree-sitter lint | AST context | LLM-generated | Multi-lang | No |

### Academic APR (Automated Program Repair)

- **62+ papers** on LLM-based APR catalogued (Jan 2022 - Oct 2025)
- **AST-based repair** is the dominant paradigm
- **SRepair** achieves $0.029/fixed bug cost
- **Repilot**: Frozen LLM + semantic Completion Engine using Eclipse JDT for constraint-guided repair

### The Gap

**No tool intercepts arbitrary error output** (compiler, linter, test runner), parses it into a unified structured form, identifies root causes across error types, and suggests concrete AST operations. Individual tools offer partial capabilities within their domains.

---

## The Complete Opportunity Map

```
                        EXISTING SOLUTIONS
                             │
         ┌───────────────────┼───────────────────┐
         │                   │                    │
    WELL-SERVED         PARTIALLY            UNADDRESSED
                         SERVED

    ┌────────────┐   ┌──────────────┐   ┌──────────────────┐
    │ Context     │   │ Codebase     │   │ AST Operation    │
    │ Selection   │   │ Skeleton     │   │ Vocabulary       │
    │             │   │              │   │ (ast-surgeon)    │
    │ Augment,    │   │ Aider RepoMap│   └──────────────────┘
    │ Factory,    │   │ partial      │   ┌──────────────────┐
    │ Greptile    │   │              │   │ Semantic Diff     │
    └────────────┘   │ Impact Graph │   │ (machine-readable)│
                     │ Greptile     │   └──────────────────┘
                     │ internal     │   ┌──────────────────┐
                     │              │   │ Atomic Multi-File │
                     │ Verify Pipe  │   │ Transactions      │
                     │ Biome JSON   │   │ (batch-edit)      │
                     │ partial      │   └──────────────────┘
                     │              │   ┌──────────────────┐
                     │ Error Doctor │   │ Type Constraint   │
                     │ APR research │   │ Scaffolds         │
                     │ partial      │   │ (type-scaffold)   │
                     └──────────────┘   └──────────────────┘
```

### Strongest Standalone: batch-edit
Clearest gap, no direct competitor, recognized industry need.

### Strongest Infrastructure: codebase-skeleton + impact-graph
Multi-resolution representation + dependency graph = the intelligence layer that Greptile and Augment build internally but don't expose.

### Most Novel: type-scaffold + ast-surgeon
Thinnest competitive landscape, validated by research (ETH PLDI 2025, TreeDiff), but adoption risk.

### Highest Risk: context-budget
Most crowded space with well-funded competitors (Augment $70%+ benchmarks, Factory $50M).

### The Combined Platform Opportunity

All 9 tools form a **pipeline** that no single company has built end-to-end:

```
context-budget → codebase-skeleton → type-scaffold → LLM REASONING
                                                          │
                impact-graph ────────────────────────────→ │
                                                          │
                                                          ▼
                                                    ast-surgeon
                                                    + batch-edit
                                                          │
                                                          ▼
                                                    verify-pipe
                                                          │
                                                    ┌─────┴─────┐
                                                  PASS        FAIL
                                                    │           │
                                                  Done    error-doctor
                                                          → fix → retry
```

**No existing tool implements this full pipeline.** The infrastructure (Oxc parsers, tree-sitter, Biome, MCP protocol) exists. The orchestration layer that composes them for machine consumption does not.

---

## Funding & Valuation Landscape (for reference)

| Company | Raised | Valuation | Relevance |
|---------|--------|-----------|-----------|
| Cursor (Anysphere) | $2.3B Series D | ~$29.3B | IDE + AI coding (acquired Graphite) |
| Factory.ai | $50M Series B | $300M | Agent-native development platform |
| Greptile | $25M Series A | ~$180M (next round) | Graph-based code review |
| Augment Code | Significant (undisclosed) | Unknown | Context Engine MCP |
| Sourcegraph | $245M total | $2.6B | Code intelligence (enterprise) |
| Codemod.com | Seed | Unknown | AST migration platform |
| Grit.io | $7M seed | Acquired by Honeycomb | AST transformation (defunct standalone) |

---

## Sources

### AST / Code Manipulation
- [ast-grep AI Tools Integration](https://ast-grep.github.io/advanced/prompting.html)
- [ast-grep MCP Server](https://github.com/ast-grep/ast-grep-mcp)
- [Codemod.com MCP](https://docs.codemod.com/model-context-protocol)
- [ASTify](https://github.com/sarthakkapila/ASTify)
- [Morph Fast Apply](https://www.morphllm.com/fast-apply-model)
- [Aider Issue #3206: AST Operations](https://github.com/Aider-AI/aider/issues/3206)
- [TreeDiff (arXiv)](https://arxiv.org/abs/2508.01473)
- [AST-T5 (ICML 2024)](https://arxiv.org/abs/2401.03003)

### Codebase Representation
- [Repomix](https://repomix.com/)
- [Aider Repository Map](https://aider.chat/2023/10/22/repomap.html)
- [Cursor Codebase Indexing](https://towardsdatascience.com/how-cursor-actually-indexes-your-codebase/)
- [Kit (Cased)](https://github.com/cased/kit)
- [LLMap](https://github.com/jbellis/llmap)
- [LongCodeZip (arXiv)](https://arxiv.org/abs/2510.00446)
- [cAST (EMNLP 2025)](https://arxiv.org/abs/2506.15655)

### Verification & Error Handling
- [Claude Code Hooks](https://code.claude.com/docs/en/hooks-guide)
- [Biome Reporters](https://biomejs.dev/reference/reporters/)
- [ESLint MCP Server](https://eslint.org/docs/latest/use/mcp)
- [Semaphore MCP Server](https://semaphore.io/blog/semaphore-mcp-server)
- [Nx Self-Healing CI](https://nx.dev/docs/features/ci-features/self-healing-ci)
- [Dagger Self-Healing Pipelines](https://dagger.io/blog/automate-your-ci-fixes-self-healing-pipelines-with-ai-agents)
- [Copilot Autofix](https://docs.github.com/en/code-security/responsible-use/responsible-use-autofix-code-scanning)
- [APR Survey (arXiv)](https://arxiv.org/html/2506.23749v1)

### Semantic Diff & Impact
- [SemanticDiff](https://semanticdiff.com/)
- [GumTree](https://github.com/GumTreeDiff/gumtree)
- [ChangeDistiller](https://www.ifi.uzh.ch/en/seal/research/tools/changeDistiller.html)
- [Difftastic](https://difftastic.wilfred.me.uk/)
- [Greptile Graph Context](https://www.greptile.com/docs/how-greptile-works/graph-based-codebase-context)
- [CodeGPT Code Graphs MCP](https://lobehub.com/mcp/judinilabs-mcp-code-graph)
- [Blast Radius](https://blast-radius.dev/)
- [Augment Context Engine MCP](https://www.augmentcode.com/blog/context-engine-mcp-now-live)
- [Sourcegraph SCIP](https://github.com/sourcegraph/scip)

### Context Management
- [Augment Code Context Engine](https://www.augmentcode.com/context-engine)
- [Factory.ai Context Compression](https://factory.ai/news/evaluating-compression)
- [HumanLayer ACE Framework](https://github.com/humanlayer/advanced-context-engineering-for-coding-agents)
- [Anthropic Context Engineering Guide](https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents)
- [CODEFILTER (COLM 2025)](https://arxiv.org/abs/2508.05970)
- [Context Rot (Chroma)](https://research.trychroma.com/context-rot)

### Type Constraints
- [Type-Constrained Code Generation (PLDI 2025)](https://arxiv.org/abs/2504.09246)
- [ETH SRI Implementation](https://github.com/eth-sri/type-constrained-code-generation)
- [Constrained Decoding for Correct Code](https://arxiv.org/html/2508.15866v1)

### Multi-File Editing
- [RedMonk: 10 Things Developers Want from Agentic IDEs](https://redmonk.com/kholterhoff/2025/12/22/10-things-developers-want-from-their-agentic-ides-in-2025/)
- [Cursor Checkpoints](https://stevekinney.com/courses/ai-development/cursor-checkpoints)
- [Copilot Agent Mode](https://code.visualstudio.com/blogs/2025/02/24/introducing-copilot-agent-mode)
