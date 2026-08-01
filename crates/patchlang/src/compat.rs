//! Conversion from internal Rust AST to TypeScript-compatible compat types.
//!
//! The internal AST (`ast.rs`) uses Rust conventions (snake_case, enums, Vec).
//! The frontend expects camelCase JSON with flat records and parsed enums.
//! This module bridges the gap without modifying the internal AST.

use std::collections::{BTreeMap, HashMap};

use crate::ast::{self, AutoResolutions, PortSide};
use crate::compat_types::*;
use crate::error::ParseResult;

/// Lookup key for auto-resolutions: (span_start, span_end, side).
/// Each `[auto]` is uniquely identified by the connection span + which side.
type AutoLookupKey = (usize, usize, PortSide);

/// Pre-built lookup table from `AutoResolutions`.
type AutoLookup = HashMap<AutoLookupKey, ast::IndexSpec>;

// ── Top-level entry point ───────────────────────────────────────────

/// Convert a `ParseResult` (internal AST + errors) into TS-compatible JSON shape.
///
/// When `resolutions` is provided, `[auto]` index specs in connections are
/// replaced with the concrete resolved indices from the side table.
pub fn to_ts_result(result: &ParseResult) -> TsParseResult {
    to_ts_result_with_resolutions(result, &AutoResolutions::default())
}

/// Like [`to_ts_result`] but merges auto-resolutions into the output.
pub fn to_ts_result_with_resolutions(
    result: &ParseResult,
    resolutions: &AutoResolutions,
) -> TsParseResult {
    let lookup = build_auto_lookup(resolutions);
    TsParseResult {
        program: to_ts_program_with_lookup(&result.program, &lookup),
        errors: result
            .errors
            .iter()
            .map(|e| TsParseError {
                message: e.message.clone(),
                span: TsSpan {
                    start: e.span.start,
                    end: e.span.end,
                },
                hint: e.hint.clone(),
                file: None,
            })
            .collect(),
    }
}

/// Build lookup table from auto-resolutions for O(1) access during conversion.
fn build_auto_lookup(resolutions: &AutoResolutions) -> AutoLookup {
    resolutions
        .resolutions
        .iter()
        .map(|r| {
            let key = (r.span.start, r.span.end, r.side.clone());
            (key, r.resolved.clone())
        })
        .collect()
}

/// Convert the internal `PatchProgram` to the TS-compatible shape.
pub fn to_ts_program(program: &ast::PatchProgram) -> TsProgram {
    to_ts_program_with_lookup(program, &HashMap::new())
}

/// Convert with auto-resolution lookup.
fn to_ts_program_with_lookup(program: &ast::PatchProgram, lookup: &AutoLookup) -> TsProgram {
    TsProgram {
        r#type: "Program",
        statements: program
            .statements
            .iter()
            .filter_map(|s| convert_statement(s, lookup))
            .collect(),
    }
}

// ── Statement dispatch ──────────────────────────────────────────────

fn convert_statement(stmt: &ast::Statement, lookup: &AutoLookup) -> Option<TsStatement> {
    match stmt {
        ast::Statement::Template(t) => Some(TsStatement::Template(convert_template(t, lookup))),
        ast::Statement::Instance(i) => Some(TsStatement::Instance(convert_instance(i))),
        ast::Statement::Connect(c) => Some(TsStatement::Connect(convert_connect(c, lookup))),
        ast::Statement::Bridge(b) => Some(TsStatement::Bridge(convert_bridge(b))),
        ast::Statement::BridgeGroup(bg) => {
            Some(TsStatement::BridgeGroup(convert_bridge_group(bg)))
        }
        ast::Statement::LinkGroup(lg) => {
            Some(TsStatement::LinkGroup(convert_link_group(lg, lookup)))
        }
        ast::Statement::Signal(s) => Some(TsStatement::Signal(convert_signal(s))),
        ast::Statement::Flag(f) => Some(TsStatement::Flag(convert_flag(f))),
        ast::Statement::Stream(s) => Some(TsStatement::Stream(convert_stream(s))),
        ast::Statement::Config(c) => Some(TsStatement::Config(convert_config(c))),
        ast::Statement::Use(u) => Some(TsStatement::Use(convert_use(u))),
        ast::Statement::Ring(r) => Some(TsStatement::Ring(convert_ring(r))),
        ast::Statement::Network(n) => Some(TsStatement::Network(convert_network(n))),
        ast::Statement::Error(_) => None, // Error nodes are dropped in TS output
    }
}

// ── Individual type converters ──────────────────────────────────────

fn convert_template(t: &ast::TemplateDecl, lookup: &AutoLookup) -> TsTemplateDecl {
    TsTemplateDecl {
        type_tag: "Template",
        name: t.name.clone(),
        params: t.params.iter().map(convert_param_def).collect(),
        meta: kv_to_string_record(&t.meta),
        ports: t.ports.iter().map(convert_port_def).collect(),
        bridges: t.bridges.iter().map(convert_bridge).collect(),
        instances: t.instances.iter().map(convert_instance).collect(),
        connects: t.connects.iter().map(|c| convert_connect(c, lookup)).collect(),
        slots: t.slots.iter().map(convert_slot_def).collect(),
        version: t.version.clone(),
    }
}

fn convert_param_def(p: &ast::ParamDef) -> TsParamDef {
    TsParamDef {
        name: p.name.clone(),
        default_value: match &p.default_value {
            ast::ParamValue::Num { value } => TsParamValue::Num(*value),
            ast::ParamValue::Str { value } => TsParamValue::Str(value.clone()),
        },
    }
}

fn convert_port_def(p: &ast::PortDef) -> TsPortDef {
    let named_attrs = kv_to_string_record(&p.named_attributes);
    // Chevrotain visitor puts named attribute VALUES into the flat attributes array too
    let mut attributes = p.attributes.clone();
    for kv in &p.named_attributes {
        if let ast::KvValue::Str { value } = &kv.value {
            attributes.push(value.clone());
        }
    }
    TsPortDef {
        name: p.name.clone(),
        range_start: p.range.as_ref().map(|r| r.start),
        range_end: p.range.as_ref().map(|r| r.end),
        direction: convert_direction(&p.direction),
        connector: p.connector.clone(),
        attributes,
        named_attributes: if named_attrs.is_empty() {
            None
        } else {
            Some(named_attrs)
        },
    }
}

fn convert_direction(d: &ast::PortDirection) -> String {
    match d {
        ast::PortDirection::In => "in".to_string(),
        ast::PortDirection::Out => "out".to_string(),
        ast::PortDirection::Io => "io".to_string(),
    }
}

fn convert_instance(i: &ast::InstanceDecl) -> TsInstanceDecl {
    let routes: Vec<TsRouteDecl> = i.routes.iter().map(convert_route_entry).collect();
    let buses: Vec<TsBusDecl> = i.buses.iter().map(convert_bus_entry).collect();
    let slots: Vec<TsSlotAssign> = i.slot_assignments.iter().map(convert_slot_assign).collect();

    TsInstanceDecl {
        type_tag: "Instance",
        name: i.name.clone(),
        template_name: i.template_name.clone(),
        args: kv_to_arg_record(&i.args),
        properties: kv_to_string_record(&i.properties),
        version_constraint: i.version_constraint.clone(),
        routes: if routes.is_empty() { None } else { Some(routes) },
        buses: if buses.is_empty() { None } else { Some(buses) },
        typed_slot_assignments: if slots.is_empty() { None } else { Some(slots) },
    }
}

fn convert_connect(c: &ast::ConnectDecl, lookup: &AutoLookup) -> TsConnectDecl {
    let src_key = (c.span.start, c.span.end, PortSide::Source);
    let tgt_key = (c.span.start, c.span.end, PortSide::Target);

    let source = match lookup.get(&src_key) {
        Some(resolved) => convert_port_ref_with_resolved(&c.source, resolved),
        None => convert_port_ref(&c.source),
    };
    let target = match lookup.get(&tgt_key) {
        Some(resolved) => convert_port_ref_with_resolved(&c.target, resolved),
        None => convert_port_ref(&c.target),
    };

    TsConnectDecl {
        type_tag: "Connect",
        source,
        target,
        properties: kv_to_string_record(&c.properties),
        suppressions: if c.suppressions.is_empty() {
            None
        } else {
            Some(TsSuppression {
                layers: c.suppressions.clone(),
            })
        },
        mapping: c.mapping.as_ref().and_then(|raw| parse_mapping_spec(raw)),
    }
}

fn convert_bridge(b: &ast::BridgeDecl) -> TsBridgeDecl {
    TsBridgeDecl {
        type_tag: "Bridge",
        source: convert_port_ref(&b.source),
        target: convert_port_ref(&b.target),
    }
}

fn convert_bridge_group(bg: &ast::BridgeGroupDecl) -> TsBridgeGroupDecl {
    TsBridgeGroupDecl {
        type_tag: "BridgeGroup",
        target: convert_port_ref(&bg.target),
        sources: bg.sources.iter().map(convert_port_ref).collect(),
    }
}

fn convert_link_group(lg: &ast::LinkGroupDecl, lookup: &AutoLookup) -> TsLinkGroupDecl {
    TsLinkGroupDecl {
        type_tag: "LinkGroup",
        name: lg.name.clone(),
        connects: lg.connects.iter().map(|c| convert_connect(c, lookup)).collect(),
        properties: kv_to_string_record(&lg.properties),
    }
}

fn convert_signal(s: &ast::SignalDecl) -> TsSignalDecl {
    TsSignalDecl {
        type_tag: "Signal",
        name: s.name.clone(),
        properties: kv_to_string_record(&s.properties),
        origin: s.origin.as_ref().map(convert_port_ref),
    }
}

fn convert_flag(f: &ast::FlagDecl) -> TsFlagDecl {
    TsFlagDecl {
        type_tag: "Flag",
        name: f.name.clone(),
        properties: kv_to_string_record(&f.properties),
    }
}

fn convert_stream(s: &ast::StreamDecl) -> TsStreamDecl {
    TsStreamDecl {
        type_tag: "Stream",
        name: s.name.clone(),
        properties: kv_to_string_record(&s.properties),
        source: s.source.as_ref().map(convert_port_ref),
    }
}

fn convert_config(c: &ast::ConfigDecl) -> TsConfigDecl {
    TsConfigDecl {
        type_tag: "Config",
        name: c.name.clone(),
        labels: c.labels.iter().map(convert_config_label).collect(),
    }
}

fn convert_config_label(cl: &ast::ConfigLabel) -> TsConfigLabel {
    TsConfigLabel {
        port: convert_port_ref(&cl.port),
        label: cl.label.clone(),
        properties: kv_to_string_record(&cl.properties),
    }
}

fn convert_use(u: &ast::UseDecl) -> TsUseDecl {
    TsUseDecl {
        type_tag: "Use",
        namespace: u.namespace.clone(),
        templates: u.templates.clone(),
        wildcard: u.wildcard,
    }
}

pub(crate) fn convert_ring(r: &ast::RingDecl) -> TsRingDecl {
    TsRingDecl {
        type_tag: "Ring",
        name: r.name.clone(),
        properties: kv_to_string_record(&r.properties),
        members: r.members.iter().map(convert_ring_member).collect(),
    }
}

pub(crate) fn convert_ring_member(m: &ast::RingMember) -> TsRingMember {
    TsRingMember {
        instance_name: m.instance_name.clone(),
        port_name: m.port_name.clone(),
    }
}

pub(crate) fn convert_network(n: &ast::NetworkDecl) -> TsNetworkDecl {
    TsNetworkDecl {
        type_tag: "Network",
        name: n.name.clone(),
        properties: kv_to_string_record(&n.properties),
        members: n.members.iter().map(convert_network_member).collect(),
    }
}

pub(crate) fn convert_network_member(m: &ast::NetworkMember) -> TsNetworkMember {
    match m {
        ast::NetworkMember::DeviceLevel { instance, .. } => {
            TsNetworkMember::DeviceLevel { instance: instance.clone() }
        }
        ast::NetworkMember::PortGroup { instance, port_group, .. } => {
            TsNetworkMember::PortGroup {
                instance: instance.clone(),
                port_group: port_group.clone(),
            }
        }
        ast::NetworkMember::SlotRef { instance, index, .. } => {
            TsNetworkMember::SlotRef {
                instance: instance.clone(),
                index: *index,
            }
        }
    }
}

fn convert_slot_def(s: &ast::SlotDef) -> TsSlotDef {
    TsSlotDef {
        name: s.name.clone(),
        range_start: s.range.as_ref().map(|r| r.start),
        range_end: s.range.as_ref().map(|r| r.end),
        slot_type: s.slot_type.clone(),
        properties: kv_to_string_record(&s.properties),
    }
}

fn convert_route_entry(r: &ast::RouteEntry) -> TsRouteDecl {
    TsRouteDecl {
        from_port: r.source.port.clone(),
        from_index: convert_index_spec(&r.source.index),
        to_port: r.target.port.clone(),
        to_index: convert_index_spec(&r.target.index),
    }
}

fn convert_bus_entry(b: &ast::BusEntry) -> TsBusDecl {
    TsBusDecl {
        name: b.name.clone(),
        label: b.label.clone(),
        inputs: b.inputs.iter().map(convert_port_ref).collect(),
        outputs: b.outputs.iter().map(|o| TsBusOutput {
            label: o.label.clone(),
            destinations: o.destinations.iter().map(convert_port_ref).collect(),
        }).collect(),
        insert_send: b.insert_send.iter().map(convert_port_ref).collect(),
        insert_return: b.insert_return.iter().map(convert_port_ref).collect(),
    }
}

fn convert_slot_assign(sa: &ast::SlotAssignment) -> TsSlotAssign {
    TsSlotAssign {
        slot_name: sa.slot_name.clone(),
        slot_index: sa.index,
        card_type_name: sa.card_name.clone(),
    }
}

// ── PortRef / IndexSpec ─────────────────────────────────────────────

fn convert_port_ref(pr: &ast::PortRef) -> TsPortRef {
    TsPortRef {
        instance: pr.instance.clone().unwrap_or_default(),
        port: pr.port.clone(),
        index_spec: convert_index_spec(&pr.index),
    }
}

/// Convert a port ref, substituting the resolved index spec for `[auto]`.
fn convert_port_ref_with_resolved(pr: &ast::PortRef, resolved: &ast::IndexSpec) -> TsPortRef {
    TsPortRef {
        instance: pr.instance.clone().unwrap_or_default(),
        port: pr.port.clone(),
        index_spec: convert_index_spec(&Some(resolved.clone())),
    }
}

fn convert_index_spec(idx: &Option<ast::IndexSpec>) -> Option<Vec<TsIndexElement>> {
    idx.as_ref().map(|spec| {
        spec.elements
            .iter()
            .map(|el| match el {
                ast::IndexElement::Single { value } => TsIndexElement::Single { value: *value },
                ast::IndexElement::Range { start, end } => TsIndexElement::Range {
                    start: *start,
                    end: *end,
                },
                ast::IndexElement::Auto => {
                    // Safety fallback for non-connection contexts (routes, buses, etc.)
                    // where [auto] should have been rejected at parse time (A01).
                    TsIndexElement::Single { value: 0 }
                }
            })
            .collect()
    })
}

// ── KeyValue → Record converters ────────────────────────────────────

/// Convert `Vec<KeyValue>` to `Record<string, string>`.
/// Numeric values become their string representation.
/// PortRef values become "Instance.Port[index]" format.
fn kv_to_string_record(kvs: &[ast::KeyValue]) -> BTreeMap<String, String> {
    kvs.iter()
        .map(|kv| {
            let val = match &kv.value {
                ast::KvValue::Str { value } => value.clone(),
                ast::KvValue::Num { value } => value.to_string(),
                ast::KvValue::PortRef(pr) => stringify_port_ref(pr),
            };
            // Normalize deprecated device_type → kind (D011)
            let key = if kv.key == "device_type" { "kind".to_string() } else { kv.key.clone() };
            (key, val)
        })
        .collect()
}

/// Convert `Vec<KeyValue>` to `Record<string, number | string>`.
/// Numeric values stay as numbers.
fn kv_to_arg_record(kvs: &[ast::KeyValue]) -> BTreeMap<String, TsArgValue> {
    kvs.iter()
        .map(|kv| {
            let val = match &kv.value {
                ast::KvValue::Str { value } => TsArgValue::Str(value.clone()),
                ast::KvValue::Num { value } => TsArgValue::Num(*value),
                ast::KvValue::PortRef(pr) => TsArgValue::Str(stringify_port_ref(pr)),
            };
            // Normalize deprecated device_type → kind (D011)
            let key = if kv.key == "device_type" { "kind".to_string() } else { kv.key.clone() };
            (key, val)
        })
        .collect()
}

/// Stringify a PortRef as "Instance.Port[index]".
fn stringify_port_ref(pr: &ast::PortRef) -> String {
    let mut result = String::new();
    if let Some(inst) = &pr.instance {
        result.push_str(inst);
        result.push('.');
    }
    result.push_str(&pr.port);
    if let Some(idx) = &pr.index {
        result.push('[');
        for (i, el) in idx.elements.iter().enumerate() {
            if i > 0 {
                result.push(',');
            }
            match el {
                ast::IndexElement::Single { value } => {
                    result.push_str(&value.to_string());
                }
                ast::IndexElement::Range { start, end } => {
                    result.push_str(&format!("{start}..{end}"));
                }
                ast::IndexElement::Auto => {
                    result.push_str("auto");
                }
            }
        }
        result.push(']');
    }
    result
}

// ── Mapping spec parser ─────────────────────────────────────────────

/// Parse a raw mapping string into a structured `TsMappingSpec`.
///
/// Supported formats:
/// - `"1:1"` → `OneToOne`
/// - `"offset 16"` or `"offset -8"` → `Offset { offset: 16 }`
/// - `"1->3, 2->4"` → `Explicit { pairs: [...] }`
pub fn parse_mapping_spec(raw: &str) -> Option<TsMappingSpec> {
    let trimmed = raw.trim();

    if trimmed == "1:1" {
        return Some(TsMappingSpec::OneToOne);
    }

    if let Some(rest) = trimmed.strip_prefix("offset") {
        let rest = rest.trim();
        if let Ok(offset) = rest.parse::<i64>() {
            return Some(TsMappingSpec::Offset { offset });
        }
    }

    // Try explicit pair list: "1->3, 2->4, 3->1"
    let pairs: Vec<TsMappingPair> = trimmed
        .split(',')
        .filter_map(|seg| {
            let seg = seg.trim();
            let (from_str, to_str) = seg.split_once("->")?;
            let from = from_str.trim().parse::<u32>().ok()?;
            let to = to_str.trim().parse::<u32>().ok()?;
            Some(TsMappingPair { from, to })
        })
        .collect();

    if !pairs.is_empty() {
        return Some(TsMappingSpec::Explicit { pairs });
    }

    // Unrecognized mapping spec — return None so the caller can decide
    None
}

#[cfg(test)]
#[path = "compat_tests/mod.rs"]
mod tests;
