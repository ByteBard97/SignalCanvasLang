//! Individual statement emitters for the PatchLang formatter.
//!
//! Split from `formatter.rs` so neither file grows unbounded. This one is now 594
//! lines — past the 500-line target though still inside the 700 hard limit — so the
//! next emitter added here should come with a further split rather than another
//! function.
//!
//! Every quoted string MUST be written through `emit_quoted`. Bare concatenation is
//! how #35 happened: a value containing `"` produced text that still parsed but
//! carried a different value, silently. See D026.

use crate::ast::*;

/// Two-space indentation unit (shared with formatter.rs).
pub(crate) const INDENT: &str = "  ";

/// Write `s` as a quoted PatchLang string literal, escaping it so the lexer reads
/// back exactly the same value (#35).
///
/// Encodes exactly the set the lexer decodes, by reading `lexer::ESCAPES` in reverse.
/// Backslash and quote are correctness — without them a value like `The "Big" Mix` is
/// silently truncated on re-parse. Newline, carriage return and tab are hygiene: they
/// survive unescaped, but emitting them raw makes a `.patch` file non-line-oriented.
///
/// Every quoted emission in this file must go through here.
fn emit_quoted(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match crate::lexer::ESCAPES.iter().find(|(_, decoded)| *decoded == c) {
            Some((source, _)) => {
                out.push('\\');
                out.push(*source);
            }
            None => out.push(c),
        }
    }
    out.push('"');
}

pub(crate) fn emit_template(out: &mut String, t: &TemplateDecl, indent: &str) {
    out.push_str(indent);
    out.push_str("template ");
    out.push_str(&t.name);
    emit_param_list(out, &t.params);
    if let Some(ver) = &t.version {
        out.push_str(" @version(");
        emit_quoted(out, ver);
        out.push(')');
    }
    out.push_str(" {\n");

    let inner = format!("{indent}{INDENT}");
    let inner2 = format!("{inner}{INDENT}");

    if !t.meta.is_empty() {
        out.push_str(&inner);
        out.push_str("meta {\n");
        for kv in &t.meta {
            emit_key_value(out, kv, &inner2);
        }
        out.push_str(&inner);
        out.push_str("}\n");
    }

    if !t.ports.is_empty() {
        out.push_str(&inner);
        out.push_str("ports {\n");
        for port in &t.ports {
            emit_port_def(out, port, &inner2);
        }
        out.push_str(&inner);
        out.push_str("}\n");
    }

    for b in &t.bridges {
        emit_bridge(out, b, &inner);
    }
    for inst in &t.instances {
        emit_instance(out, inst, &inner);
    }
    for c in &t.connects {
        emit_connect(out, c, &inner);
    }
    for s in &t.slots {
        emit_slot_def(out, s, &inner);
    }

    out.push_str(indent);
    out.push_str("}\n");
}

fn emit_param_list(out: &mut String, params: &[ParamDef]) {
    if params.is_empty() {
        return;
    }
    out.push('(');
    for (i, p) in params.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        out.push_str(&p.name);
        out.push_str(": ");
        emit_param_value(out, &p.default_value);
    }
    out.push(')');
}

fn emit_param_value(out: &mut String, val: &ParamValue) {
    match val {
        ParamValue::Str { value } => emit_quoted(out, value),
        ParamValue::Num { value } => out.push_str(&value.to_string()),
    }
}

fn emit_port_def(out: &mut String, port: &PortDef, indent: &str) {
    out.push_str(indent);
    out.push_str(&port.name);
    if let Some(range) = &port.range {
        out.push_str(&format!("[{}..{}]", range.start, range.end));
    }
    out.push_str(": ");
    out.push_str(match port.direction {
        PortDirection::In => "in",
        PortDirection::Out => "out",
        PortDirection::Io => "io",
    });
    if let Some(conn) = &port.connector {
        out.push('(');
        out.push_str(conn);
        out.push(')');
    }
    if !port.attributes.is_empty() || !port.named_attributes.is_empty() {
        out.push_str(" [");
        let mut first = true;
        for attr in &port.attributes {
            if !first {
                out.push_str(", ");
            }
            out.push_str(attr);
            first = false;
        }
        for kv in &port.named_attributes {
            if !first {
                out.push_str(", ");
            }
            out.push_str(&kv.key);
            out.push_str(": ");
            emit_kv_value_inline(out, &kv.value);
            first = false;
        }
        out.push(']');
    }
    out.push('\n');
}

pub(crate) fn emit_instance(out: &mut String, inst: &InstanceDecl, indent: &str) {
    out.push_str(indent);
    out.push_str("instance ");
    out.push_str(&inst.name);
    out.push_str(" is ");
    out.push_str(&inst.template_name);
    emit_arg_list(out, &inst.args);
    if let Some(ver) = &inst.version_constraint {
        out.push_str(" @version(");
        emit_quoted(out, ver);
        out.push(')');
    }

    let has_body = !inst.properties.is_empty()
        || !inst.routes.is_empty()
        || !inst.buses.is_empty()
        || !inst.slot_assignments.is_empty();

    if has_body {
        out.push_str(" {\n");
        let inner = format!("{indent}{INDENT}");
        for kv in &inst.properties {
            emit_key_value(out, kv, &inner);
        }
        for route in &inst.routes {
            emit_route_entry(out, route, &inner);
        }
        for bus in &inst.buses {
            emit_bus_entry(out, bus, &inner);
        }
        for sa in &inst.slot_assignments {
            emit_slot_assignment(out, sa, &inner);
        }
        out.push_str(indent);
        out.push_str("}\n");
    } else {
        out.push('\n');
    }
}

fn emit_arg_list(out: &mut String, args: &[KeyValue]) {
    if args.is_empty() {
        return;
    }
    out.push('(');
    for (i, kv) in args.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        out.push_str(&kv.key);
        out.push_str(": ");
        emit_kv_value_inline(out, &kv.value);
    }
    out.push(')');
}

pub(crate) fn emit_connect(out: &mut String, c: &ConnectDecl, indent: &str) {
    out.push_str(indent);
    out.push_str("connect ");
    emit_port_ref(out, &c.source);
    out.push_str(" -> ");
    emit_port_ref(out, &c.target);

    let has_body =
        !c.properties.is_empty() || !c.suppressions.is_empty() || c.mapping.is_some();

    if has_body {
        out.push_str(" {\n");
        let inner = format!("{indent}{INDENT}");
        if !c.suppressions.is_empty() {
            out.push_str(&inner);
            out.push_str("@suppress(");
            out.push_str(&c.suppressions.join(", "));
            out.push_str(")\n");
        }
        if let Some(mapping) = &c.mapping {
            out.push_str(&inner);
            out.push_str("mapping: ");
            emit_quoted(out, mapping);
            out.push('\n');
        }
        for kv in &c.properties {
            emit_key_value(out, kv, &inner);
        }
        out.push_str(indent);
        out.push_str("}\n");
    } else {
        out.push('\n');
    }
}

pub(crate) fn emit_bridge(out: &mut String, b: &BridgeDecl, indent: &str) {
    out.push_str(indent);
    out.push_str("bridge ");
    emit_port_ref(out, &b.source);
    out.push_str(" -> ");
    emit_port_ref(out, &b.target);
    out.push('\n');
}

pub(crate) fn emit_bridge_group(out: &mut String, bg: &BridgeGroupDecl, indent: &str) {
    out.push_str(indent);
    out.push_str("bridge_group ");
    emit_port_ref(out, &bg.target);
    out.push_str(" {\n");
    let inner = format!("{indent}{INDENT}");
    for src in &bg.sources {
        out.push_str(&inner);
        emit_port_ref(out, src);
        out.push('\n');
    }
    out.push_str(indent);
    out.push_str("}\n");
}

pub(crate) fn emit_link_group(out: &mut String, lg: &LinkGroupDecl, indent: &str) {
    out.push_str(indent);
    out.push_str("link_group ");
    out.push_str(&lg.name);
    out.push_str(" {\n");
    let inner = format!("{indent}{INDENT}");
    for kv in &lg.properties {
        emit_key_value(out, kv, &inner);
    }
    for c in &lg.connects {
        emit_connect(out, c, &inner);
    }
    out.push_str(indent);
    out.push_str("}\n");
}

pub(crate) fn emit_signal(out: &mut String, s: &SignalDecl, indent: &str) {
    out.push_str(indent);
    out.push_str("signal ");
    out.push_str(&s.name);
    emit_body_with_port_ref(out, &s.properties, s.origin.as_ref(), "origin", indent);
}

pub(crate) fn emit_flag(out: &mut String, f: &FlagDecl, indent: &str) {
    out.push_str(indent);
    out.push_str("flag ");
    out.push_str(&f.name);
    emit_kv_body(out, &f.properties, indent);
}

pub(crate) fn emit_stream(out: &mut String, s: &StreamDecl, indent: &str) {
    out.push_str(indent);
    out.push_str("stream ");
    out.push_str(&s.name);
    emit_body_with_port_ref(out, &s.properties, s.source.as_ref(), "source", indent);
}

fn emit_body_with_port_ref(
    out: &mut String,
    properties: &[KeyValue],
    port_ref: Option<&PortRef>,
    ref_key: &str,
    indent: &str,
) {
    let has_body = !properties.is_empty() || port_ref.is_some();
    if has_body {
        out.push_str(" {\n");
        let inner = format!("{indent}{INDENT}");
        if let Some(pr) = port_ref {
            out.push_str(&inner);
            out.push_str(ref_key);
            out.push_str(": ");
            emit_port_ref(out, pr);
            out.push('\n');
        }
        for kv in properties {
            emit_key_value(out, kv, &inner);
        }
        out.push_str(indent);
        out.push_str("}\n");
    } else {
        out.push('\n');
    }
}

fn emit_kv_body(out: &mut String, properties: &[KeyValue], indent: &str) {
    if properties.is_empty() {
        out.push('\n');
        return;
    }
    out.push_str(" {\n");
    let inner = format!("{indent}{INDENT}");
    for kv in properties {
        emit_key_value(out, kv, &inner);
    }
    out.push_str(indent);
    out.push_str("}\n");
}

pub(crate) fn emit_config(out: &mut String, c: &ConfigDecl, indent: &str) {
    out.push_str(indent);
    out.push_str("config ");
    out.push_str(&c.name);
    out.push_str(" {\n");
    let inner = format!("{indent}{INDENT}");
    for label in &c.labels {
        emit_config_label(out, label, &inner);
    }
    out.push_str(indent);
    out.push_str("}\n");
}

fn emit_config_label(out: &mut String, label: &ConfigLabel, indent: &str) {
    out.push_str(indent);
    out.push_str("label ");
    emit_port_ref(out, &label.port);
    out.push_str(": ");
    emit_quoted(out, &label.label);
    if !label.properties.is_empty() {
        out.push_str(" {\n");
        let inner = format!("{indent}{INDENT}");
        for kv in &label.properties {
            emit_key_value(out, kv, &inner);
        }
        out.push_str(indent);
        out.push('}');
    }
    out.push('\n');
}

pub(crate) fn emit_use(out: &mut String, u: &UseDecl, indent: &str) {
    out.push_str(indent);
    out.push_str("use ");
    out.push_str(&u.namespace);
    if u.wildcard {
        out.push_str(".*");
    } else if !u.templates.is_empty() {
        out.push_str(" { ");
        out.push_str(&u.templates.join(", "));
        out.push_str(" }");
    }
    out.push('\n');
}

pub(crate) fn emit_ring(out: &mut String, r: &RingDecl, indent: &str) {
    out.push_str(indent);
    out.push_str("ring ");
    out.push_str(&r.name);
    out.push_str(" {\n");
    let inner = format!("{indent}{INDENT}");
    for kv in &r.properties {
        emit_key_value(out, kv, &inner);
    }
    for member in &r.members {
        out.push_str(&inner);
        out.push_str("member ");
        out.push_str(&member.instance_name);
        if let Some(port) = &member.port_name {
            out.push('.');
            out.push_str(port);
        }
        out.push('\n');
    }
    out.push_str(indent);
    out.push_str("}\n");
}

pub(crate) fn emit_network(out: &mut String, n: &NetworkDecl, indent: &str) {
    out.push_str(indent);
    out.push_str("network ");
    out.push_str(&n.name);
    out.push_str(" {\n");
    let inner = format!("{indent}{INDENT}");
    for kv in &n.properties {
        emit_key_value(out, kv, &inner);
    }
    for member in &n.members {
        out.push_str(&inner);
        out.push_str("member ");
        match member {
            NetworkMember::DeviceLevel { instance, .. } => {
                out.push_str(instance);
            }
            NetworkMember::PortGroup { instance, port_group, .. } => {
                out.push_str(instance);
                out.push('.');
                out.push_str(port_group);
            }
            NetworkMember::SlotRef { instance, index, .. } => {
                out.push_str(instance);
                out.push_str(".slot[");
                out.push_str(&index.to_string());
                out.push(']');
            }
        }
        out.push('\n');
    }
    out.push_str(indent);
    out.push_str("}\n");
}

pub(crate) fn emit_slot_def(out: &mut String, s: &SlotDef, indent: &str) {
    out.push_str(indent);
    out.push_str("slot ");
    out.push_str(&s.name);
    if let Some(range) = &s.range {
        out.push_str(&format!("[{}..{}]", range.start, range.end));
    }
    out.push_str(": ");
    out.push_str(&s.slot_type);
    if !s.properties.is_empty() {
        out.push_str(" {\n");
        let inner = format!("{indent}{INDENT}");
        for kv in &s.properties {
            emit_key_value(out, kv, &inner);
        }
        out.push_str(indent);
        out.push('}');
    }
    out.push('\n');
}

fn emit_slot_assignment(out: &mut String, sa: &SlotAssignment, indent: &str) {
    out.push_str(indent);
    out.push_str("slot ");
    out.push_str(&sa.slot_name);
    if let Some(idx) = sa.index {
        out.push_str(&format!("[{idx}]"));
    }
    out.push_str(": ");
    if needs_quoting(&sa.card_name) {
        emit_quoted(out, &sa.card_name);
    } else {
        out.push_str(&sa.card_name);
    }
    out.push('\n');
}

fn emit_route_entry(out: &mut String, route: &RouteEntry, indent: &str) {
    out.push_str(indent);
    out.push_str("route ");
    emit_port_ref(out, &route.source);
    out.push_str(" -> ");
    emit_port_ref(out, &route.target);
    out.push('\n');
}

fn emit_bus_entry(out: &mut String, bus: &BusEntry, indent: &str) {
    out.push_str(indent);
    out.push_str("bus ");
    out.push_str(&bus.name);
    out.push_str(" {\n");
    let inner = format!("{indent}{INDENT}");
    if let Some(label) = &bus.label {
        out.push_str(&inner);
        out.push_str("label: ");
        emit_quoted(out, label);
        out.push('\n');
    }
    for input in &bus.inputs {
        out.push_str(&inner);
        out.push_str("input: ");
        emit_port_ref(out, input);
        out.push('\n');
    }
    // Insert legs after the inputs, before the outputs — send/return read as a detour
    // off the bus, not as another destination (#31).
    for (key, legs) in [("insert_send", &bus.insert_send), ("insert_return", &bus.insert_return)] {
        if legs.is_empty() {
            continue;
        }
        out.push_str(&inner);
        out.push_str(key);
        out.push_str(": ");
        for (i, leg) in legs.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            emit_port_ref(out, leg);
        }
        out.push('\n');
    }
    for output in &bus.outputs {
        out.push_str(&inner);
        out.push_str("output ");
        emit_quoted(out, &output.label);
        if !output.destinations.is_empty() {
            out.push_str(": ");
            for (i, dest) in output.destinations.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                emit_port_ref(out, dest);
            }
        }
        out.push('\n');
    }
    out.push_str(indent);
    out.push_str("}\n");
}

pub(crate) fn emit_port_ref(out: &mut String, pr: &PortRef) {
    if let Some(inst) = &pr.instance {
        out.push_str(inst);
        out.push('.');
    }
    out.push_str(&pr.port);
    if let Some(idx) = &pr.index {
        emit_index_spec(out, idx);
    }
}

fn emit_index_spec(out: &mut String, spec: &IndexSpec) {
    out.push('[');
    for (i, elem) in spec.elements.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        match elem {
            IndexElement::Single { value } => out.push_str(&value.to_string()),
            IndexElement::Range { start, end } => {
                out.push_str(&format!("{start}..{end}"));
            }
            IndexElement::Auto => {
                out.push_str("auto");
            }
        }
    }
    out.push(']');
}

pub(crate) fn emit_key_value(out: &mut String, kv: &KeyValue, indent: &str) {
    out.push_str(indent);
    out.push_str(&kv.key);
    out.push_str(": ");
    emit_kv_value_inline(out, &kv.value);
    out.push('\n');
}

fn emit_kv_value_inline(out: &mut String, value: &KvValue) {
    match value {
        KvValue::Str { value } => emit_quoted(out, value),
        KvValue::Num { value } => out.push_str(&value.to_string()),
        KvValue::PortRef(pr) => emit_port_ref(out, pr),
    }
}

/// Returns true if an identifier needs quoting (contains non-alphanumeric/underscore chars).
fn needs_quoting(s: &str) -> bool {
    s.is_empty() || !s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}
