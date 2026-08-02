//! Graph construction — compiles a PatchLang AST into a hierarchical graph
//! of devices, ports, and edges matching the TypeScript `CompileToGraphResult`.

pub mod types;

mod edges;
mod ports;
mod sublevel;

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::ast::{
    BridgeDecl, BridgeGroupDecl, ConfigDecl, ConnectDecl, InstanceDecl, KvValue, PatchProgram,
    SignalDecl, Statement, StreamDecl, TemplateDecl,
};
use crate::builder::LibraryContext;

use edges::{expand_bridge, expand_bridge_group_edges, expand_connect_edges};
use ports::expand_template_ports;
use sublevel::build_sub_level;
use types::*;

/// Parse PatchLang source and build the graph. Returns JSON string.
pub fn compile_to_graph(source: &str) -> String {
    let parse_result = crate::parser::parse(source);
    let result = compile_ast_to_graph(&parse_result.program, &LibraryContext::empty());
    serde_json::to_string(&result).unwrap_or_else(|e| {
        format!(r#"{{"error":"serialization failed: {e}"}}"#)
    })
}

/// Multi-file parse and graph compilation. Returns JSON string.
pub fn compile_project_to_graph(files: HashMap<String, String>, entry: &str) -> String {
    // Reuse the multi-file compiler to get a merged AST, then build graph
    let project_result = crate::multi_file::compile_project(files, entry);

    // Reconstruct a PatchProgram from the TS program by re-parsing from formatted source.
    // However, compile_project already merges statements — we need the internal AST.
    // Instead, we can re-parse the entry with all files merged.
    // The cleanest approach: parse each file and merge statements ourselves.
    // But compile_project already does this. We need the raw AST, not the TS compat one.
    //
    // Actually, we should just do our own multi-file merge here to get the raw AST.
    let merged = merge_files_to_ast(
        &project_result.files,
        &project_result.use_graph,
        entry,
    );

    // If the project had errors, we still try to build what we can
    let result = compile_ast_to_graph(&merged, &LibraryContext::empty());
    serde_json::to_string(&result).unwrap_or_else(|e| {
        format!(r#"{{"error":"serialization failed: {e}"}}"#)
    })
}

/// Re-merge files to get the raw Rust AST (not TS compat).
fn merge_files_to_ast(
    _files: &[String],
    _use_graph: &BTreeMap<String, Vec<String>>,
    _entry: &str,
) -> PatchProgram {
    // This is a simplified approach — the real multi-file merge is in compile_project.
    // For now, we'll accept that compile_project_to_graph requires re-parsing.
    // A better approach would be to refactor compile_project to expose the raw AST.
    PatchProgram {
        statements: Vec::new(),
    }
}

/// Multi-file graph compilation from raw file map (re-parses and merges).
pub fn compile_project_to_graph_from_sources(
    files: &HashMap<String, String>,
    entry: &str,
) -> String {
    use std::collections::VecDeque;

    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    let mut merged_stmts = Vec::new();

    queue.push_back(entry.to_string());
    visited.insert(entry.to_string());

    while let Some(file_path) = queue.pop_front() {
        let source = match files.get(&file_path) {
            Some(s) => s,
            None => continue,
        };
        let parse_result = crate::parser::parse(source);

        for stmt in parse_result.program.statements {
            if let Statement::Use(ref u) = stmt {
                let dep_path = format!("{}.patch", u.namespace.replace('.', "/"));
                if !visited.contains(&dep_path) {
                    visited.insert(dep_path.clone());
                    queue.push_back(dep_path);
                }
            }
            if !matches!(stmt, Statement::Use(_)) {
                merged_stmts.push(stmt);
            }
        }
    }

    let program = PatchProgram {
        statements: merged_stmts,
    };
    let result = compile_ast_to_graph(&program, &LibraryContext::empty());
    serde_json::to_string(&result).unwrap_or_else(|e| {
        format!(r#"{{"error":"serialization failed: {e}"}}"#)
    })
}

/// Compile from a builder's in-memory AST + library context.
/// No format/reparse round-trip.
pub fn compile_program_to_graph(
    program: &PatchProgram,
    library: &LibraryContext,
) -> CompileToGraphResult {
    compile_ast_to_graph(program, library)
}

/// Core graph compilation from a parsed AST.
pub fn compile_ast_to_graph(ast: &PatchProgram, library: &LibraryContext) -> CompileToGraphResult {
    // 1. Classify statements
    let mut templates: BTreeMap<String, TemplateDecl> = BTreeMap::new();
    let mut instances: Vec<&InstanceDecl> = Vec::new();
    let mut connects: Vec<&ConnectDecl> = Vec::new();
    let mut bridges: Vec<&BridgeDecl> = Vec::new();
    let mut bridge_groups: Vec<&BridgeGroupDecl> = Vec::new();
    let mut signal_decls: Vec<&SignalDecl> = Vec::new();
    let mut stream_decls: Vec<&StreamDecl> = Vec::new();
    let mut config_decls: Vec<&ConfigDecl> = Vec::new();

    for stmt in &ast.statements {
        match stmt {
            Statement::Template(t) => {
                templates.insert(t.name.clone(), t.clone());
            }
            Statement::Instance(i) => instances.push(i),
            Statement::Connect(c) => connects.push(c),
            Statement::Bridge(b) => bridges.push(b),
            Statement::BridgeGroup(bg) => bridge_groups.push(bg),
            Statement::Signal(s) => signal_decls.push(s),
            Statement::Stream(s) => stream_decls.push(s),
            Statement::Config(c) => config_decls.push(c),
            _ => {}
        }
    }

    // Merge library templates (program-local takes precedence)
    for (name, tmpl) in &library.templates {
        templates.entry(name.clone()).or_insert_with(|| tmpl.clone());
    }

    let mut root_nodes: BTreeMap<String, DeviceNode> = BTreeMap::new();
    let mut root_edges: BTreeMap<String, GraphEdge> = BTreeMap::new();
    let mut levels: BTreeMap<String, GraphLevel> = BTreeMap::new();
    let mut signals: BTreeMap<String, SignalIdentity> = BTreeMap::new();
    let mut streams: BTreeMap<String, StreamIdentity> = BTreeMap::new();

    // 2. Expand instances
    expand_instances(&instances, &templates, &mut root_nodes, &mut levels);

    // 3. Expand connect edges
    let connect_refs: Vec<ConnectDecl> = connects.iter().map(|c| (*c).clone()).collect();
    expand_connect_edges(&connect_refs, &mut root_edges);

    // 4. Expand root-level bridges
    for bridge in &bridges {
        let src_inst = bridge.source.instance.as_deref().unwrap_or("");
        let tgt_inst = bridge.target.instance.as_deref().unwrap_or("");
        let bridge_edges = expand_bridge(
            bridge,
            &format!("{src_inst}:"),
            &format!("{tgt_inst}:"),
            &format!("bridge_{src_inst}_{}", bridge.source.port),
            src_inst,
            tgt_inst,
        );
        root_edges.extend(bridge_edges);
    }

    // 5. Expand bridge groups
    let bg_refs: Vec<BridgeGroupDecl> = bridge_groups.iter().map(|bg| (*bg).clone()).collect();
    expand_bridge_group_edges(&bg_refs, &mut root_edges);

    // 6. Scalar port fallback
    apply_scalar_port_fallback(&root_nodes, &mut root_edges);

    // 6b. Drop over-range edges (channels beyond a declared port range) so no
    //     edge references a missing port shape. (User-facing warning: DRC.)
    drop_over_range_edges(&root_nodes, &mut root_edges);

    // 7. Mark port connectivity
    mark_port_connectivity(&mut root_nodes, &root_edges);

    // 8. Build signals
    for sig in &signal_decls {
        let label = sig
            .properties
            .iter()
            .find(|kv| kv.key == "label")
            .and_then(|kv| match &kv.value {
                KvValue::Str { value } => Some(value.clone()),
                _ => None,
            })
            .unwrap_or_else(|| sig.name.clone());

        let (origin_node, origin_port) = if let Some(ref origin) = sig.origin {
            (
                origin.instance.clone(),
                Some(resolve_port_id(origin)),
            )
        } else {
            (None, None)
        };

        signals.insert(
            sig.name.clone(),
            SignalIdentity {
                name: sig.name.clone(),
                label,
                origin_node,
                origin_port,
            },
        );
    }

    // 9. Build streams
    for stream in &stream_decls {
        let props = kv_to_string_map(&stream.properties);
        let (source_node, source_port) = if let Some(ref src) = stream.source {
            (src.instance.clone(), Some(resolve_stream_source_port_id(src)))
        } else {
            (None, None)
        };

        streams.insert(
            stream.name.clone(),
            StreamIdentity {
                name: stream.name.clone(),
                properties: props,
                source_node,
                source_port,
            },
        );
    }

    // 10. Apply config labels
    for config in &config_decls {
        for lbl in &config.labels {
            let inst_name = lbl.port.instance.as_deref().unwrap_or("");
            if let Some(node) = root_nodes.get_mut(inst_name) {
                let port_id = resolve_port_id(&lbl.port);
                if let Some(port) = node.ports.iter_mut().find(|p| p.id == port_id) {
                    port.label = Some(lbl.label.clone());
                    let lbl_props = kv_to_string_map(&lbl.properties);
                    if !lbl_props.is_empty() {
                        port.label_properties = Some(lbl_props);
                    }
                }
            }
        }
    }

    // Build root level
    levels.insert(
        "root".to_string(),
        GraphLevel {
            id: "root".to_string(),
            parent_id: None,
            label: "Root".to_string(),
            nodes: root_nodes,
            edges: root_edges,
        },
    );

    CompileToGraphResult {
        levels,
        signals,
        streams,
    }
}

/// Expand instances into root nodes, building sub-levels for drillable ones.
fn expand_instances(
    instances: &[&InstanceDecl],
    templates: &BTreeMap<String, TemplateDecl>,
    root_nodes: &mut BTreeMap<String, DeviceNode>,
    levels: &mut BTreeMap<String, GraphLevel>,
) {
    for inst in instances {
        let tmpl = templates.get(&inst.template_name);
        let ports = if let Some(t) = tmpl {
            expand_template_ports(inst, t, templates)
        } else {
            Vec::new()
        };

        let drillable = tmpl
            .map(|t| !t.bridges.is_empty() || !t.instances.is_empty())
            .unwrap_or(false);

        let properties = kv_to_string_map(&inst.properties);

        root_nodes.insert(
            inst.name.clone(),
            DeviceNode {
                id: inst.name.clone(),
                label: inst.name.clone(),
                template_name: inst.template_name.clone(),
                ports,
                properties,
                drillable,
            },
        );

        if drillable {
            if let Some(t) = tmpl {
                build_sub_level(inst, t, templates, levels, "root", &[]);
            }
        }
    }
}

/// After all edges are built, check if any edge references a port that doesn't
/// exist. Tries these fallbacks in order:
/// 1. Append `_1` (scalar ref to ranged port: `MADI` → `MADI_1`)
/// 2. Append `_In_1` or `_Out_1` (abbreviated name: `MADI` → `MADI_In_1` or `MADI_Out_1`)
/// 3. Prefix match against all ports on the node (finds first port starting with the ref)
pub(crate) fn apply_scalar_port_fallback(
    nodes: &BTreeMap<String, DeviceNode>,
    edges: &mut BTreeMap<String, GraphEdge>,
) {
    let all_port_ids: HashSet<String> = nodes
        .values()
        .flat_map(|n| n.ports.iter().map(|p| p.id.clone()))
        .collect();

    for edge in edges.values_mut() {
        if !all_port_ids.contains(&edge.source_port) {
            if let Some(resolved) = resolve_port_fallback(&edge.source_port, &edge.source_node, nodes, &all_port_ids) {
                edge.source_port = resolved;
            }
        }
        if !all_port_ids.contains(&edge.target_port) {
            if let Some(resolved) = resolve_port_fallback(&edge.target_port, &edge.target_node, nodes, &all_port_ids) {
                edge.target_port = resolved;
            }
        }
    }
}

/// Try to resolve a broken port reference using fallback strategies.
fn resolve_port_fallback(
    port_id: &str,
    node_id: &str,
    nodes: &BTreeMap<String, DeviceNode>,
    all_port_ids: &HashSet<String>,
) -> Option<String> {
    // Strategy 1: append _1 (scalar ref to ranged port)
    let with_1 = format!("{port_id}_1");
    if all_port_ids.contains(&with_1) {
        return Some(with_1);
    }

    // Strategy 2: append _In_1 or _Out_1 (abbreviated interface name)
    let with_in = format!("{port_id}_In_1");
    if all_port_ids.contains(&with_in) {
        return Some(with_in);
    }
    let with_out = format!("{port_id}_Out_1");
    if all_port_ids.contains(&with_out) {
        return Some(with_out);
    }

    // Strategy 3: prefix match on the node's ports
    // e.g., "Monitors/AI8_Unit_1:MADI" matches "Monitors/AI8_Unit_1:MADI_Out_1"
    if let Some(node) = nodes.get(node_id) {
        let prefix = format!("{port_id}_");
        if let Some(port) = node.ports.iter().find(|p| p.id.starts_with(&prefix)) {
            return Some(port.id.clone());
        }
    }

    None
}

/// Drop edges that reference a channel index beyond a node's declared port
/// range (e.g. `MADI_Out_49` where the node only declares `MADI_Out_1..48`).
///
/// Such over-range channels have no port shape, so the edge dangles and crashes
/// the strict drill (ELK JSON) import. The top-level layout silently tolerates
/// them, which is why the source defect (a connect referencing more channels
/// than the device physically has) only surfaces on drill-down.
///
/// Cause-keyed by design: ONLY indices beyond a *declared ranged port* are
/// dropped. A dangling reference with no ranged sibling — e.g. a `slot`-name
/// bridge endpoint (`Venue_Input_Slot`) — is left untouched, to be resolved by
/// port-shape synthesis instead. This keeps "fake channel → drop" and
/// "real bay → synthesize" distinct, per the drill-consistency invariant.
///
/// Returns the `source -> target` description of each dropped edge (for
/// diagnostics / tests). The user-facing warning is raised separately by the
/// DRC over-range check.
pub(crate) fn drop_over_range_edges(
    nodes: &BTreeMap<String, DeviceNode>,
    edges: &mut BTreeMap<String, GraphEdge>,
) -> Vec<String> {
    let existing: HashSet<&str> = nodes
        .values()
        .flat_map(|n| n.ports.iter().map(|p| p.id.as_str()))
        .collect();

    // True if `port_id` is an indexed reference whose index exceeds the highest
    // declared index for that base port on its node.
    let is_over_range = |port_id: &str| -> bool {
        if existing.contains(port_id) {
            return false;
        }
        let Some((node_id, port_name)) = port_id.split_once(':') else {
            return false;
        };
        let Some((base, idx_str)) = port_name.rsplit_once('_') else {
            return false;
        };
        let Ok(idx) = idx_str.parse::<u32>() else {
            return false;
        };
        let Some(node) = nodes.get(node_id) else {
            return false;
        };
        let prefix = format!("{base}_");
        let max_declared = node
            .ports
            .iter()
            .filter_map(|p| p.name.strip_prefix(&prefix).and_then(|s| s.parse::<u32>().ok()))
            .max();
        matches!(max_declared, Some(max) if idx > max)
    };

    let over_range_ids: Vec<String> = edges
        .iter()
        .filter(|(_, e)| is_over_range(&e.source_port) || is_over_range(&e.target_port))
        .map(|(id, _)| id.clone())
        .collect();

    let mut dropped = Vec::with_capacity(over_range_ids.len());
    for id in &over_range_ids {
        if let Some(edge) = edges.remove(id) {
            dropped.push(format!("{} -> {}", edge.source_port, edge.target_port));
        }
    }
    dropped
}

/// Mark ports as connected based on edge references.
fn mark_port_connectivity(
    nodes: &mut BTreeMap<String, DeviceNode>,
    edges: &BTreeMap<String, GraphEdge>,
) {
    let mut connected_ids = HashSet::new();
    for edge in edges.values() {
        connected_ids.insert(edge.source_port.clone());
        connected_ids.insert(edge.target_port.clone());
    }

    for node in nodes.values_mut() {
        for port in &mut node.ports {
            if connected_ids.contains(&port.id) {
                port.connected = Some(true);
            }
        }
    }
}

/// Resolve a `stream`'s source `PortRef` to a port ID string (#37).
///
/// A stream's channel selection describes the flow, not the port: the source of
/// `stream S { source: DSP.AES67_Out[3] }` is the `AES67_Out` interface, exactly as
/// it is for a stream with no index or with `[7, 1, 5, 3]`. Deliberately NOT routed
/// through `resolve_port_id`, and `resolve_port_id` must not be "simplified" to match
/// it: its other two callers — config labels and signal origins — depend on a
/// one-element index resolving to the channel port (`Inst:Port_3`), which is how a
/// label finds the channel it belongs to.
fn resolve_stream_source_port_id(port_ref: &crate::ast::PortRef) -> String {
    let inst = port_ref.instance.as_deref().unwrap_or("");
    format!("{inst}:{}", port_ref.port)
}

/// Resolve a `PortRef` to a port ID string.
///
/// A one-element index resolves to that channel's port (`Inst:Port_3`); anything
/// wider resolves to the bare port. See `resolve_stream_source_port_id` for why
/// streams do not use this.
fn resolve_port_id(port_ref: &crate::ast::PortRef) -> String {
    let inst = port_ref.instance.as_deref().unwrap_or("");
    if let Some(ref idx) = port_ref.index {
        let indices = edges::flatten_index_spec(idx);
        if indices.len() == 1 {
            return format!("{inst}:{}_{}", port_ref.port, indices[0]);
        }
    }
    format!("{inst}:{}", port_ref.port)
}

fn kv_to_string_map(kvs: &[crate::ast::KeyValue]) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for kv in kvs {
        let val = match &kv.value {
            KvValue::Str { value } => value.clone(),
            KvValue::Num { value } => value.to_string(),
            KvValue::PortRef(pr) => {
                let inst = pr.instance.as_deref().unwrap_or("");
                format!("{}.{}", inst, pr.port)
            }
        };
        map.insert(kv.key.clone(), val);
    }
    map
}
