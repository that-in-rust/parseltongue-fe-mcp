# Landing Page Sub-1-Second Load Optimization Plan

## Context

The OriginByte landing page (Docusaurus 3.8.1 + React 19) currently loads ~153KB of critical-path resources before showing content. The goal is to get LCP under 1 second. The site uses SSG (static HTML), which is already a great foundation -- the bottlenecks are oversized assets, conflicting directives, and unnecessary bundle weight.

---

## Optimizations (Ordered by Impact)

### 1. Fix Favicon (saves ~62KB on critical path)
**File:** `docusaurus.config.ts`
- Change `favicon: 'img/profile.jpg'` (66KB!) to `favicon: 'img/favicon.ico'` (3.5KB already exists)

### 2. Fix Profile Image `loading="lazy"` Conflict (improves LCP 200-500ms)
**File:** `src/pages/LandingPage/index.tsx`
- The profile image is above-the-fold AND preloaded, but has `loading="lazy"` which contradicts the preload
- Remove `loading="lazy"`, add `fetchPriority="high"`

### 3. Replace react-icons with Inline SVGs (saves ~15-30KB JS)
**File:** `src/pages/LandingPage/index.tsx`, `package.json`
- Only 5 icons used from 3 icon sets -- each set pulls in GenIcon wrapper infrastructure
- Copy the exact SVG paths from the current `build/index.html` (already rendered there by SSG)
- Remove `react-icons` from dependencies

### 4. Enable `@docusaurus/faster` (saves ~40-80KB JS)
**Files:** `package.json`, `docusaurus.config.ts`
- Install `@docusaurus/faster` and add `future: { experimental_faster: true }`
- Uses SWC instead of Babel for better minification and smaller bundles

### 5. Convert Profile Image to WebP (saves ~40-50KB LCP)
**File:** `static/img/profile.webp` (new), `src/pages/LandingPage/index.tsx`
- Current: 66KB JPEG displayed at 150x150
- Target: 300x300 WebP at quality 80 (~15-20KB)
- Update import to use the new WebP file

### 6. Defer SplashCursor to User Interaction (improves TTI)
**File:** `src/pages/LandingPage/index.tsx`
- Currently lazy-loaded but still triggers on mount
- Gate loading behind first `mousemove`/`touchstart` with `requestIdleCallback` fallback (3s timeout)
- Frees main thread during initial load

### 7. Delete Unused Static Assets (cleanup)
**Files to delete:**
- `static/img/profile-original.jpg` (1.1MB unused backup)
- `static/img/undraw_docusaurus_mountain.svg` (31KB)
- `static/img/undraw_docusaurus_react.svg` (35KB)
- `static/img/undraw_docusaurus_tree.svg` (12KB)
- `static/img/docusaurus.png` (5KB)
- `static/img/logo.svg` (6KB)
- `src/components/HomepageFeatures/` (dead code, never imported)

### 8. Inline Critical CSS (improves FCP 100-300ms)
**File:** `docusaurus.config.ts`
- Add `headTags` with a small inline `<style>` block containing essential landing page styles (background, layout, image)
- This prevents flash of unstyled content while the full 75KB CSS loads

---

## Expected Results

| Metric | Before | After (estimated) |
|---|---|---|
| Critical path resources | ~87KB | ~12KB |
| LCP image | 66KB | ~20KB |
| Main JS bundle | 484KB | ~380-420KB |
| Favicon | 66KB | 3.5KB |
| **Total before LCP** | **~153KB** | **~32KB** |

At typical 4G speeds, 32KB transfers in ~20ms. Combined with DNS/TLS (~200ms), server response (~100ms), and parsing (~50ms), LCP should land comfortably under 1 second.

---

## Verification

1. Run `npm run build` and verify build succeeds
2. Run `npx docusaurus serve` and open landing page
3. Use Chrome DevTools Lighthouse (Performance audit) to measure FCP and LCP
4. Verify all 5 badge icons render correctly (inline SVGs)
5. Verify SplashCursor still works on mouse movement
6. Verify profile image displays correctly in WebP
7. Verify favicon shows in browser tab
