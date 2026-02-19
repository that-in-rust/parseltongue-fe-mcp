# Viz-Wizard: Network Graph Visualization

## User Requirements
- **Style**: Network graph (nodes and edges)
- **Problems to solve**:
  - Can't see the whole codebase
  - Buildings overlap/cluttered
  - Camera controls frustrating

## Solution: Force-Directed Network Graph

Replace the 3D city with a **2.5D force-directed network graph**:
- Directories as large cluster nodes
- Files as smaller nodes within clusters
- Import connections as curved edges
- Force simulation keeps nodes separated (no overlap)
- Smooth pan/zoom like a map

---

## Architecture

### Visual Design

```
┌─────────────────────────────────────────────┐
│                                             │
│    ○ components ←────────→ ○ hooks          │
│      ╱ ╲                     │              │
│    ○   ○   ○               ○ ○              │
│   files...               useAuth...         │
│                              │              │
│              ○ services ←────┘              │
│               ╱ ╲                           │
│             ○   ○   ○                       │
│                                             │
└─────────────────────────────────────────────┘
```

**Nodes**:
- **Cluster nodes** (directories): Large circles, colored by dominant category
- **File nodes**: Small circles inside clusters, colored by file category
- Size based on lines of code

**Edges**:
- Curved lines connecting nodes
- Color by connection type (cyan=import, green=call, etc.)
- Thickness by connection weight
- Animated flow direction (optional)

---

## Implementation Plan

### Phase 1: Core Graph Renderer

**New File: `src/components/canvas/NetworkGraph.tsx`**

Using Three.js with custom force simulation:
```typescript
interface GraphNode {
  id: string;
  type: 'cluster' | 'file';
  name: string;
  x: number;
  y: number;
  radius: number;
  color: string;
  clusterId?: string; // For file nodes
}

interface GraphEdge {
  source: string;
  target: string;
  color: string;
  weight: number;
}
```

**Key Features**:
- Force-directed layout that runs once on load
- Nodes repel each other (no overlap)
- Edges act as springs (connected nodes stay close)
- Clusters contain their file nodes

### Phase 2: Camera & Navigation

**Update: `src/components/canvas/CameraController.tsx`**

- Remove polar angle constraints (allow full 3D movement)
- Smooth pan with mouse drag
- Zoom to cursor position
- Double-click to focus on node
- Keyboard shortcuts: WASD for pan, scroll for zoom

**New: Minimap Component**
- Small overview in corner
- Shows current viewport
- Click to jump to location

### Phase 3: Interaction

**Node Interactions**:
- Hover: Highlight node + connected edges
- Click: Select node, show details panel
- Double-click: Zoom to node
- Drag: Reposition node (optional)

**Edge Interactions**:
- Hover: Show connection type tooltip
- Click: Highlight both connected nodes

### Phase 4: Visual Polish

- Glow effects on hover/select
- Smooth animations for all transitions
- Particle effects along edges (showing data flow)
- Labels that appear on zoom
- Legend showing node/edge colors

---

## Files to Create

| File | Purpose |
|------|---------|
| `src/components/canvas/NetworkGraph.tsx` | Main graph renderer |
| `src/components/canvas/GraphNode.tsx` | Individual node component |
| `src/components/canvas/GraphEdge.tsx` | Edge/connection component |
| `src/lib/forceLayout.ts` | Force-directed layout algorithm |
| `src/components/ui/Minimap.tsx` | Navigation minimap |

## Files to Modify

| File | Changes |
|------|---------|
| `src/components/canvas/Scene.tsx` | Use NetworkGraph instead of City |
| `src/components/canvas/CameraController.tsx` | Free movement, better zoom |
| `src/lib/mockData.ts` | Generate graph data structure |
| `src/stores/visualizationStore.ts` | Graph-specific state |

## Files to Remove/Deprecate

- `src/components/canvas/City.tsx` - Replace with NetworkGraph
- `src/components/canvas/Building.tsx` - Replace with GraphNode
- `src/components/canvas/District.tsx` - Replace with cluster nodes
- `src/components/canvas/Cluster.tsx` - Not needed
- `src/components/canvas/TreemapLayer.tsx` - Not needed

---

## Implementation Order

### Step 1: Force Layout Engine
1. Create `forceLayout.ts` with simple force simulation
2. Nodes repel, edges attract
3. Run simulation until stable

### Step 2: Basic Graph Render
1. Create `NetworkGraph.tsx`
2. Render nodes as circles (InstancedMesh for performance)
3. Render edges as lines
4. Test with small dataset

### Step 3: Camera Freedom
1. Update CameraController for free pan/zoom
2. Remove angle constraints
3. Add smooth damping

### Step 4: Interactions
1. Node hover/select highlighting
2. Edge highlighting
3. Details panel integration

### Step 5: Visual Polish
1. Glow effects
2. Labels on zoom
3. Minimap

---

## Technical Details

### Force Layout Algorithm

Simple spring-electric model:
```typescript
function simulateForces(nodes: GraphNode[], edges: GraphEdge[]) {
  // Repulsion between all nodes
  for (let i = 0; i < nodes.length; i++) {
    for (let j = i + 1; j < nodes.length; j++) {
      const dx = nodes[j].x - nodes[i].x;
      const dy = nodes[j].y - nodes[i].y;
      const dist = Math.sqrt(dx * dx + dy * dy);
      const force = REPULSION / (dist * dist);
      // Apply force to both nodes
    }
  }

  // Attraction along edges
  for (const edge of edges) {
    const source = nodes.find(n => n.id === edge.source);
    const target = nodes.find(n => n.id === edge.target);
    const dist = distance(source, target);
    const force = (dist - IDEAL_LENGTH) * SPRING_STRENGTH;
    // Pull nodes together
  }
}
```

### Instanced Rendering for Nodes

```typescript
// Single InstancedMesh for all file nodes
<instancedMesh ref={meshRef} args={[undefined, undefined, nodeCount]}>
  <circleGeometry args={[1, 32]} />
  <meshBasicMaterial />
</instancedMesh>
```

### Edge Rendering

Use `THREE.BufferGeometry` with `LineSegments`:
```typescript
const positions = new Float32Array(edgeCount * 6); // 2 points × 3 coords
const colors = new Float32Array(edgeCount * 6);    // 2 points × 3 colors

<lineSegments>
  <bufferGeometry>
    <bufferAttribute attach="attributes-position" array={positions} count={edgeCount * 2} itemSize={3} />
    <bufferAttribute attach="attributes-color" array={colors} count={edgeCount * 2} itemSize={3} />
  </bufferGeometry>
  <lineBasicMaterial vertexColors />
</lineSegments>
```

---

## Verification Plan

### Navigation Tests
- [ ] Pan with mouse drag - smooth, no stuttering
- [ ] Zoom with scroll - zooms to cursor position
- [ ] Can see entire graph when zoomed out
- [ ] Double-click zooms to node

### Visual Tests
- [ ] No node overlap after layout settles
- [ ] Cluster nodes contain their file nodes
- [ ] Edge colors match connection types
- [ ] Labels appear when zoomed in

### Performance Tests
- [ ] 60 FPS with 1000+ nodes
- [ ] Layout completes in < 2 seconds
- [ ] Smooth interactions with React repo (6700 files)

### Interaction Tests
- [ ] Hover highlights node and edges
- [ ] Click selects node, shows panel
- [ ] Can navigate to any part of graph
