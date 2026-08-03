//! Top-level declaration emitters: the statements `formatter.rs` dispatches on.
//!
//! Each function here renders one `Statement` variant. Bodies nested inside a
//! declaration (ports, params, slots, routes, buses) live in the sibling `bodies`
//! module; shared primitives live in the parent `formatter_emit` module.

use crate::ast::*;

use super::bodies::{
    emit_arg_list, emit_bus_entry, emit_param_list, emit_port_def, emit_route_entry,
    emit_slot_assignment, emit_slot_def,
};
use super::{
    emit_body_with_port_ref, emit_key_value, emit_kv_body, emit_port_ref, emit_quoted, INDENT,
};

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
