use std::sync::Mutex;

use wasm_bindgen::prelude::*;

use patchlang::builder::PatchProgramBuilder;

/// Sentinel value indicating a non-ranged (single) port in [`generate_port_id`].
pub const NO_INDEX: i32 = -1;

// ---------------------------------------------------------------------------
// Handle-based builder store
// ---------------------------------------------------------------------------

static BUILDERS: Mutex<Vec<Option<PatchProgramBuilder>>> = Mutex::new(Vec::new());

/// Allocate a slot for a builder, returning the handle (index).
fn allocate_builder(builder: PatchProgramBuilder) -> u32 {
    let mut store = BUILDERS.lock().unwrap();
    // Reuse the first empty slot.
    for (i, slot) in store.iter_mut().enumerate() {
        if slot.is_none() {
            *slot = Some(builder);
            return i as u32;
        }
    }
    let idx = store.len();
    store.push(Some(builder));
    idx as u32
}

/// Borrow a builder immutably by handle and call `f`.
fn with_builder<R>(handle: u32, f: impl FnOnce(&PatchProgramBuilder) -> R) -> Result<R, String> {
    let store = BUILDERS.lock().unwrap();
    match store.get(handle as usize) {
        Some(Some(b)) => Ok(f(b)),
        _ => Err(format!("invalid handle {handle}")),
    }
}

/// Borrow a builder mutably by handle and call `f`.
fn with_builder_mut<R>(
    handle: u32,
    f: impl FnOnce(&mut PatchProgramBuilder) -> R,
) -> Result<R, String> {
    let mut store = BUILDERS.lock().unwrap();
    match store.get_mut(handle as usize) {
        Some(Some(b)) => Ok(f(b)),
        _ => Err(format!("invalid handle {handle}")),
    }
}

/// Format a JSON error string.
fn json_err(msg: &str) -> String {
    let escaped = msg.replace('\\', "\\\\").replace('"', "\\\"");
    format!(r#"{{"error":"{escaped}"}}"#)
}

/// Return `{"ok":true}`.
fn json_ok() -> String {
    r#"{"ok":true}"#.to_string()
}

// ---------------------------------------------------------------------------
// Builder WASM exports
// ---------------------------------------------------------------------------

/// Create an empty program builder. Returns a handle (u32).
#[wasm_bindgen]
pub fn create_program() -> u32 {
    allocate_builder(PatchProgramBuilder::new())
}

/// Create a builder from PatchLang source. Returns handle as JSON number on
/// success or `{"error":"..."}` on parse failure.
#[wasm_bindgen]
pub fn create_program_from_source(source: &str) -> String {
    let result = patchlang::parse(source);
    if !result.is_valid() {
        let msgs: Vec<String> = result.errors.iter().map(|e| e.to_string()).collect();
        return json_err(&msgs.join("; "));
    }
    let handle = allocate_builder(PatchProgramBuilder::from_program(result.program));
    handle.to_string()
}

/// Format the program as canonical PatchLang source text.
#[wasm_bindgen]
pub fn format_program(handle: u32) -> String {
    with_builder(handle, |b| b.format()).unwrap_or_else(|e| json_err(&e))
}

/// Get the program as JSON (TS-compatible shape).
#[wasm_bindgen]
pub fn get_program_json(handle: u32) -> String {
    with_builder(handle, |b| b.to_json()).unwrap_or_else(|e| json_err(&e))
}

/// Run DRC checks and return diagnostics as JSON array.
#[wasm_bindgen]
pub fn check_program(handle: u32) -> String {
    with_builder(handle, |b| {
        let diags = b.check();
        serde_json::to_string(&diags).unwrap_or_else(|e| json_err(&e.to_string()))
    })
    .unwrap_or_else(|e| json_err(&e))
}

/// Free a builder handle.
#[wasm_bindgen]
pub fn free_program(handle: u32) {
    let mut store = BUILDERS.lock().unwrap();
    if let Some(slot) = store.get_mut(handle as usize) {
        *slot = None;
    }
}

/// Add a template from JSON. Returns `{"ok":true}` or `{"error":"..."}`.
#[wasm_bindgen]
pub fn add_template(handle: u32, template_json: &str) -> String {
    let decl: patchlang::ast::TemplateDecl = match serde_json::from_str(template_json) {
        Ok(d) => d,
        Err(e) => return json_err(&e.to_string()),
    };
    with_builder_mut(handle, |b| match b.add_template(decl) {
        Ok(()) => json_ok(),
        Err(e) => json_err(&e.to_string()),
    })
    .unwrap_or_else(|e| json_err(&e))
}

/// Remove a template by name. Returns `{"ok":true}` or `{"error":"..."}`.
#[wasm_bindgen]
pub fn remove_template(handle: u32, name: &str) -> String {
    with_builder_mut(handle, |b| match b.remove_template(name) {
        Ok(()) => json_ok(),
        Err(e) => json_err(&e.to_string()),
    })
    .unwrap_or_else(|e| json_err(&e))
}

/// Add an instance from JSON. Returns `{"ok":true}` or `{"error":"..."}`.
#[wasm_bindgen]
pub fn add_instance(handle: u32, instance_json: &str) -> String {
    let decl: patchlang::ast::InstanceDecl = match serde_json::from_str(instance_json) {
        Ok(d) => d,
        Err(e) => return json_err(&e.to_string()),
    };
    with_builder_mut(handle, |b| match b.add_instance(decl) {
        Ok(()) => json_ok(),
        Err(e) => json_err(&e.to_string()),
    })
    .unwrap_or_else(|e| json_err(&e))
}

/// Remove an instance by name. Returns cascade result as JSON or `{"error":"..."}`.
#[wasm_bindgen]
pub fn remove_instance(handle: u32, name: &str) -> String {
    with_builder_mut(handle, |b| match b.remove_instance(name) {
        Ok(cascade) => {
            serde_json::to_string(&serde_json::json!({
                "ok": true,
                "removed_connects": cascade.removed_connects,
                "removed_bridges": cascade.removed_bridges,
                "removed_configs": cascade.removed_configs,
                "removed_ring_members": cascade.removed_ring_members,
                "removed_signal_origins": cascade.removed_signal_origins,
                "removed_stream_sources": cascade.removed_stream_sources,
            }))
            .unwrap_or_else(|e| json_err(&e.to_string()))
        }
        Err(e) => json_err(&e.to_string()),
    })
    .unwrap_or_else(|e| json_err(&e))
}

/// Add a connection. Returns `{"ok":true,"id":"..."}` or `{"error":"..."}`.
#[wasm_bindgen]
pub fn add_connect(
    handle: u32,
    source_json: &str,
    target_json: &str,
    props_json: &str,
) -> String {
    let source: patchlang::ast::PortRef = match serde_json::from_str(source_json) {
        Ok(d) => d,
        Err(e) => return json_err(&e.to_string()),
    };
    let target: patchlang::ast::PortRef = match serde_json::from_str(target_json) {
        Ok(d) => d,
        Err(e) => return json_err(&e.to_string()),
    };
    let props: Vec<patchlang::ast::KeyValue> = match serde_json::from_str(props_json) {
        Ok(d) => d,
        Err(e) => return json_err(&e.to_string()),
    };
    with_builder_mut(handle, |b| match b.add_connect(source, target, props) {
        Ok(id) => format!(r#"{{"ok":true,"id":"{id}"}}"#),
        Err(e) => json_err(&e.to_string()),
    })
    .unwrap_or_else(|e| json_err(&e))
}

/// Remove a connection by ID. Returns `{"ok":true}` or `{"error":"..."}`.
#[wasm_bindgen]
pub fn remove_connect(handle: u32, id: &str) -> String {
    with_builder_mut(handle, |b| match b.remove_connect(id) {
        Ok(()) => json_ok(),
        Err(e) => json_err(&e.to_string()),
    })
    .unwrap_or_else(|e| json_err(&e))
}

/// Add an internal route to an instance. Returns `{"ok":true}` or `{"error":"..."}`.
#[wasm_bindgen]
pub fn add_route(
    handle: u32,
    instance: &str,
    from_port: &str,
    from_ch: u32,
    to_port: &str,
    to_ch: u32,
) -> String {
    with_builder_mut(handle, |b| {
        match b.add_route(instance, from_port, from_ch, to_port, to_ch) {
            Ok(()) => json_ok(),
            Err(e) => json_err(&e.to_string()),
        }
    })
    .unwrap_or_else(|e| json_err(&e))
}

/// Replace all routes on an instance from JSON. Returns `{"ok":true}` or `{"error":"..."}`.
#[wasm_bindgen]
pub fn set_routes(handle: u32, instance: &str, routes_json: &str) -> String {
    let routes: Vec<patchlang::ast::RouteEntry> = match serde_json::from_str(routes_json) {
        Ok(d) => d,
        Err(e) => return json_err(&e.to_string()),
    };
    with_builder_mut(handle, |b| match b.set_routes(instance, routes) {
        Ok(()) => json_ok(),
        Err(e) => json_err(&e.to_string()),
    })
    .unwrap_or_else(|e| json_err(&e))
}

/// Set a channel label. Returns `{"ok":true}` or `{"error":"..."}`.
#[wasm_bindgen]
pub fn set_label(
    handle: u32,
    instance: &str,
    port: &str,
    index: u32,
    label: &str,
    props_json: &str,
) -> String {
    let props: std::collections::HashMap<String, String> = match serde_json::from_str(props_json) {
        Ok(d) => d,
        Err(e) => return json_err(&e.to_string()),
    };
    with_builder_mut(handle, |b| {
        match b.set_label(instance, port, index, label, props) {
            Ok(()) => json_ok(),
            Err(e) => json_err(&e.to_string()),
        }
    })
    .unwrap_or_else(|e| json_err(&e))
}

/// Set a slot assignment on an instance. `slot_index` of -1 means None.
/// Returns `{"ok":true}` or `{"error":"..."}`.
#[wasm_bindgen]
pub fn set_slot(
    handle: u32,
    instance: &str,
    slot_name: &str,
    slot_index: i32,
    card_template: &str,
) -> String {
    let index = if slot_index < 0 {
        None
    } else {
        Some(slot_index as u32)
    };
    with_builder_mut(handle, |b| {
        match b.set_slot(instance, slot_name, index, card_template) {
            Ok(()) => json_ok(),
            Err(e) => json_err(&e.to_string()),
        }
    })
    .unwrap_or_else(|e| json_err(&e))
}

/// Add a bus to an instance from JSON. Returns `{"ok":true}` or `{"error":"..."}`.
///
/// `bus_json` must be a JSON object matching:
/// ```json
/// {
///   "name": "Link_1",
///   "label": null,
///   "inputs": [{"instance": null, "port": "Fader", "index": {"elements": [{"type": "Single", "value": 1}]}}],
///   "outputs": [
///     {
///       "label": "Link 1-L",
///       "destinations": [{"instance": null, "port": "MADI_1_Out", "index": {"elements": [{"type": "Single", "value": 1}]}}],
///       "span": {"start": 0, "end": 0}
///     }
///   ],
///   "span": {"start": 0, "end": 0}
/// }
/// ```
/// An unrouted output has `"destinations": []`.
/// `"label"` on the bus itself is an optional display name; pass `null` if not needed.
///
/// Optional `"insert_send"` / `"insert_return"` carry the bus's insert legs (issue #31)
/// as arrays of the same port-ref shape as `"inputs"`. Both default to `[]`, so payloads
/// written against an earlier version still deserialize. Order is significant — `[L]`
/// mono, `[L, R]` stereo — and the legs are never grouped or channel-unioned the way
/// `"inputs"` are, so a port may legitimately repeat. Each leg needs a single-channel
/// index; ranges are not accepted.
#[wasm_bindgen]
pub fn add_bus(handle: u32, instance: &str, bus_json: &str) -> String {
    let bus: patchlang::ast::BusEntry = match serde_json::from_str(bus_json) {
        Ok(d) => d,
        Err(e) => return json_err(&e.to_string()),
    };
    with_builder_mut(handle, |b| match b.add_bus(instance, bus) {
        Ok(()) => json_ok(),
        Err(e) => json_err(&e.to_string()),
    })
    .unwrap_or_else(|e| json_err(&e))
}

/// Add a stream from JSON. Returns `{"ok":true}` or `{"error":"..."}`.
#[wasm_bindgen]
pub fn add_stream(handle: u32, stream_json: &str) -> String {
    let decl: patchlang::ast::StreamDecl = match serde_json::from_str(stream_json) {
        Ok(d) => d,
        Err(e) => return json_err(&e.to_string()),
    };
    with_builder_mut(handle, |b| match b.add_stream(decl) {
        Ok(()) => json_ok(),
        Err(e) => json_err(&e.to_string()),
    })
    .unwrap_or_else(|e| json_err(&e))
}

/// Add a signal from JSON. Returns `{"ok":true}` or `{"error":"..."}`.
#[wasm_bindgen]
pub fn add_signal(handle: u32, signal_json: &str) -> String {
    let decl: patchlang::ast::SignalDecl = match serde_json::from_str(signal_json) {
        Ok(d) => d,
        Err(e) => return json_err(&e.to_string()),
    };
    with_builder_mut(handle, |b| match b.add_signal(decl) {
        Ok(()) => json_ok(),
        Err(e) => json_err(&e.to_string()),
    })
    .unwrap_or_else(|e| json_err(&e))
}

/// Add a ring from JSON. Returns `{"ok":true}` or `{"error":"..."}`.
#[wasm_bindgen]
pub fn add_ring(handle: u32, ring_json: &str) -> String {
    let decl: patchlang::ast::RingDecl = match serde_json::from_str(ring_json) {
        Ok(d) => d,
        Err(e) => return json_err(&e.to_string()),
    };
    with_builder_mut(handle, |b| match b.add_ring(decl) {
        Ok(()) => json_ok(),
        Err(e) => json_err(&e.to_string()),
    })
    .unwrap_or_else(|e| json_err(&e))
}

/// Add a member to an existing ring. Empty port string is treated as None.
#[wasm_bindgen]
pub fn add_ring_member(handle: u32, ring_name: &str, instance: &str, port: &str) -> String {
    let port_opt = if port.is_empty() { None } else { Some(port) };
    with_builder_mut(handle, |b| {
        match b.add_ring_member(ring_name, instance, port_opt) {
            Ok(()) => json_ok(),
            Err(e) => json_err(&e.to_string()),
        }
    })
    .unwrap_or_else(|e| json_err(&e))
}

/// Add a connection with explicit channel mappings.
/// `mappings_json`: JSON array of `[fromCh, toCh, "label"]` triples.
/// Returns `{"ok":true,"ids":["conn_1",...]}` or `{"error":"..."}`.
#[wasm_bindgen]
pub fn add_connect_mapped(
    handle: u32,
    source_instance: &str,
    source_port: &str,
    target_instance: &str,
    target_port: &str,
    mappings_json: &str,
    props_json: &str,
) -> String {
    let mappings: Vec<(u32, u32, String)> = match serde_json::from_str(mappings_json) {
        Ok(d) => d,
        Err(e) => return json_err(&e.to_string()),
    };
    let props: Vec<patchlang::ast::KeyValue> =
        serde_json::from_str(props_json).unwrap_or_default();
    with_builder_mut(handle, |b| {
        match b.add_connect_mapped(
            source_instance,
            source_port,
            target_instance,
            target_port,
            mappings,
            props,
        ) {
            Ok(ids) => {
                let ids_json: Vec<String> = ids.iter().map(|id| format!(r#""{id}""#)).collect();
                format!(r#"{{"ok":true,"ids":[{}]}}"#, ids_json.join(","))
            }
            Err(e) => json_err(&e.to_string()),
        }
    })
    .unwrap_or_else(|e| json_err(&e))
}

/// Set RF channel labels on an rf-system instance.
/// `labels_json`: JSON array of `[channelIndex, "label", {}]` triples.
/// Returns `{"ok":true}` or `{"error":"..."}`.
#[wasm_bindgen]
pub fn set_rf_labels(handle: u32, instance: &str, labels_json: &str) -> String {
    let labels: Vec<(u32, String, Vec<patchlang::ast::KeyValue>)> =
        match serde_json::from_str(labels_json) {
            Ok(d) => d,
            Err(e) => return json_err(&e.to_string()),
        };
    with_builder_mut(handle, |b| match b.set_rf_labels(instance, labels) {
        Ok(()) => json_ok(),
        Err(e) => json_err(&e.to_string()),
    })
    .unwrap_or_else(|e| json_err(&e))
}

/// Add a bridge from JSON port refs. Returns `{"ok":true}` or `{"error":"..."}`.
#[wasm_bindgen]
pub fn add_bridge(handle: u32, source_json: &str, target_json: &str) -> String {
    let source: patchlang::ast::PortRef = match serde_json::from_str(source_json) {
        Ok(d) => d,
        Err(e) => return json_err(&e.to_string()),
    };
    let target: patchlang::ast::PortRef = match serde_json::from_str(target_json) {
        Ok(d) => d,
        Err(e) => return json_err(&e.to_string()),
    };
    with_builder_mut(handle, |b| match b.add_bridge(source, target) {
        Ok(()) => json_ok(),
        Err(e) => json_err(&e.to_string()),
    })
    .unwrap_or_else(|e| json_err(&e))
}

/// Parse library files and store their templates in the builder's library context.
/// files_json: { "lib/yamaha.patch": "template CL5 { ... }\n...", ... }
/// Returns: {"ok": true} or {"error": "parse error in lib/yamaha.patch: ..."}
#[wasm_bindgen]
pub fn set_builder_library(handle: u32, files_json: &str) -> String {
    let files: std::collections::HashMap<String, String> = match serde_json::from_str(files_json) {
        Ok(f) => f,
        Err(e) => return json_err(&format!("invalid JSON: {e}")),
    };

    let mut templates = std::collections::HashMap::new();
    for (path, source) in &files {
        let parse_result = patchlang::parse(source);
        if !parse_result.errors.is_empty() {
            return json_err(&format!(
                "parse error in {}: {}",
                path, parse_result.errors[0]
            ));
        }
        for stmt in &parse_result.program.statements {
            if let patchlang::ast::Statement::Template(t) = stmt {
                templates.insert(t.name.clone(), t.clone());
            }
        }
    }

    with_builder_mut(handle, |builder| {
        builder.set_library(patchlang::builder::LibraryContext { templates });
    })
    .map(|_| json_ok())
    .unwrap_or_else(|e| json_err(&e))
}

/// Compile from the builder's in-memory AST + library context.
/// No format/reparse round-trip.
/// Returns: CompileToGraphResult JSON (same shape as compile_to_graph)
#[wasm_bindgen]
pub fn compile_program_to_graph(handle: u32) -> String {
    with_builder(handle, |builder| {
        let result = patchlang::graph::compile_program_to_graph(
            builder.program(),
            builder.library(),
        );
        serde_json::to_string(&result).unwrap_or_else(|e| json_err(&e.to_string()))
    })
    .unwrap_or_else(|e| json_err(&e))
}

/// Update a template by name from JSON. Returns `{"ok":true}` or `{"error":"..."}`.
#[wasm_bindgen]
pub fn update_template(handle: u32, name: &str, template_json: &str) -> String {
    let decl: patchlang::ast::TemplateDecl = match serde_json::from_str(template_json) {
        Ok(d) => d,
        Err(e) => return json_err(&e.to_string()),
    };
    with_builder_mut(handle, |b| match b.update_template(name, decl) {
        Ok(()) => json_ok(),
        Err(e) => json_err(&e.to_string()),
    })
    .unwrap_or_else(|e| json_err(&e))
}

/// Get a template by name as JSON. Returns JSON or `{"error":"..."}`.
#[wasm_bindgen]
pub fn get_template(handle: u32, name: &str) -> String {
    with_builder(handle, |b| match b.get_template(name) {
        Some(t) => serde_json::to_string(t).unwrap_or_else(|e| json_err(&e.to_string())),
        None => json_err(&format!("template '{name}' not found")),
    })
    .unwrap_or_else(|e| json_err(&e))
}

/// Get all template names as JSON array. Returns `["name1", "name2", ...]`.
#[wasm_bindgen]
pub fn template_names(handle: u32) -> String {
    with_builder(handle, |b| {
        let names = b.template_names();
        serde_json::to_string(&names).unwrap_or_else(|e| json_err(&e.to_string()))
    })
    .unwrap_or_else(|e| json_err(&e))
}

/// Get an instance by name as JSON. Returns JSON or `{"error":"..."}`.
#[wasm_bindgen]
pub fn get_instance(handle: u32, name: &str) -> String {
    with_builder(handle, |b| match b.get_instance(name) {
        Some(i) => serde_json::to_string(i).unwrap_or_else(|e| json_err(&e.to_string())),
        None => json_err(&format!("instance '{name}' not found")),
    })
    .unwrap_or_else(|e| json_err(&e))
}

/// Update all properties on an instance. `props_json`: `{"key": "value", ...}`.
/// Returns `{"ok":true}` or `{"error":"..."}`.
#[wasm_bindgen]
pub fn update_instance_properties(handle: u32, name: &str, props_json: &str) -> String {
    let props: std::collections::HashMap<String, String> = match serde_json::from_str(props_json) {
        Ok(d) => d,
        Err(e) => return json_err(&e.to_string()),
    };
    with_builder_mut(handle, |b| match b.update_instance_properties(name, props) {
        Ok(()) => json_ok(),
        Err(e) => json_err(&e.to_string()),
    })
    .unwrap_or_else(|e| json_err(&e))
}

/// Remove a slot assignment from an instance. `slot_index` of -1 means None.
/// Returns `{"ok":true}` or `{"error":"..."}`.
#[wasm_bindgen]
pub fn remove_slot(handle: u32, instance: &str, slot_name: &str, slot_index: i32) -> String {
    let index = if slot_index < 0 {
        None
    } else {
        Some(slot_index as u32)
    };
    with_builder_mut(handle, |b| {
        match b.remove_slot(instance, slot_name, index) {
            Ok(_cascade) => json_ok(),
            Err(e) => json_err(&e.to_string()),
        }
    })
    .unwrap_or_else(|e| json_err(&e))
}

/// Update properties on a connection by ID. Returns `{"ok":true}` or `{"error":"..."}`.
#[wasm_bindgen]
pub fn update_connect_properties(handle: u32, id: &str, props_json: &str) -> String {
    let props: Vec<patchlang::ast::KeyValue> = match serde_json::from_str(props_json) {
        Ok(d) => d,
        Err(e) => return json_err(&e.to_string()),
    };
    with_builder_mut(handle, |b| match b.update_connect_properties(id, props) {
        Ok(()) => json_ok(),
        Err(e) => json_err(&e.to_string()),
    })
    .unwrap_or_else(|e| json_err(&e))
}

/// Add a bridge group from JSON. Returns `{"ok":true}` or `{"error":"..."}`.
#[wasm_bindgen]
pub fn add_bridge_group(handle: u32, bridge_group_json: &str) -> String {
    let decl: patchlang::ast::BridgeGroupDecl = match serde_json::from_str(bridge_group_json) {
        Ok(d) => d,
        Err(e) => return json_err(&e.to_string()),
    };
    with_builder_mut(handle, |b| match b.add_bridge_group(decl) {
        Ok(()) => json_ok(),
        Err(e) => json_err(&e.to_string()),
    })
    .unwrap_or_else(|e| json_err(&e))
}

/// Clear all routes on an instance. Returns `{"ok":true}` or `{"error":"..."}`.
#[wasm_bindgen]
pub fn clear_routes(handle: u32, instance: &str) -> String {
    with_builder_mut(handle, |b| match b.clear_routes(instance) {
        Ok(()) => json_ok(),
        Err(e) => json_err(&e.to_string()),
    })
    .unwrap_or_else(|e| json_err(&e))
}

/// Update a bus on an instance (full replacement by name).
/// Returns `{"ok":true}` or `{"error":"..."}`.
///
/// `bus_json` must be a JSON object matching:
/// ```json
/// {
///   "name": "Link_1",
///   "label": null,
///   "inputs": [{"instance": null, "port": "Fader", "index": {"elements": [{"type": "Single", "value": 1}]}}],
///   "outputs": [
///     {
///       "label": "Link 1-L",
///       "destinations": [{"instance": null, "port": "MADI_1_Out", "index": {"elements": [{"type": "Single", "value": 1}]}}],
///       "span": {"start": 0, "end": 0}
///     }
///   ],
///   "span": {"start": 0, "end": 0}
/// }
/// ```
/// An unrouted output has `"destinations": []`.
/// `"label"` on the bus itself is an optional display name; pass `null` if not needed.
///
/// Optional `"insert_send"` / `"insert_return"` carry the bus's insert legs (issue #31)
/// as arrays of the same port-ref shape as `"inputs"`. Both default to `[]`, so payloads
/// written against an earlier version still deserialize. Order is significant — `[L]`
/// mono, `[L, R]` stereo — and the legs are never grouped or channel-unioned the way
/// `"inputs"` are, so a port may legitimately repeat. Each leg needs a single-channel
/// index; ranges are not accepted.
#[wasm_bindgen]
pub fn update_bus(handle: u32, instance: &str, bus_name: &str, bus_json: &str) -> String {
    let bus: patchlang::ast::BusEntry = match serde_json::from_str(bus_json) {
        Ok(d) => d,
        Err(e) => return json_err(&e.to_string()),
    };
    with_builder_mut(handle, |b| match b.update_bus(instance, bus_name, bus) {
        Ok(()) => json_ok(),
        Err(e) => json_err(&e.to_string()),
    })
    .unwrap_or_else(|e| json_err(&e))
}

/// Add a flag from JSON. Returns `{"ok":true}` or `{"error":"..."}`.
#[wasm_bindgen]
pub fn add_flag(handle: u32, flag_json: &str) -> String {
    let decl: patchlang::ast::FlagDecl = match serde_json::from_str(flag_json) {
        Ok(d) => d,
        Err(e) => return json_err(&e.to_string()),
    };
    with_builder_mut(handle, |b| match b.add_flag(decl) {
        Ok(()) => json_ok(),
        Err(e) => json_err(&e.to_string()),
    })
    .unwrap_or_else(|e| json_err(&e))
}

/// Remove a flag by name. Returns `{"ok":true}` or `{"error":"..."}`.
#[wasm_bindgen]
pub fn remove_flag(handle: u32, name: &str) -> String {
    with_builder_mut(handle, |b| match b.remove_flag(name) {
        Ok(()) => json_ok(),
        Err(e) => json_err(&e.to_string()),
    })
    .unwrap_or_else(|e| json_err(&e))
}

/// Remove a channel label. Returns `{"ok":true}` or `{"error":"..."}`.
#[wasm_bindgen]
pub fn remove_label(handle: u32, instance: &str, port: &str, index: u32) -> String {
    with_builder_mut(handle, |b| match b.remove_label(instance, port, index) {
        Ok(()) => json_ok(),
        Err(e) => json_err(&e.to_string()),
    })
    .unwrap_or_else(|e| json_err(&e))
}

/// Remove an entire config block for an instance. Returns `{"ok":true}` or `{"error":"..."}`.
#[wasm_bindgen]
pub fn remove_config(handle: u32, instance: &str) -> String {
    with_builder_mut(handle, |b| match b.remove_config(instance) {
        Ok(()) => json_ok(),
        Err(e) => json_err(&e.to_string()),
    })
    .unwrap_or_else(|e| json_err(&e))
}

/// Remove a bridge matching the given source and target port refs.
/// Returns `{"ok":true}` or `{"error":"..."}`.
#[wasm_bindgen]
pub fn remove_bridge(handle: u32, source_json: &str, target_json: &str) -> String {
    let source: patchlang::ast::PortRef = match serde_json::from_str(source_json) {
        Ok(d) => d,
        Err(e) => return json_err(&e.to_string()),
    };
    let target: patchlang::ast::PortRef = match serde_json::from_str(target_json) {
        Ok(d) => d,
        Err(e) => return json_err(&e.to_string()),
    };
    with_builder_mut(handle, |b| match b.remove_bridge(&source, &target) {
        Ok(()) => json_ok(),
        Err(e) => json_err(&e.to_string()),
    })
    .unwrap_or_else(|e| json_err(&e))
}

/// Remove a bridge group matching the given target port ref.
/// Returns `{"ok":true}` or `{"error":"..."}`.
#[wasm_bindgen]
pub fn remove_bridge_group(handle: u32, target_json: &str) -> String {
    let target: patchlang::ast::PortRef = match serde_json::from_str(target_json) {
        Ok(d) => d,
        Err(e) => return json_err(&e.to_string()),
    };
    with_builder_mut(handle, |b| match b.remove_bridge_group(&target) {
        Ok(()) => json_ok(),
        Err(e) => json_err(&e.to_string()),
    })
    .unwrap_or_else(|e| json_err(&e))
}

/// Remove a bus from an instance. Returns `{"ok":true}` or `{"error":"..."}`.
#[wasm_bindgen]
pub fn remove_bus(handle: u32, instance: &str, bus_name: &str) -> String {
    with_builder_mut(handle, |b| match b.remove_bus(instance, bus_name) {
        Ok(()) => json_ok(),
        Err(e) => json_err(&e.to_string()),
    })
    .unwrap_or_else(|e| json_err(&e))
}

/// Remove a signal by name. Returns `{"ok":true}` or `{"error":"..."}`.
#[wasm_bindgen]
pub fn remove_signal(handle: u32, name: &str) -> String {
    with_builder_mut(handle, |b| match b.remove_signal(name) {
        Ok(()) => json_ok(),
        Err(e) => json_err(&e.to_string()),
    })
    .unwrap_or_else(|e| json_err(&e))
}

/// Remove a stream by name. Returns `{"ok":true}` or `{"error":"..."}`.
#[wasm_bindgen]
pub fn remove_stream(handle: u32, name: &str) -> String {
    with_builder_mut(handle, |b| match b.remove_stream(name) {
        Ok(()) => json_ok(),
        Err(e) => json_err(&e.to_string()),
    })
    .unwrap_or_else(|e| json_err(&e))
}

/// Remove a ring by name. Returns `{"ok":true}` or `{"error":"..."}`.
#[wasm_bindgen]
pub fn remove_ring(handle: u32, name: &str) -> String {
    with_builder_mut(handle, |b| match b.remove_ring(name) {
        Ok(()) => json_ok(),
        Err(e) => json_err(&e.to_string()),
    })
    .unwrap_or_else(|e| json_err(&e))
}

/// Remove a member from a ring. Returns `{"ok":true}` or `{"error":"..."}`.
#[wasm_bindgen]
pub fn remove_ring_member(handle: u32, ring_name: &str, instance: &str) -> String {
    with_builder_mut(handle, |b| match b.remove_ring_member(ring_name, instance) {
        Ok(()) => json_ok(),
        Err(e) => json_err(&e.to_string()),
    })
    .unwrap_or_else(|e| json_err(&e))
}

// ---------------------------------------------------------------------------
// Original exports (unchanged)
// ---------------------------------------------------------------------------

/// Parse PatchLang source and return JSON result.
/// Returns JSON with { program, errors } in TypeScript-compatible shape.
/// The program field matches the `PatchProgram` type from the frontend.
#[wasm_bindgen]
pub fn parse(source: &str) -> String {
    let result = patchlang::parse(source);
    let ts_result = patchlang::to_ts_result(&result);
    serde_json::to_string(&ts_result).unwrap_or_else(|e| {
        format!(r#"{{"error":"serialization failed: {e}"}}"#)
    })
}

/// Validate PatchLang source. Returns true if no errors.
#[wasm_bindgen]
pub fn validate(source: &str) -> bool {
    patchlang::parse(source).is_valid()
}

/// Generate a deterministic port ID.
///
/// `index` uses `i32` because `wasm_bindgen` does not support `Option<u32>`.
/// Pass [`NO_INDEX`] (`-1`) for single (non-ranged) ports.
/// Pass a non-negative value as the channel index for ranged ports.
#[wasm_bindgen]
pub fn generate_port_id(
    instance_name: &str,
    template_name: &str,
    port_name: &str,
    index: i32,
) -> String {
    let idx = if index < 0 { None } else { Some(index as u32) };
    patchlang::generate_port_id(instance_name, template_name, port_name, idx)
}

/// Generate a deterministic route ID.
#[wasm_bindgen]
pub fn generate_route_id(
    template_name: &str,
    source_port: &str,
    target_port: &str,
) -> String {
    patchlang::generate_route_id(template_name, source_port, target_port)
}

/// Generate a deterministic slot ID.
#[wasm_bindgen]
pub fn generate_slot_id(
    template_name: &str,
    slot_name: &str,
) -> String {
    patchlang::generate_slot_id(template_name, slot_name)
}

/// Format PatchLang source into canonical style.
/// Returns the formatted source, or a JSON error object if parsing fails.
#[wasm_bindgen]
pub fn format_source(source: &str) -> String {
    match patchlang::format_source(source) {
        Ok(formatted) => formatted,
        Err(e) => format!(r#"{{"error":"{e}"}}"#),
    }
}

/// Parse PatchLang source and run DRC checks.
/// Returns JSON with { program, errors, diagnostics } in TypeScript-compatible shape.
#[wasm_bindgen]
pub fn check(source: &str) -> String {
    let result = patchlang::check(source);
    serde_json::to_string(&result).unwrap_or_else(|e| {
        format!(r#"{{"error":"serialization failed: {e}"}}"#)
    })
}

/// Quick-parse source and return JSON array of namespace strings from `use` statements.
/// Useful for discovering file dependencies before full compilation.
#[wasm_bindgen]
pub fn resolve_uses(source: &str) -> String {
    let deps = patchlang::resolve_uses(source);
    serde_json::to_string(&deps).unwrap_or_else(|e| {
        format!(r#"{{"error":"serialization failed: {e}"}}"#)
    })
}

/// Multi-file compilation with namespace resolution and merged DRC.
///
/// `files_json` is a JSON object mapping file paths to source strings.
/// `entry` is the path of the entry file.
/// Returns JSON with { program, errors, diagnostics }.
#[wasm_bindgen]
pub fn compile_project(files_json: &str, entry: &str) -> String {
    let files: std::collections::HashMap<String, String> = match serde_json::from_str(files_json) {
        Ok(f) => f,
        Err(e) => return format!(r#"{{"error":"invalid files JSON: {e}"}}"#),
    };
    let result = patchlang::compile_project(files, entry);
    serde_json::to_string(&result).unwrap_or_else(|e| {
        format!(r#"{{"error":"serialization failed: {e}"}}"#)
    })
}

/// Parse and validate a project.json manifest string.
/// Returns JSON: `{ "manifest": {...} | null, "errors": [...] }`.
#[wasm_bindgen]
pub fn parse_manifest(json: &str) -> String {
    let result = patchlang::parse_manifest(json);
    serde_json::to_string(&result).unwrap_or_else(|e| {
        format!(r#"{{"error":"serialization failed: {e}"}}"#)
    })
}

/// Validate a `.layout.json` string against the schema.
/// Returns JSON: `{ "valid": bool, "errors": [...] }`.
#[wasm_bindgen]
pub fn validate_layout(json: &str) -> String {
    patchlang::validate_layout(json)
}

/// Cross-validate instance names between a `.patch` source and a `.layout.json`.
/// Returns JSON: `{ "valid": bool, "errors": [...], "warnings": [...] }`.
#[wasm_bindgen]
pub fn validate_project_consistency(patch: &str, layout: &str) -> String {
    patchlang::validate_project_consistency(patch, layout)
}

/// Compile PatchLang source to a hierarchical graph (JSON).
/// Returns a `CompileToGraphResult` with levels, signals, and streams.
#[wasm_bindgen]
pub fn compile_to_graph(source: &str) -> String {
    patchlang::compile_to_graph(source)
}

/// Multi-file compilation to graph.
/// `files_json` is a JSON object mapping file paths to source strings.
/// `entry` is the path of the entry file.
/// Returns a `CompileToGraphResult` JSON string.
#[wasm_bindgen]
pub fn compile_project_to_graph(files_json: &str, entry: &str) -> String {
    let files: std::collections::HashMap<String, String> = match serde_json::from_str(files_json) {
        Ok(f) => f,
        Err(e) => return format!(r#"{{"error":"invalid files JSON: {e}"}}"#),
    };
    patchlang::compile_project_to_graph_from_sources(&files, entry)
}

/// Contract a flat PatchLang source into a hierarchical one ("russian doll maker").
/// `assignments_json` is a JSON object mapping instance name -> cluster id. Returns canonical
/// hierarchical PatchLang text on success, or `{"error":"..."}` on failure.
#[wasm_bindgen]
pub fn contract_to_hierarchy(source: &str, assignments_json: &str) -> String {
    let assignments: std::collections::BTreeMap<String, String> =
        match serde_json::from_str(assignments_json) {
            Ok(a) => a,
            Err(e) => return format!(r#"{{"error":"invalid assignments JSON: {e}"}}"#),
        };
    let parsed = patchlang::parse(source);
    if !parsed.errors.is_empty() {
        return r#"{"error":"source has parse errors"}"#.to_string();
    }
    let hierarchical = patchlang::contract::contract_to_hierarchy(&parsed.program, &assignments);
    patchlang::formatter::format_program(&hierarchical)
}

/// Emit PatchLang source from a canvas state bundle (JSON).
/// `input_json` must deserialize to `CanvasEmitInput`.
/// Returns canonical PatchLang text on success, `{"error":"..."}` on failure.
#[wasm_bindgen]
pub fn emit_from_canvas_input(input_json: &str) -> String {
    let input: patchlang::builder::canvas_input::CanvasEmitInput =
        match serde_json::from_str(input_json) {
            Ok(v) => v,
            Err(e) => return json_err(&format!("JSON parse error: {e}")),
        };
    match patchlang::builder::emit_from_canvas_input(input) {
        Ok(patch) => patch,
        Err(e) => json_err(&e.to_string()),
    }
}

/// Parse PatchLang source text and return `CanvasLoadOutput` as JSON.
/// Returns `{"error":"..."}` on parse/logic failure.
#[wasm_bindgen]
pub fn load_from_patch(patch_source: &str, layout_json: &str) -> String {
    match patchlang::builder::load_from_patch(patch_source, layout_json) {
        Ok(output) => serde_json::to_string(&output).unwrap_or_else(|e| json_err(&e.to_string())),
        Err(e) => json_err(&e.to_string()),
    }
}

/// Import an EasySchematic JSON save file and return `{ patch, layout, warnings }` as JSON,
/// or `{"error":"..."}` on failure.
///
/// - `patch` — canonical PatchLang source text
/// - `layout` — `.layout.json` sidecar (version 2 schema)
/// - `warnings` — non-fatal issues (e.g. skipped direction-violating connections)
#[wasm_bindgen]
pub fn import_from_easyschematic(json: &str) -> String {
    match patchlang::import::import_easyschematic(json) {
        Ok(result) => {
            serde_json::to_string(&result).unwrap_or_else(|e| json_err(&e.to_string()))
        }
        Err(e) => json_err(&e.to_string()),
    }
}
