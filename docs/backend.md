---
layout: default
title: Backend
permalink: /backend/
---

# Backend Data Model

## Database Schema

### Project (metadata container)

```python
class Project(models.Model):
    facility = ForeignKey('organizations.Facility', on_delete=CASCADE)
    name = CharField(max_length=255)
    slug = SlugField(max_length=255)
    manifest = JSONField(default=dict, blank=True)       # project.json contents
    project_summary = JSONField(default=dict, blank=True) # for list views / search
    thumbnail_url = URLField(blank=True, null=True)
    created_at = DateTimeField(auto_now_add=True)
    updated_at = DateTimeField(auto_now=True)
    deleted_at = DateTimeField(null=True, blank=True)
```

Project holds only metadata. No signal flow content.

### ProjectPage (one row per canvas level)

```python
class ProjectPage(models.Model):
    project = ForeignKey(Project, related_name='pages', on_delete=CASCADE)
    parent = ForeignKey('self', null=True, blank=True, related_name='children', on_delete=CASCADE)
    path = CharField(max_length=500, db_index=True)       # e.g., "buildings/foh"
    name = CharField(max_length=255)                       # human-readable: "Front of House"
    patch_content = TextField(default='', blank=True)
    layout_json = JSONField(default=dict, blank=True)
    parse_result = JSONField(default=dict, blank=True)
    sort_order = PositiveIntegerField(default=0)
    created_at = DateTimeField(auto_now_add=True)
    updated_at = DateTimeField(auto_now=True)
```

Every canvas level, including the root, is a `ProjectPage`. The root page has `parent=None`. The `path` field matches the file path on disk.

### LibraryFile (shared templates)

```python
class LibraryFile(models.Model):
    facility = ForeignKey('organizations.Facility', on_delete=CASCADE)
    name = CharField(max_length=255)
    slug = SlugField(max_length=255)
    patch_content = TextField()
    created_at = DateTimeField(auto_now_add=True)
    updated_at = DateTimeField(auto_now=True)
```

Library `.patch` files scoped to a facility. Shared across projects.

### ProjectSnapshot (version history)

```python
class ProjectSnapshot(models.Model):
    project = ForeignKey(Project, related_name='snapshots', on_delete=CASCADE)
    version_number = PositiveIntegerField()
    message = CharField(max_length=500, blank=True)
    created_at = DateTimeField(auto_now_add=True)
    created_by = ForeignKey(User, null=True, on_delete=SET_NULL)
```

A snapshot captures the state of all pages at a point in time. Individual page versions are stored in a related table:

```python
class PageVersion(models.Model):
    snapshot = ForeignKey(ProjectSnapshot, related_name='page_versions', on_delete=CASCADE)
    page = ForeignKey(ProjectPage, on_delete=CASCADE)
    patch_content = TextField()
    layout_json = JSONField(default=dict)
```

This avoids monolithic JSON blobs. Each page's content is stored in its own row. Restoring a snapshot updates each page individually. Diffing two snapshots compares page versions row by row.

## API Endpoints

```
GET  /projects/{id}/                      → Project metadata + root page content
GET  /projects/{id}/pages/                → Flat list of all pages (for sidebar tree)
GET  /projects/{id}/pages/{path}/         → Single page content (drill-down load)
PUT  /projects/{id}/pages/{path}/         → Save one level
POST /projects/{id}/snapshots/            → Create snapshot (captures all pages)
GET  /projects/{id}/snapshots/{ver}/      → Retrieve snapshot with page versions
GET  /facilities/{id}/libraries/          → List shared library files
```

## Migration Strategy

**Phase 1 (additive, no breaking changes):**
1. Create `ProjectPage`, `LibraryFile`, `PageVersion` tables.
2. Data migration: for each existing `Project`, create a `ProjectPage` with `parent=None`, copying `patch_content` and `layout_json`.
3. Keep deprecated `patch_content` and `layout_json` fields on `Project` during transition.
4. New page-based API endpoints coexist alongside existing endpoints.

**Phase 2 (after frontend migration):**
1. Frontend switches to page-based endpoints.
2. Remove deprecated fields from `Project`.
3. Remove old snapshot format.
