---
layout: default
title: Project Structure
permalink: /project-structure/
---

# Project Structure

## Multi-File Projects

A SignalCanvas project is **hierarchical**. Templates compose recursively: devices make up rooms, rooms make up buildings, buildings make up campuses. Each level that renders on a canvas has its own `.patch` + `.layout.json` pair.

A `.patch` file IS a template at every scale. Whether it describes a single Yamaha CL5 or an entire campus, it is the same construct. The hierarchy is not a special feature: it is templates importing other templates via `use` statements.

### The Three Layers

| Layer | File | What it stores | Editable by humans? |
|-------|------|----------------|---------------------|
| **Signal flow** | `.patch` | Templates, instances, connections, bridges, signals, config, routes, buses, rings | Yes |
| **Canvas layout** | `.layout.json` | Block positions, group boxes, viewport state | No (UI-generated) |
| **Project manifest** | `project.json` | Name, author, root file, library declarations, external dependencies | Yes |

**The rule:** If it affects signal routing, it is PatchLang. If it affects appearance, it is `.layout.json`. If it is project metadata, it is `project.json`.

### Namespace Resolution

`use` statements use dotted namespaces. Dots map to path separators:

```
use buildings.foh         →  buildings/foh.patch
use lib.custom_tf5        →  lib/custom_tf5.patch
use yamaha                →  yamaha.patch
```

Resolution order:

1. **Project-local:** Relative to the project root directory
2. **Project libraries:** Listed in `project.json` `libraries` array
3. **External dependencies:** Declared in `project.json` `dependencies`, resolved from the backend's library tier system (stock → org → user)

If a namespace cannot be resolved, the compiler emits an error.

### Flat Namespace

All templates share a single namespace after `use` resolution. If two `.patch` files define a template with the same name, the compiler emits an error with file and line information for both definitions.

Future versions may add scoped namespaces if real-world usage demands it.

### Drill-Down Navigation

Clicking a block on the canvas drills into it. The frontend decides what to show based on the compiler output:

- **Template has sub-instances and connections** (drillable): Load that level's `.patch` + `.layout.json` and render the sub-level canvas.
- **Template has only ports** (leaf device): Show the device's port details. No drill-down.

The compiler has already resolved the entire `use` graph before anything renders. The frontend knows the contents of every template.

### Missing `.layout.json`

If a `.layout.json` file does not exist for a `.patch` file, the frontend offers to auto-generate one using the SignalCanvasRouter's auto-layout engine. The user can accept or manually arrange blocks.

---

## `project.json` Manifest

### Design Principle

`project.json` declares what cannot be inferred from the `use` graph. Sub-level files are **not listed**; the compiler discovers them by walking `use` statements from the root. Only metadata, library declarations, and external dependencies appear in the manifest.

### Schema

```json
{
  "name": "Hillsong MTG",
  "author": "A. Engineer",
  "created": "2026-03-15",
  "description": "Main campus signal flow for Hillsong MTG broadcast",
  "root": "campus.patch",
  "libraries": [
    "lib/custom_yamaha_tf5.patch"
  ],
  "dependencies": {
    "@stock/shure-wireless": "^1.0.0",
    "@hillsong/dante-infrastructure": "^2.1.0"
  }
}
```

| Field | Required | Description |
|-------|----------|-------------|
| `name` | Yes | Human-readable project name |
| `root` | Yes | Path to the root `.patch` file (entry point) |
| `author` | No | Project author |
| `created` | No | ISO 8601 creation date |
| `description` | No | Project description |
| `libraries` | No | Array of project-local library `.patch` file paths that may not be referenced by any `use` statement yet |
| `dependencies` | No | Map of external library package names to version constraints (future feature) |

### What Is NOT in `project.json`

The sub-level file list is inferred from the `use` graph; adding a `use` statement makes a file part of the project, so there's no separate list to keep in sync. Layout sidecar mappings are convention-based (`foo.patch` maps to `foo.layout.json`), so no explicit mapping is needed. The resolved file graph is the compiler's output, not user-maintained data.

### Why Sub-Levels Are Inferred

Two engineers adding rooms to different buildings never touch `project.json`; they just create `.patch` files and add `use` statements. No merge conflicts on a shared manifest. The `use` graph in `.patch` files is the single source of truth for project hierarchy.

### External Dependencies

The `dependencies` field in `project.json` declares external library packages by name and SemVer constraint:

```json
{
  "dependencies": {
    "@stock/yamaha": "^2.0.0",
    "@stock/shure-wireless": "^1.0.0"
  }
}
```

Keys use `@tier/package-name` notation. Values are SemVer constraint strings (caret, tilde, exact, range; same semantics as npm). The compiler resolves the latest version of each package satisfying all stated constraints from the backend library tier system. See D021 for the full versioning design.

**Resolution order:**
1. Local `use` resolution takes precedence: a template found locally is used regardless of the version constraint.
2. Otherwise, the compiler resolves from the backend library tier using the constraint.
3. If two constraints for the same package conflict, the compiler errors.

**Template version annotations:**
Templates in the stock library carry `@version("semver")` annotations:
```
template Rio3224 @version("2.1.0") { ... }
```
Breaking port schema changes (renaming/removing/direction-changing a port) require a major version bump. Adding ports is non-breaking.

### Layout Sidecar Convention

Every `.patch` file can have a layout sidecar. The sidecar path is derived by convention:

```
campus.patch          →  campus.layout.json
buildings/foh.patch   →  buildings/foh.layout.json
```

If the sidecar does not exist, the level has no saved layout. The frontend can auto-generate one.

---

## `.layout.json` Schema

Canvas positions and visual state for one level of the hierarchy. Keyed by instance name (must match instance names in the corresponding `.patch` file exactly).

```json
{
  "version": 1,
  "positions": {
    "FOH_Console": { "x": 500, "y": 200 },
    "Stage_Left_Box": { "x": -200, "y": 400 },
    "Dante_Switch": { "x": 150, "y": 300, "collapsed": true }
  },
  "groupBoxes": [
    { "id": "stage-area", "label": "Stage", "x": -300, "y": 350, "width": 600, "height": 300, "color": "#e0f0e0" }
  ],
  "viewport": { "x": 0, "y": 0, "zoom": 1.0 }
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `version` | integer | Yes | Schema version. Currently `1`. |
| `positions` | object | Yes | Map of instance name → `{x, y, collapsed?}` |
| `groupBoxes` | array | No | Visual annotation boxes. Each has `id`, `label`, `x`, `y`, `width`, `height`, `color?`. |
| `viewport` | object | No | Last viewport state: `{x, y, zoom}` |

### Rules

- Keys in `positions` MUST match instance names in the `.patch` file exactly (case-sensitive, underscore-separated).
- If an instance exists in the `.patch` but not in the layout, the frontend auto-lays it out.
- If a key exists in the layout but not in the `.patch`, ignore it silently (orphaned after rename).
- `groupBoxes[].id` is a local identifier (not a UUID). Use a slugified label or any short string.
- Each `.layout.json` stores the viewport state for its own level. Drilling into a sub-level and coming back restores the parent's viewport.
