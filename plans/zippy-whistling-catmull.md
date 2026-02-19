# Node Wiki — Deep-Dive Knowledge Base Per Block

## Context

The DB Simulator has 23 block types (22 with Rust WASM implementations + 1 frontend-only `schema_definition`). Each block has brief documentation (1-3 sentences per field) that flows from Rust → WASM → frontend hydration. The current "Learn" tab is a narrow accordion in a 72px sidebar — too cramped for deep educational content.

**Goal**: A wide slide-over wiki panel (~680px) that renders a complete, beginner-friendly knowledge page for any block. Phase 1 (MVP) — no AI chat, no visual explainers.

**Current documentation pipeline**:
- Rust `BlockDocumentation` (6 fields) in `block-system/src/core/block.rs:120-134`
- Serialized via `build_block_detail()` in `block-system/src/wasm_api.rs:328-427`
- Frontend `WASMBlockDetail` type in `frontend/src/wasm/types.ts:200-214`
- Hydrated into `BLOCK_REGISTRY` via `frontend/src/wasm/hydrate.ts:20-81`
- Rendered by `frontend/src/components/education/BlockEducationPanel.tsx`

---

## Phase 1a: Rust Struct Changes

### Extend `BlockDocumentation` in `block-system/src/core/block.rs`

Add 4 new fields + 1 new struct:

```rust
pub struct BlockDocumentation {
    // Existing: overview, algorithm, complexity, use_cases, tradeoffs, examples
    // NEW:
    pub motivation: String,
    pub parameter_guide: HashMap<String, String>,  // param_id → explanation
    pub alternatives: Vec<Alternative>,
    pub suggested_questions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alternative {
    pub block_type: String,
    pub comparison: String,
}
```

### Update WASM serialization in `block-system/src/wasm_api.rs`

| Change | Location |
|--------|----------|
| Add `AlternativeResponse` struct | After `BlockDocResponse` (~line 260) |
| Add 4 new fields to `BlockDocResponse` | Lines 251-259 |
| Update `build_block_detail()` to pipe new fields | Lines 333-343 |

### Expand content in all 22 Rust block files

Each block's `build_metadata()` needs:
- `overview`: expand to 2-4 paragraphs (beginner-friendly, with analogy)
- `motivation`: NEW — what problem does it solve, what happens without it
- `algorithm`: expand with pseudocode formatting using `\n` line breaks
- `parameter_guide`: NEW — per-parameter deep explanation (HashMap)
- `alternatives`: NEW — comparisons with sibling blocks (Vec<Alternative>)
- `suggested_questions`: NEW — 3 AI starter questions
- `use_cases`: add 1-2 more entries
- `tradeoffs`: add 1-2 more entries
- `examples`: expand with brief context per example

**22 block files to modify:**

| Category | Blocks | File paths |
|----------|--------|------------|
| Storage (4) | heap_file, lsm_tree, clustered, columnar | `categories/storage/*.rs` |
| Index (3) | btree, hash_index, covering_index | `categories/index/*.rs` |
| Buffer (2) | lru_buffer, clock_buffer | `categories/buffer/*.rs` |
| Execution (5) | sequential_scan, index_scan, filter, sort, hash_join | `categories/execution/*.rs` |
| Concurrency (2) | row_lock, mvcc | `categories/concurrency/*.rs` |
| Transaction (1) | wal | `categories/transaction/wal.rs` |
| Optimization (2) | bloom_filter, statistics_collector | `categories/optimization/*.rs` |
| Partitioning (1) | hash_partitioner | `categories/partitioning/hash_partitioner.rs` |
| Distribution (1) | replication | `categories/distribution/replication.rs` |
| Compression (1) | dictionary_encoding | `categories/compression/dictionary_encoding.rs` |

**Verification**: `cargo check && cargo test`

---

## Phase 1b: Frontend Type + Hydration Updates

### Update `frontend/src/wasm/types.ts`

Add to `BlockDocumentation` interface:
```typescript
motivation: string;
parameter_guide: Record<string, string>;
alternatives: WASMAlternative[];
suggested_questions: string[];
```
Add `WASMAlternative` interface: `{ blockType: string; comparison: string }`

### Update `frontend/src/types/blocks.ts`

Add to `BlockDocumentation` interface:
```typescript
motivation?: string;
parameterGuide?: Record<string, string>;
alternatives?: BlockAlternative[];
suggestedQuestions?: string[];
```
Add `BlockAlternative` interface: `{ blockType: string; comparison: string }`

### Update `frontend/src/wasm/hydrate.ts`

Add 4 new fields to the merge in `hydrateBlockRegistry()` (lines 39-49):
```typescript
motivation: detail.documentation.motivation,
parameterGuide: detail.documentation.parameter_guide,
alternatives: detail.documentation.alternatives?.map(a => ({
  blockType: a.blockType, comparison: a.comparison,
})),
suggestedQuestions: detail.documentation.suggested_questions,
```

**Verification**: `npx tsc --noEmit`

---

## Phase 1c: Wiki Store + Markdown Utility

### Create `frontend/src/stores/wikiStore.ts`

Zustand store following existing patterns (aiStore, challengeStore):
```typescript
interface WikiState {
  isOpen: boolean;
  blockType: string | null;
  history: string[];        // breadcrumb trail for back navigation
  open(blockType: string): void;
  close(): void;
  navigateTo(blockType: string): void;  // push current to history, switch
  back(): void;                          // pop history
}
```

### Extract + extend `MarkdownLite` → `frontend/src/lib/markdown.tsx`

Extract `MarkdownLite` and `formatInline` from `AITeachingPanel.tsx` (lines 271-339) into a shared `Markdown` component. Extend with:
- `##` and `###` heading support
- Bullet lists (`- item`)
- Numbered lists (`1. item`)
- Link support `[text](url)`

Update `AITeachingPanel.tsx` to import from `@/lib/markdown`.

**Verification**: `npx tsc --noEmit`

---

## Phase 1d: Wiki UI Components

### New directory: `frontend/src/components/wiki/`

| Component | Purpose |
|-----------|---------|
| `NodeWikiPanel.tsx` | Slide-over container: `fixed right-0 top-0 bottom-0 w-[680px] z-40`, backdrop `bg-black/20`, Escape to close |
| `WikiHeader.tsx` | Block name, category badge (colored), complexity badges, back arrow (if history), close button |
| `WikiTOC.tsx` | Sticky left column (~160px), section links, `IntersectionObserver` highlights active section |
| `WikiContent.tsx` | Scrollable right area, renders all sections with `id="wiki-{section}"` anchors |
| `WikiSection.tsx` | Generic section wrapper: icon + heading + body (always expanded, not collapsible) |
| `WikiAlgorithm.tsx` | Monospace pseudocode renderer (`<pre>` with proper formatting) |
| `WikiAlternatives.tsx` | Clickable cards linking to other block wikis via `wikiStore.navigateTo()` |
| `WikiMetrics.tsx` | Metrics table (name, type badge, unit, description) |
| `WikiReferences.tsx` | Reference list with type badges (Paper/Book/Blog/Implementation) |

### Sections rendered in WikiContent (in order):

1. **Overview** — `doc.overview` rendered via Markdown
2. **Why It Exists** — `doc.motivation` rendered via Markdown
3. **Algorithm** — `doc.algorithm` rendered via WikiAlgorithm (monospace)
4. **Complexity** — Time/Space badges (reuse `ComplexityBadge` pattern from BlockEducationPanel)
5. **Parameters Explained** — `doc.parameterGuide` entries with parameter name headers
6. **When To Use** — `doc.useCases` bullet list
7. **Tradeoffs** — `doc.tradeoffs` bullet list
8. **Compared To** — `doc.alternatives` as WikiAlternatives cards
9. **Real-World Usage** — `doc.examples` with context
10. **Metrics** — `block.metricDefinitions` as WikiMetrics table
11. **References** — `block.references` as WikiReferences list

### Mount in App.tsx

Add `<NodeWikiPanel />` after `<ArchitectureAnnotations />`.

**Verification**: `npx tsc --noEmit && npx vite build`

---

## Phase 1e: Entry Points

### 1. ParameterPanel Learn tab — "Open full wiki" button

**File**: `frontend/src/components/layout/ParameterPanel.tsx`

Add a button below `<BlockEducationPanel>` in the Learn tab content:
```tsx
<button onClick={() => useWikiStore.getState().open(data.blockType)}>
  Open full wiki →
</button>
```

### 2. Double-click on canvas node

**File**: `frontend/src/components/layout/Canvas.tsx`

Add `onNodeDoubleClick` handler to ReactFlow:
```tsx
const handleNodeDoubleClick = useCallback((_: React.MouseEvent, node: Node) => {
  useWikiStore.getState().open((node.data as BlockNodeData).blockType);
}, []);
```

### 3. BlockPalette info icon

**File**: `frontend/src/components/layout/BlockPalette.tsx`

Change the info icon click from inline expand to `useWikiStore.getState().open(block.type)`. Remove the inline expand state/UI.

### 4. Keyboard shortcut `?` when block selected

**File**: `frontend/src/components/layout/Canvas.tsx`

In the existing keyboard handler, add:
```tsx
if (e.key === '?' && selectedNodeId) {
  const node = useCanvasStore.getState().nodes.find(n => n.id === selectedNodeId);
  if (node) useWikiStore.getState().open((node.data as BlockNodeData).blockType);
}
```

**Verification**: Test all 4 entry points manually in the browser.

---

## Phase 1f: Final Verification

```bash
# Rust
cargo check && cargo test

# Frontend
cd frontend && npx tsc --noEmit && npx vite build

# Visual smoke test
npm run dev
# Double-click B-tree block → wiki opens with full content
# Click "Compared To" Hash Index → navigates, back button works
# TOC highlights active section on scroll
# Press ? with block selected → wiki opens
# Palette info icon → wiki opens
```

---

## Implementation Order Summary

```
1a. Rust struct changes (block.rs) + content for all 22 blocks
1b. WASM bridge (wasm_api.rs) + frontend types (types.ts, hydrate.ts)
1c. wikiStore.ts + extract Markdown utility
1d. Wiki UI components (7 files)
1e. Entry points (ParameterPanel, Canvas, BlockPalette)
1f. Verify build + visual test
```

Phases 1a and 1b are sequential (Rust first, then bridge, then frontend types). Phases 1c and 1d can overlap (store first, then UI). Phase 1e depends on 1d.
