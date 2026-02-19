# Scale Viz-Wizard to Handle Massive Repos (10K → 100K → 1M+ files)

## The Problem

Currently, each building is an **individual React component** with its own `useFrame` callback, 11 materials, and separate mesh. Each connection is a separate `<Line>` component with its own `useFrame`. The API caps at 500 files. This approach collapses at ~2K objects. We need to support repos like Linux kernel (70K+ files), Chromium (300K+ files), and monorepos with millions of files.

## Current Bottlenecks (from codebase analysis)

| Bottleneck | Current | Impact |
|---|---|---|
| Building rendering | Individual `<Building>` component per file | N React components, N useFrame callbacks, 11×N materials |
| Connection rendering | Individual `<SingleConnection>` per road | N Line components, N useFrame callbacks, N bezier curves |
| Layout computation | `repositoryToCityData()` runs synchronously on main thread | Blocks UI for large datasets |
| API file cap | `MAX_FILES = 500` in route.ts | Can't even fetch large repos |
| No culling | Every object renders regardless of camera view | GPU waste on off-screen objects |
| No LOD | Binary simplified/full only (via District `simplified` prop) | No distance-based optimization |

## Architecture: 3 Phases

---

## Phase 1: GPU Instancing (Target: 10,000 files)

Replace N individual meshes with **2 instanced draw calls** (buildings + glow planes). This is the highest-impact change.

### Files to Create

#### 1. `src/components/canvas/InstancedBuildings.tsx`

Replaces per-building rendering with a single `THREE.InstancedMesh`.

**Approach:**
- One `InstancedMesh` for buildings (box geometry shared), one for glow planes
- Per-instance data via `InstancedBufferAttribute`:
  - `instanceColor` (vec3) — building color based on language/type
  - `instanceEmissive` (vec3) — glow color for highlights
  - `instanceMatrix` — position, rotation, scale (set via `setMatrixAt`)
- Single `useFrame` callback manages ALL animations:
  - Hovered building: scale up via `setMatrixAt` + `instanceMatrix.needsUpdate = true`
  - Selected building: emissive boost via attribute update
  - Insights/findUsages highlights: update `instanceColor` + `instanceEmissive` for affected indices
  - Float animation: only animate the ~1 selected/hovered building, not all

**Key design:**
```typescript
interface InstancedBuildingsProps {
  buildings: BuildingData[];  // flat array from all districts
  buildingIndex: Map<string, number>;  // fileId → instance index
}
```

- Build a `Map<string, number>` (fileId → instance index) for O(1) lookups
- On highlight mode changes, batch-update only affected instance attributes
- Raycasting: use `instanceId` from R3F's `onPointerOver` event to identify which building

**Materials:**
- Single `MeshStandardMaterial` with `vertexColors: true`
- Highlight states handled by updating per-instance color attributes, not swapping materials
- Selected/hovered state: update the specific instance's color + emissive in the attribute buffer

#### 2. `src/components/canvas/BatchedConnections.tsx`

Replaces per-connection `<Line>` components with a single `BufferGeometry`.

**Approach:**
- Pre-compute all bezier curves into a single `Float32Array` of positions
- Single `THREE.LineSegments` with `BufferGeometry`
- Per-segment colors via `BufferAttribute` (for connection type styling)
- Connection filtering: toggle visibility by zeroing out positions of hidden segments (or use a separate visibility attribute)
- No per-connection `useFrame` — opacity handled via single material

**Key design:**
```typescript
interface BatchedConnectionsProps {
  roads: RoadData[];
  connectionFilters: Record<string, boolean>;
}
```

#### 3. `src/lib/buildingLookup.ts`

Index utilities for fast building lookups used across the app.

```typescript
export function buildBuildingIndex(cityData: CityData): Map<string, { districtIdx: number; buildingIdx: number; building: BuildingData }>;
export function flattenBuildings(cityData: CityData): BuildingData[];
```

### Files to Modify

#### 4. `src/components/canvas/City.tsx`

- Replace `districts.map(d => <District>)` with `<InstancedBuildings>` + `<BatchedConnections>`
- Keep district ground planes as simple meshes (these are cheap — one per directory)
- Flatten all buildings from all districts into a single array, pass to `<InstancedBuildings>`

#### 5. `src/stores/visualizationStore.ts`

- Add `buildingIndex: Map<string, number>` to state (built once when cityData loads)
- Add `flatBuildings: BuildingData[]` derived from cityData
- Modify `loadRepoDirectly` and `loadFixture` to build the index on load

#### 6. `src/app/api/repo/[owner]/[name]/route.ts`

- Raise `MAX_FILES` from 500 to 10,000
- Add pagination: if tree has >10K eligible files, return first 10K with `hasMore: true` and a continuation token

### Performance Impact

| Metric | Before (500 files) | After (10K files) |
|---|---|---|
| Draw calls | ~1,000 (2 per building) | ~4 (2 instanced + ground planes + connections) |
| useFrame callbacks | ~500+ | ~2 (one for buildings, one optional for connections) |
| Material instances | ~5,500 | ~2 |
| React components in scene | ~600+ | ~20 (districts ground planes + 2 instanced + controls) |

---

## Phase 2: Spatial Indexing + LOD (Target: 100,000 files)

At 100K buildings, even instanced rendering needs culling and LOD to stay interactive.

### Files to Create

#### 7. `src/lib/spatialIndex.ts` — Quadtree

```typescript
export class SpatialQuadtree {
  constructor(bounds: { x: number; z: number; width: number; depth: number }, maxDepth?: number);
  insert(buildingId: string, x: number, z: number): void;
  queryFrustum(frustum: THREE.Frustum): string[];
  queryRadius(center: THREE.Vector3, radius: number): string[];
}
```

- Buildings indexed by their XZ position
- `queryFrustum` returns only visible building IDs for the current camera
- Used by InstancedBuildings to set count on the InstancedMesh (only render visible)

#### 8. `src/components/canvas/LODBuildings.tsx` — 3-Tier LOD

Replaces `InstancedBuildings` with distance-based detail levels:

| Tier | Distance | Rendering |
|---|---|---|
| Full | < 20 units | Full instanced boxes with glow, labels on hover |
| Simple | 20–60 units | Instanced boxes, no glow, no interaction |
| Dot | > 60 units | `THREE.Points` with per-point color (single draw call for all distant buildings) |

- Each tier is a separate InstancedMesh (or Points for dots)
- On camera move: re-bucket buildings into tiers based on distance from camera
- Throttle re-bucketing to every 200ms (not every frame)

#### 9. `src/lib/layoutWorker.ts` — Web Worker for Layout

Move `repositoryToCityData()` off the main thread:

```typescript
// Worker message protocol
type WorkerInput = { type: 'computeLayout'; repo: RepositoryData };
type WorkerOutput = { type: 'layoutComplete'; cityData: CityData };
```

- Post repo data to worker → receive cityData back
- Show loading progress while computing
- For 100K files, layout computation could take 1-2 seconds — must not block UI

### Files to Modify

#### 10. `src/components/canvas/InstancedBuildings.tsx`

- Integrate frustum culling from `SpatialQuadtree`
- Only set `instancedMesh.count` to visible instances
- Re-order instance buffer to pack visible instances contiguously

#### 11. `src/components/canvas/City.tsx`

- Use `LODBuildings` instead of `InstancedBuildings`
- Subscribe to camera distance for LOD tier switching

#### 12. `src/lib/mockData.ts`

- Optimize `repositoryToCityData()` for 100K files:
  - Use typed arrays instead of object arrays for position data
  - Pack-style layout algorithm (bin-packing instead of simple grid)
  - Return `Float32Array` for positions/dimensions alongside the CityData objects

### Performance Impact

| Metric | Phase 1 (10K) | Phase 2 (100K) |
|---|---|---|
| Rendered instances | 10,000 | ~2,000–5,000 (frustum culled) |
| Interaction targets | 10,000 | ~500 (only full-detail tier) |
| Layout computation | ~200ms main thread | ~1-2s in worker |
| Memory | ~50MB | ~200MB (spatial index + buffers) |

---

## Phase 3: Backend Aggregation + Streaming (Target: 1,000,000+ files)

For truly massive repos, we can't send all files to the client. The backend must aggregate and stream.

### Files to Create

#### 13. `src/app/api/repo/[owner]/[name]/summary/route.ts` — Aggregated Overview

Returns directory-level summaries instead of individual files:

```typescript
// Response shape
{
  directories: [
    {
      path: "src/components",
      fileCount: 342,
      totalLines: 45000,
      languages: { TypeScript: 280, CSS: 62 },
      avgComplexity: 35,
      children: ["src/components/ui", "src/components/canvas", ...],
    },
    // ...
  ],
  topFiles: FileNode[],  // Top 100 most important files
  totalFiles: 1_200_000,
  connections: Connection[],  // Directory-level connections only
}
```

- Each directory becomes a single building (aggregated)
- Click to expand → fetches children via streaming API
- Top 100 files always shown as individual buildings

#### 14. `src/app/api/repo/[owner]/[name]/district/[path]/route.ts` — Streaming District Detail

When user zooms into a directory, fetch its files:

```typescript
// GET /api/repo/owner/name/district/src%2Fcomponents
// Returns individual files for that directory
{
  files: FileNode[],
  connections: Connection[],
  hasChildren: boolean,
}
```

- Paginated: max 1000 files per request
- Files loaded on-demand as user navigates the 3D city

#### 15. `src/lib/virtualDistricts.ts` — Dynamic Load/Unload

```typescript
export class VirtualDistrictManager {
  loadDistrict(path: string): Promise<void>;     // Fetch + add to scene
  unloadDistrict(path: string): void;             // Remove from scene, keep in cache
  isLoaded(path: string): boolean;
  getLoadedCount(): number;
}
```

- Keeps max ~20 districts loaded at once
- LRU eviction when limit exceeded
- Uses `IndexedDB` to cache fetched district data (avoid re-fetching)

#### 16. `src/lib/indexedDBCache.ts` — Client-Side Cache

```typescript
export class RepoCache {
  static async get(repoKey: string, districtPath: string): Promise<DistrictData | null>;
  static async set(repoKey: string, districtPath: string, data: DistrictData): Promise<void>;
  static async clear(repoKey: string): Promise<void>;
}
```

- Cache repo data in IndexedDB (persists across sessions)
- Keyed by `owner/name@sha` to invalidate on new commits
- Max cache size: 500MB per repo

### Files to Modify

#### 17. `src/stores/visualizationStore.ts`

- Add `loadedDistricts: Map<string, DistrictData>` state
- Add `expandDistrict(path: string)` action — fetches detail, adds buildings to scene
- Add `collapseDistrict(path: string)` action — removes buildings, shows aggregate
- Modify `loadRepoDirectly` to accept summary-level data

#### 18. `src/components/canvas/City.tsx`

- Render aggregate buildings for unloaded districts
- When district is loaded, swap aggregate → individual buildings
- Double-click on aggregate building → trigger `expandDistrict`

#### 19. `src/app/api/repo/[owner]/[name]/route.ts`

- For repos > 10K files: return summary response instead of full file list
- Add `mode=summary` query param
- Detect repo size from GitHub tree response `truncated` field

### Performance Impact

| Metric | Phase 2 (100K) | Phase 3 (1M+) |
|---|---|---|
| Initial data transfer | ~10MB | ~200KB (summary only) |
| Time to first render | ~3s | ~1s |
| Max client memory | ~200MB | ~100MB (loaded districts only) |
| Total viewable files | 100K | Unlimited (streamed on demand) |

---

## Implementation Order

### Phase 1 (GPU Instancing) — Do First
1. `src/lib/buildingLookup.ts` — index utilities
2. `src/components/canvas/InstancedBuildings.tsx` — instanced building renderer
3. `src/components/canvas/BatchedConnections.tsx` — batched connection renderer
4. `src/components/canvas/City.tsx` — swap District rendering for instanced
5. `src/stores/visualizationStore.ts` — add building index, flat buildings array
6. `src/app/api/repo/[owner]/[name]/route.ts` — raise MAX_FILES to 10K
7. Verify: load a ~5K file repo, confirm smooth 60fps

### Phase 2 (LOD + Culling) — After Phase 1 works
8. `src/lib/spatialIndex.ts` — quadtree
9. `src/lib/layoutWorker.ts` — offload layout to worker
10. `src/components/canvas/LODBuildings.tsx` — 3-tier LOD
11. Update City.tsx + InstancedBuildings for culling integration
12. Optimize `repositoryToCityData()` for typed arrays
13. Verify: load a ~50K file repo, confirm smooth interaction

### Phase 3 (Streaming) — After Phase 2 works
14. Summary + district API routes
15. `src/lib/indexedDBCache.ts` — client cache
16. `src/lib/virtualDistricts.ts` — dynamic load/unload
17. Update store + City for aggregate/expand model
18. Verify: load Linux kernel repo, confirm progressive loading

**Total: ~10 new files, ~5 modified files across all phases**

---

## Verification

### Phase 1
1. `npm run dev` → navigate to `/explore`
2. Enter a medium repo (e.g., `facebook/react`) → should render ~3K-5K buildings smoothly
3. Hover/click buildings → single building highlights correctly
4. Insights panel "Show me" → instanced buildings highlight in batch
5. FPS counter should show ≥45fps with 5K buildings
6. `npm run build` → no type errors

### Phase 2
7. Enter a large repo (e.g., `kubernetes/kubernetes`) → loads without freezing UI
8. Zoom in → buildings transition from dots → boxes → full detail
9. Pan around → only visible buildings render (check draw call count in browser devtools)
10. Layout computation happens in background with loading indicator

### Phase 3
11. Enter `torvalds/linux` → summary loads in ~1s
12. See aggregate buildings for top-level directories
13. Double-click a directory → expands into individual file buildings
14. Navigate away and back → data loads from IndexedDB cache
15. Memory stays under 200MB even with multiple districts expanded
