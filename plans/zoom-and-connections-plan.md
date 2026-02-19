# Implementation Plan: Zoom Navigation & Connection Visibility

## Problems to Solve

1. **No zoom level navigation** - ForceGraph only has "all clusters" and "single expanded cluster" views, missing the satellite → regional → district → street progression
2. **Aggregated connections hide details** - Can't see which specific files connect clusters

---

## Phase 1: Semantic Zoom Levels for ForceGraph

### Concept
Instead of camera-distance-based zoom (which doesn't make sense for 2D), use **semantic zoom levels**:

| Level | What's Shown | Connections |
|-------|--------------|-------------|
| **Satellite** | Clusters only | Cluster-to-cluster links (aggregated) |
| **Regional** | Clusters + file counts | Cluster links with thickness by weight |
| **District** | Clusters expanded, files visible | File-to-file within cluster + cross-cluster |
| **Street** | Single cluster's files | All file connections with labels |

### Implementation

**1. Add zoom level state to store** (`visualizationStore.ts`):
```typescript
// New state
graphZoomLevel: 'satellite' | 'regional' | 'district' | 'street';
setGraphZoomLevel: (level) => void;
```

**2. Add zoom controls UI** - Buttons or slider to switch between levels

**3. Update ForceGraph rendering** based on zoom level:
- Satellite: Only cluster nodes, aggregated links
- Regional: Cluster nodes with size indicating file count, weighted links
- District: Show files around their cluster (radial layout)
- Street: Flat view of all files with full connections

---

## Phase 2: Show Actual File Connections Between Clusters

### Current Problem
When cluster A connects to cluster B, we only show ONE line. But there might be 50 file-to-file imports.

### Solution Options

**Option A: Weighted Cluster Links**
- Keep single line but make thickness proportional to connection count
- Add tooltip: "23 imports between src/components and src/hooks"
- Clicking the link shows a panel listing all file pairs

**Option B: Bundled Edge Visualization**
- Show multiple curved lines bundled together
- Each line represents one file-to-file connection
- Lines fan out near the clusters to connect to specific files

**Option C: Expand-on-Demand**
- Single click on cluster link → expands to show file-level connections
- Creates a temporary "zoomed" view of just those two clusters and their files

### Recommended: Option A + C Combined
1. Show weighted cluster links (thickness = connection count)
2. Click cluster link → expand both connected clusters, show actual file connections
3. Add "Connection Details" panel listing all file pairs

---

## Phase 3: Implementation Steps

### Step 1: Weighted Cluster Links (2-3 hours)
- [ ] Store connection weight when creating cluster-to-cluster links
- [ ] Render link thickness based on weight
- [ ] Add tooltip showing connection count on hover

### Step 2: Connection Details Panel (2-3 hours)
- [ ] Create new UI component `ClusterConnectionPanel`
- [ ] When cluster link is clicked, show panel with:
  - Source cluster name
  - Target cluster name
  - List of file pairs with connection types
- [ ] Click file pair → navigate to that file

### Step 3: Zoom Level Controls (3-4 hours)
- [ ] Add zoom level buttons to UI (satellite/regional/district/street icons)
- [ ] Implement different rendering for each level:
  - Satellite: clusters only
  - Regional: clusters with weighted edges
  - District: clusters + nearby files (partial expansion)
  - Street: full file view

### Step 4: Semantic Navigation (2-3 hours)
- [ ] Double-click cluster = zoom to district level for that cluster
- [ ] Double-click file = zoom to street level
- [ ] Breadcrumb shows: Satellite > Regional > [Cluster Name] > [File Name]
- [ ] Back button or breadcrumb click to zoom out

---

## Visual Mockup

```
SATELLITE VIEW (all clusters)
┌─────────────────────────────────────┐
│     ○ components ══════ ○ hooks     │  ← Thick line = many connections
│           ║                         │
│     ○ utils ──────── ○ services     │  ← Thin line = few connections
└─────────────────────────────────────┘

DISTRICT VIEW (expanded cluster)
┌─────────────────────────────────────┐
│  ┌─ components ─┐                   │
│  │ • Button.tsx ├───────→ ○ hooks   │
│  │ • Modal.tsx  ├───────→           │
│  │ • Form.tsx   │                   │
│  └──────────────┘                   │
└─────────────────────────────────────┘

STREET VIEW (connection expanded)
┌─────────────────────────────────────┐
│  • Button.tsx ──import──→ useAuth   │
│  • Button.tsx ──import──→ useTheme  │
│  • Modal.tsx ───import──→ useModal  │
│  • Form.tsx ────import──→ useForm   │
└─────────────────────────────────────┘
```

---

## Files to Modify

| File | Changes |
|------|---------|
| `src/stores/visualizationStore.ts` | Add graphZoomLevel state |
| `src/components/canvas/ForceGraph.tsx` | Zoom-aware rendering, weighted links |
| `src/components/ui/ZoomControls.tsx` | New component for zoom buttons |
| `src/components/ui/ClusterConnectionPanel.tsx` | New panel for connection details |
| `src/components/ui/Breadcrumbs.tsx` | Update for semantic zoom navigation |

---

## Questions for User

1. **Zoom controls**: Prefer buttons (Satellite/Regional/District/Street) or a slider?
2. **Connection expansion**: When clicking a cluster link, should it:
   - (A) Open a side panel listing connections, OR
   - (B) Visually expand both clusters in the graph?
3. **Priority**: Start with weighted links + details panel, or zoom levels first?
