//! Statement emitters for the PatchLang formatter.
//!
//! Split from `formatter.rs` so neither file grows unbounded, then split again once the
//! emitters outgrew a single file:
//!
//! - this module root — the shared primitives every emitter builds on (quoting, port
//!   refs, index specs, key/value bodies, the indentation unit)
//! - [`decls`] — one emitter per top-level `Statement` variant
//! - [`bodies`] — the pieces nested inside a template or instance body
//!
//! A new emitter belongs in `decls` or `bodies`; only something shared by both belongs
//! here.
//!
//! Every quoted string MUST be written through `emit_quoted`. Bare concatenation is
//! how #35 happened: a value containing `"` produced text that still parsed but
//! carried a different value, silently. See D026.

use crate::ast::*;

mod bodies;
mod decls;

// Re-exported so `formatter.rs` keeps calling `formatter_emit::emit_*` unchanged.
pub(crate) use decls::{
    emit_bridge, emit_bridge_group, emit_config, emit_connect, emit_flag, emit_instance,
    emit_link_group, emit_network, emit_ring, emit_signal, emit_stream, emit_template, emit_use,
};

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
/// Every quoted emission in this module tree must go through here.
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
