//! Emitters for the pieces nested inside a template or instance body: parameter and
//! argument lists, port and slot definitions, slot assignments, route entries and bus
//! entries.
//!
//! Callers live in the sibling `decls` module; shared primitives live in the parent
//! `formatter_emit` module.

use crate::ast::*;

use super::{
    emit_key_value, emit_kv_value_inline, emit_port_ref, emit_quoted, needs_quoting, INDENT,
};

pub(super) fn emit_param_list(out: &mut String, params: &[ParamDef]) {
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

pub(super) fn emit_port_def(out: &mut String, port: &PortDef, indent: &str) {
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
            // A named attribute's value is a bare identifier, NOT a quoted string:
            // `attribute = identifier [ ":" identifier ]` (SPEC §port-def). The parser
            // reads an identifier here and stores it as `KvValue::Str`, so routing this
            // through `emit_kv_value_inline` quoted it and produced `[split: "direct"]`
            // — which our own parser then rejects. Emit the value bare.
            match &kv.value {
                KvValue::Str { value } => out.push_str(value),
                other => emit_kv_value_inline(out, other),
            }
            first = false;
        }
        out.push(']');
    }
    out.push('\n');
}

pub(super) fn emit_arg_list(out: &mut String, args: &[KeyValue]) {
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

pub(super) fn emit_slot_def(out: &mut String, s: &SlotDef, indent: &str) {
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

pub(super) fn emit_slot_assignment(out: &mut String, sa: &SlotAssignment, indent: &str) {
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

pub(super) fn emit_route_entry(out: &mut String, route: &RouteEntry, indent: &str) {
    out.push_str(indent);
    out.push_str("route ");
    emit_port_ref(out, &route.source);
    out.push_str(" -> ");
    emit_port_ref(out, &route.target);
    out.push('\n');
}

pub(super) fn emit_bus_entry(out: &mut String, bus: &BusEntry, indent: &str) {
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
