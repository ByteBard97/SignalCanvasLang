//! PatchLang source formatter — public API and statement dispatch.
//!
//! Parses PatchLang source, walks the AST, and emits a consistently
//! formatted version. Comments are NOT preserved (lexer discards them).
//!
//! Individual statement emitters live in the `formatter_emit` module tree.

use crate::ast::*;
use crate::formatter_emit;
use crate::parser::parse;

/// Format PatchLang source into canonical style.
/// Returns Err if the source has parse errors.
pub fn format_source(source: &str) -> Result<String, String> {
    let result = parse(source);
    if !result.is_valid() {
        return Err(format!("{} parse error(s)", result.errors.len()));
    }
    Ok(format_program(&result.program))
}

pub fn format_program(program: &PatchProgram) -> String {
    let mut out = String::new();
    for (i, stmt) in program.statements.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        emit_statement(&mut out, stmt, "");
    }
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn emit_statement(out: &mut String, stmt: &Statement, indent: &str) {
    match stmt {
        Statement::Template(t) => formatter_emit::emit_template(out, t, indent),
        Statement::Instance(inst) => formatter_emit::emit_instance(out, inst, indent),
        Statement::Connect(c) => formatter_emit::emit_connect(out, c, indent),
        Statement::Bridge(b) => formatter_emit::emit_bridge(out, b, indent),
        Statement::BridgeGroup(bg) => formatter_emit::emit_bridge_group(out, bg, indent),
        Statement::LinkGroup(lg) => formatter_emit::emit_link_group(out, lg, indent),
        Statement::Signal(s) => formatter_emit::emit_signal(out, s, indent),
        Statement::Flag(f) => formatter_emit::emit_flag(out, f, indent),
        Statement::Stream(s) => formatter_emit::emit_stream(out, s, indent),
        Statement::Config(c) => formatter_emit::emit_config(out, c, indent),
        Statement::Use(u) => formatter_emit::emit_use(out, u, indent),
        Statement::Ring(r) => formatter_emit::emit_ring(out, r, indent),
        Statement::Network(n) => formatter_emit::emit_network(out, n, indent),
        Statement::Error(_) => {} // skip error recovery nodes
    }
}
