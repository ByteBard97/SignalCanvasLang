pub mod ast;
pub mod builder;
pub mod compat;
pub mod compat_types;
pub mod contract;
pub mod drc;
pub mod error;
pub mod formatter;
pub(crate) mod formatter_emit;
pub mod graph;
pub mod ids;
pub mod layout_validator;
pub mod lexer;
pub mod manifest;
pub mod multi_file;
pub mod parser;
pub(crate) mod body_parser;
pub mod resolve_auto;
pub mod import;
pub(crate) mod template_parser;

#[cfg(test)]
mod formatter_tests;
#[cfg(test)]
mod template_parser_tests_core;
#[cfg(test)]
mod template_parser_tests_slots;
#[cfg(test)]
mod layout_validator_tests;
#[cfg(test)]
mod layout_cross_validation_tests;
#[cfg(test)]
mod manifest_tests;
#[cfg(test)]
mod multi_file_tests;
#[cfg(test)]
mod output_test_helpers;
#[cfg(test)]
mod output_tests_templates;
#[cfg(test)]
mod output_tests_connections;
#[cfg(test)]
mod output_tests_declarations;
#[cfg(test)]
mod output_tests_errors;
#[cfg(test)]
mod output_tests_drc;
#[cfg(test)]
mod output_tests_ids;
#[cfg(test)]
mod resolve_auto_tests;
#[cfg(test)]
mod output_tests_auto;
#[cfg(test)]
mod builder_tests;
#[cfg(test)]
mod graph_tests;
#[cfg(test)]
mod graph_drill_consistency_tests;
#[cfg(test)]
mod contract_tests;
#[cfg(test)]
mod string_escaping_tests;

pub use ast::PatchProgram;
pub use compat::{parse_mapping_spec, to_ts_program, to_ts_result, to_ts_result_with_resolutions};
pub use drc::{CheckResult, Diagnostic};
pub use error::{ParseError, Span};
pub use ids::{generate_port_id, generate_route_id, generate_slot_id};
pub use layout_validator::{validate_layout, validate_project_consistency};
pub use manifest::{parse_manifest, ManifestResult, ProjectManifest};
pub use multi_file::{compile_project, resolve_uses, ProjectResult};
pub use builder::{BuilderError, CascadeResult, LibraryContext, PatchProgramBuilder};
pub use formatter::{format_program, format_source};
pub use graph::{compile_to_graph, compile_project_to_graph_from_sources, compile_program_to_graph};
pub use parser::parse;

/// Parse PatchLang source and run all DRC checks.
/// Returns AST, parse errors, and semantic diagnostics.
///
/// When the source contains parse errors, DRC is skipped entirely because
/// the AST may be incomplete or malformed — running semantic checks on a
/// partial tree would produce misleading false positives.
pub fn check(source: &str) -> CheckResult {
    let parse_result = parse(source);
    if !parse_result.errors.is_empty() {
        let ts_result = to_ts_result(&parse_result);
        return CheckResult {
            program: ts_result.program,
            errors: ts_result.errors,
            diagnostics: Vec::new(),
        };
    }

    let (resolutions, auto_errors) = resolve_auto::resolve_auto_indices(&parse_result.program);
    let ts_result = to_ts_result_with_resolutions(&parse_result, &resolutions);

    let mut diagnostics: Vec<Diagnostic> = auto_errors
        .into_iter()
        .map(|e| Diagnostic {
            severity: drc::Severity::Error,
            layer: drc::DRCLayer::Structural,
            message: format!("{}: {}", e.code, e.message),
            span: Some(e.span),
            source: None,
            target: None,
            fix: None,
        })
        .collect();

    diagnostics.extend(drc::run_all(&parse_result.program, &builder::LibraryContext::empty()));

    CheckResult {
        program: ts_result.program,
        errors: ts_result.errors,
        diagnostics,
    }
}
