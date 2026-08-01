// PyO3 0.22 macro-generated code triggers this lint on newer Rust toolchains.
#![allow(clippy::useless_conversion)]

use std::collections::HashMap;

use pyo3::prelude::*;
use pyo3::exceptions::PyValueError;

use patchlang::builder::PatchProgramBuilder;

// ---------------------------------------------------------------------------
// ProgramBuilder class
// ---------------------------------------------------------------------------

/// Programmatic builder for constructing and mutating PatchLang programs.
#[pyclass]
struct ProgramBuilder {
    inner: PatchProgramBuilder,
}

#[pymethods]
impl ProgramBuilder {
    /// Create an empty program builder.
    #[new]
    fn new() -> Self {
        Self {
            inner: PatchProgramBuilder::new(),
        }
    }

    /// Create a builder from PatchLang source text.
    #[staticmethod]
    fn from_source(source: &str) -> PyResult<Self> {
        let result = patchlang::parse(source);
        if !result.is_valid() {
            let msgs: Vec<String> = result.errors.iter().map(|e| e.to_string()).collect();
            return Err(PyValueError::new_err(msgs.join("; ")));
        }
        Ok(Self {
            inner: PatchProgramBuilder::from_program(result.program),
        })
    }

    /// Format the program as canonical PatchLang source text.
    fn format(&self) -> String {
        self.inner.format()
    }

    /// Run DRC checks and return diagnostics as JSON.
    fn check(&self) -> PyResult<String> {
        let diags = self.inner.check();
        serde_json::to_string(&diags)
            .map_err(|e| PyValueError::new_err(format!("serialization failed: {e}")))
    }

    /// Serialize the program to JSON (TS-compatible shape).
    fn to_json(&self) -> String {
        self.inner.to_json()
    }

    /// Add a template from JSON.
    fn add_template(&mut self, template_json: &str) -> PyResult<()> {
        let decl: patchlang::ast::TemplateDecl = serde_json::from_str(template_json)
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
        self.inner
            .add_template(decl)
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    /// Remove a template by name.
    fn remove_template(&mut self, name: &str) -> PyResult<()> {
        self.inner
            .remove_template(name)
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    /// Add an instance from JSON.
    fn add_instance(&mut self, instance_json: &str) -> PyResult<()> {
        let decl: patchlang::ast::InstanceDecl = serde_json::from_str(instance_json)
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
        self.inner
            .add_instance(decl)
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    /// Remove an instance by name. Returns cascade result as JSON.
    fn remove_instance(&mut self, name: &str) -> PyResult<String> {
        let cascade = self
            .inner
            .remove_instance(name)
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
        serde_json::to_string(&serde_json::json!({
            "removed_connects": cascade.removed_connects,
            "removed_bridges": cascade.removed_bridges,
            "removed_configs": cascade.removed_configs,
            "removed_ring_members": cascade.removed_ring_members,
            "removed_signal_origins": cascade.removed_signal_origins,
            "removed_stream_sources": cascade.removed_stream_sources,
        }))
        .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    /// Add a connection. Returns the deterministic ID.
    fn add_connect(
        &mut self,
        source_json: &str,
        target_json: &str,
        props_json: &str,
    ) -> PyResult<String> {
        let source: patchlang::ast::PortRef = serde_json::from_str(source_json)
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
        let target: patchlang::ast::PortRef = serde_json::from_str(target_json)
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
        let props: Vec<patchlang::ast::KeyValue> = serde_json::from_str(props_json)
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
        self.inner
            .add_connect(source, target, props)
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    /// Remove a connection by ID.
    fn remove_connect(&mut self, id: &str) -> PyResult<()> {
        self.inner
            .remove_connect(id)
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    /// Add an internal route to an instance.
    fn add_route(
        &mut self,
        instance: &str,
        from_port: &str,
        from_ch: u32,
        to_port: &str,
        to_ch: u32,
    ) -> PyResult<()> {
        self.inner
            .add_route(instance, from_port, from_ch, to_port, to_ch)
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    /// Set a channel label on an instance.
    ///
    /// `props` carries the label's properties — `phantom`, `source_type`, `capsule`,
    /// `rf_band`, `insert_send`/`insert_return` (#31), or any custom key. It was
    /// previously hard-coded to an empty map, so the Python binding could not set a
    /// single property; the WASM binding has accepted them all along (#32).
    ///
    /// A native dict, not a JSON string. The WASM binding takes JSON only because it
    /// crosses a boundary that has no dict type; Python has one.
    ///
    /// `Option` is required because pyo3 0.22 cannot default a bare `HashMap` in the
    /// signature string — it carries no meaning here, as `None` and `{}` both forward
    /// an empty map.
    #[pyo3(signature = (instance, port, index, label, props=None))]
    fn set_label(
        &mut self,
        instance: &str,
        port: &str,
        index: u32,
        label: &str,
        props: Option<HashMap<String, String>>,
    ) -> PyResult<()> {
        self.inner
            .set_label(instance, port, index, label, props.unwrap_or_default())
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }
}

// ---------------------------------------------------------------------------
// Standalone functions (unchanged)
// ---------------------------------------------------------------------------

/// Parse PatchLang source and return JSON string with AST and errors.
#[pyfunction]
fn parse(source: &str) -> PyResult<String> {
    let result = patchlang::parse(source);
    serde_json::to_string(&result)
        .map_err(|e| PyValueError::new_err(format!("serialization failed: {e}")))
}

/// Validate PatchLang source. Returns True if no errors.
#[pyfunction]
fn validate(source: &str) -> bool {
    patchlang::parse(source).is_valid()
}

/// Generate a deterministic port ID.
/// Pass index=None for single (non-ranged) ports; pass an integer for ranged ports.
#[pyfunction]
#[pyo3(signature = (instance_name, template_name, port_name, index=None))]
fn generate_port_id(
    instance_name: &str,
    template_name: &str,
    port_name: &str,
    index: Option<u32>,
) -> String {
    patchlang::generate_port_id(instance_name, template_name, port_name, index)
}

/// Generate a deterministic route ID.
#[pyfunction]
fn generate_route_id(
    template_name: &str,
    source_port: &str,
    target_port: &str,
) -> String {
    patchlang::generate_route_id(template_name, source_port, target_port)
}

/// Generate a deterministic slot ID.
#[pyfunction]
fn generate_slot_id(
    template_name: &str,
    slot_name: &str,
) -> String {
    patchlang::generate_slot_id(template_name, slot_name)
}

/// Format PatchLang source into canonical style.
/// Returns the formatted source string, or raises ValueError on parse errors.
#[pyfunction]
fn format_source(source: &str) -> PyResult<String> {
    patchlang::format_source(source)
        .map_err(PyValueError::new_err)
}

/// Parse PatchLang source and run DRC checks.
/// Returns JSON string with { program, errors, diagnostics }.
#[pyfunction]
fn check(source: &str) -> PyResult<String> {
    let result = patchlang::check(source);
    serde_json::to_string(&result)
        .map_err(|e| PyValueError::new_err(format!("serialization failed: {e}")))
}

/// Quick-parse source and return namespace strings from `use` statements.
#[pyfunction]
fn resolve_uses(source: &str) -> PyResult<Vec<String>> {
    Ok(patchlang::resolve_uses(source))
}

/// Multi-file compilation with namespace resolution and merged DRC.
///
/// `files` is a dict mapping file paths to source strings.
/// `entry` is the path of the entry file.
/// Returns JSON string with { program, errors, diagnostics }.
#[pyfunction]
fn compile_project(files: HashMap<String, String>, entry: &str) -> PyResult<String> {
    let result = patchlang::compile_project(files, entry);
    serde_json::to_string(&result)
        .map_err(|e| PyValueError::new_err(format!("serialization failed: {e}")))
}

/// Parse and validate a project.json manifest string.
/// Returns JSON string: `{ "manifest": {...} | null, "errors": [...] }`.
#[pyfunction]
fn parse_manifest(json: &str) -> PyResult<String> {
    let result = patchlang::parse_manifest(json);
    serde_json::to_string(&result)
        .map_err(|e| PyValueError::new_err(format!("serialization failed: {e}")))
}

/// Validate a `.layout.json` string against the schema.
/// Returns JSON: `{ "valid": bool, "errors": [...] }`.
#[pyfunction]
fn validate_layout(json: &str) -> String {
    patchlang::validate_layout(json)
}

/// Cross-validate instance names between a `.patch` source and a `.layout.json`.
/// Returns JSON: `{ "valid": bool, "errors": [...], "warnings": [...] }`.
#[pyfunction]
fn validate_project_consistency(patch: &str, layout: &str) -> String {
    patchlang::validate_project_consistency(patch, layout)
}

/// Python module definition.
#[pymodule]
fn patchlang_python(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<ProgramBuilder>()?;
    m.add_function(wrap_pyfunction!(parse, m)?)?;
    m.add_function(wrap_pyfunction!(validate, m)?)?;
    m.add_function(wrap_pyfunction!(format_source, m)?)?;
    m.add_function(wrap_pyfunction!(check, m)?)?;
    m.add_function(wrap_pyfunction!(resolve_uses, m)?)?;
    m.add_function(wrap_pyfunction!(compile_project, m)?)?;
    m.add_function(wrap_pyfunction!(generate_port_id, m)?)?;
    m.add_function(wrap_pyfunction!(generate_route_id, m)?)?;
    m.add_function(wrap_pyfunction!(generate_slot_id, m)?)?;
    m.add_function(wrap_pyfunction!(parse_manifest, m)?)?;
    m.add_function(wrap_pyfunction!(validate_layout, m)?)?;
    m.add_function(wrap_pyfunction!(validate_project_consistency, m)?)?;
    Ok(())
}
