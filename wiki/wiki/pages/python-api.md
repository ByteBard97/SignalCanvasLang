---
title: Python API
tags: [python, pyo3, django, api, backend]
sources: [patchlang-design-guide/compiler, docs/python]
updated: 2026-04-16
---

# Python API

**Source:** `docs/python.md`, `docs/patchlang-design-guide/compiler.md`
**Type:** API reference

## Summary

PatchLang provides Python bindings via PyO3 and maturin, producing a native extension module (`patchlang_python`). Most functions return **JSON strings** — call `json.loads()`. Exceptions: `validate` returns `bool`, `resolve_uses` returns a native `list[str]`.

## Requirements

- Python 3.9+
- Rust toolchain (for building from source)
- maturin (`pipx install maturin`)

## Building

```bash
./scripts/build-python.sh
```

Creates `.venv/` and installs `patchlang_python` in development mode.

---

## Core Functions

### `parse(source: str) -> str`
```python
result = json.loads(patchlang_python.parse(source))
# { "program": {...} | None, "errors": [...] }
```

### `validate(source: str) -> bool`
```python
if not patchlang_python.validate(source):
    raise ValueError("invalid .patch file")
```

### `check(source: str) -> str`
Parse + DRC. Use instead of `parse()` when you want DRC warnings.
```python
result = json.loads(patchlang_python.check(source))
# { "program": {...} | None, "errors": [...], "diagnostics": [...] }
for d in result['diagnostics']:
    print(f"[{d['severity']}] {d['message']}")
```

### `resolve_uses(source: str) -> list[str]`
Returns a native Python list — no `json.loads()` needed.
```python
namespaces = patchlang_python.resolve_uses(source)
# ["buildings.foh", "yamaha"]
```

### `compile_project(files: dict[str, str], entry: str) -> str`
```python
files = {
    "main.patch": open("main.patch").read(),
    "vendor/dante.patch": open("vendor/dante.patch").read(),
}
result = json.loads(patchlang_python.compile_project(files, "main.patch"))
# { "program": {...}, "errors": [...], "diagnostics": [...], "files": [...],
#   "templateFiles": {...}, "useGraph": {...} }
```

### `parse_manifest(json: str) -> str`
```python
result = json.loads(patchlang_python.parse_manifest(open("project.json").read()))
# { "manifest": {...} | None, "errors": [...] }
```

### `validate_layout(json: str) -> str`
```python
result = json.loads(patchlang_python.validate_layout(layout_json_str))
# { "valid": bool, "errors": [...] }
```

### `validate_project_consistency(patch: str, layout: str) -> str`
```python
result = json.loads(patchlang_python.validate_project_consistency(patch_src, layout_src))
# { "valid": bool, "errors": [...], "warnings": [...] }
```

---

## ID Generation

```python
port_id = patchlang_python.generate_port_id("Console", "CL5", "Dante_In", 1)
# "pl::CL5::Dante_In_1"

port_id_scalar = patchlang_python.generate_port_id("Console", "CL5", "Dante_In")
# "pl::CL5::Dante_In"  (index defaults to None)

route_id = patchlang_python.generate_route_id("CL5", "Mic_In", "Dante_Out")
slot_id = patchlang_python.generate_slot_id("CL5", "MY_Slot")
```

Python's `index` defaults to `None` for scalar ports (unlike WASM which uses `-1`).

---

## Builder API (`ProgramBuilder`)

```python
from patchlang_python import ProgramBuilder
import json

prog = ProgramBuilder()                          # empty
prog = ProgramBuilder.from_source(patch_source)  # from existing .patch

# Mutations
prog.add_template(template_json)
prog.add_instance(instance_json)
conn_id = prog.add_connect(source_json, target_json, props_json)
prog.add_route("FOH", "MADI_In", 41, "LINE_Out", 1)
prog.set_label("FOH", "Dante_In", 1, "Lead Vocal")
prog.set_label("FOH", "Dante_In", 2, "Kick", {          # optional properties dict
    "phantom": "true",
    "insert_send": "Ext_Out[3], Ext_Out[10]",           # channel insert legs (#31)
})
prog.remove_connect(conn_id)
cascade_json = prog.remove_instance("Stage_Left")  # returns cascade JSON string

# Output
source = prog.format()          # → .patch text
diags_json = prog.check()       # → diagnostics JSON
ast_json = prog.to_json()       # → AST JSON
```

All errors raise `ValueError`. `remove_instance` returns cascade result as JSON string.

---

## Django Integration Pattern

```python
import json
from django.core.exceptions import ValidationError
import patchlang_python

def validate_patch_content(content: str) -> dict:
    result = json.loads(patchlang_python.parse(content))
    if result['errors']:
        err = result['errors'][0]
        raise ValidationError(f"Invalid PatchLang: {err['message']}")
    return result['program']

def extract_project_summary(content: str) -> dict:
    program = validate_patch_content(content)
    statements = program['statements']
    return {
        'template_count': len([s for s in statements if s['type'] == 'Template']),
        'instance_count': len([s for s in statements if s['type'] == 'Instance']),
        'connection_count': len([s for s in statements if s['type'] == 'Connect']),
        'device_names': [i['name'] for i in statements if i['type'] == 'Instance'],
    }
```

---

## Relation to Other Wiki Pages

- [[compiler-architecture]] — Rust implementation behind these functions
- [[builder-api]] — full builder API reference
- [[wasm-api]] — JavaScript/TypeScript equivalent
- [[backend-data-model]] — how the Django backend uses patchlang_python
