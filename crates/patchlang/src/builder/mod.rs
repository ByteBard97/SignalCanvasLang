//! Programmatic builder API for constructing and mutating `PatchProgram` values.
//!
//! Instead of concatenating PatchLang source text and re-parsing, frontends can
//! use `PatchProgramBuilder` to add/remove/reorder statements with eager
//! validation and cascade tracking.

pub mod canvas_emit;
pub mod canvas_input;
pub mod canvas_load;
pub mod canvas_output;
pub mod error;
pub mod insert_endpoints;
mod config;
mod connections;
mod instances;
mod routing;
mod signals;
mod templates;
pub(crate) mod validate;

pub use canvas_emit::emit_from_canvas_input;
pub use canvas_load::load_from_patch;

pub use error::{BuilderError, CascadeResult};

use std::collections::HashMap;

use crate::ast::{KvValue, PatchProgram, Statement};
use crate::compat::to_ts_program;
use crate::drc;
use crate::formatter::format_program;

/// Templates available from imported library files.
/// Set once via set_library(), used for validation and compilation.
/// NOT emitted by format_program() — the program's use statements are preserved instead.
pub struct LibraryContext {
    /// Template name -> owned TemplateDecl (parsed from library .patch files)
    pub templates: HashMap<String, crate::ast::TemplateDecl>,
}

impl LibraryContext {
    /// Create an empty library context.
    pub fn empty() -> Self {
        Self {
            templates: HashMap::new(),
        }
    }
}

/// Builder wrapper around a `PatchProgram` that provides validated mutation
/// operations, canonical ordering, and serialization helpers.
pub struct PatchProgramBuilder {
    program: PatchProgram,
    /// Tracks the next numeric suffix for auto-generated connect IDs,
    /// keyed by the base string `connect_{src}_{src_port}_{tgt}_{tgt_port}`.
    /// Used by `add_connect` (added in a later task).
    #[allow(dead_code)]
    connect_id_counter: HashMap<String, u32>,
    library: LibraryContext,
}

impl PatchProgramBuilder {
    /// Create a builder wrapping a new, empty program.
    pub fn new() -> Self {
        Self {
            program: PatchProgram {
                statements: Vec::new(),
            },
            connect_id_counter: HashMap::new(),
            library: LibraryContext::empty(),
        }
    }

    /// Wrap an existing program, seeding the connect-ID counter from any
    /// `Statement::Connect` entries already present.
    pub fn from_program(program: PatchProgram) -> Self {
        let mut counter: HashMap<String, u32> = HashMap::new();
        for stmt in &program.statements {
            if let Statement::Connect(c) = stmt {
                let src_inst = c.source.instance.as_deref().unwrap_or("_");
                let tgt_inst = c.target.instance.as_deref().unwrap_or("_");
                let base = format!(
                    "connect_{}_{}_{}_{}",
                    src_inst, c.source.port, tgt_inst, c.target.port
                );
                let entry = counter.entry(base).or_insert(0);
                *entry += 1;
            }
        }
        Self {
            program,
            connect_id_counter: counter,
            library: LibraryContext::empty(),
        }
    }

    /// Borrow the underlying program (read-only).
    pub fn program(&self) -> &PatchProgram {
        &self.program
    }

    /// Borrow the underlying program mutably.
    /// Prefer the typed mutation methods for validated changes.
    pub fn program_mut(&mut self) -> &mut PatchProgram {
        &mut self.program
    }

    /// Set library context from parsed library files. Replaces any existing library context.
    pub fn set_library(&mut self, library: LibraryContext) {
        self.library = library;
    }

    /// Get read-only access to library templates.
    pub fn library(&self) -> &LibraryContext {
        &self.library
    }

    /// Format the program as canonical PatchLang source text.
    pub fn format(&self) -> String {
        let ordered = self.canonical_program();
        format_program(&ordered)
    }

    /// Run all DRC checks against the current program state.
    pub fn check(&self) -> Vec<drc::Diagnostic> {
        drc::run_all(&self.program, &self.library)
    }

    /// Serialize the program to JSON (TS-compatible shape).
    pub fn to_json(&self) -> String {
        let ts = to_ts_program(&self.program);
        serde_json::to_string(&ts).unwrap_or_default()
    }

    /// Return a copy of the program with statements sorted into canonical
    /// section order. Within each section, insertion order is preserved.
    ///
    /// Section order: uses, card templates, non-card templates, instances,
    /// connects, bridges, bridge_groups, link_groups, signals, streams,
    /// flags, configs, rings.
    pub fn canonical_program(&self) -> PatchProgram {
        let mut uses = Vec::new();
        let mut card_templates = Vec::new();
        let mut templates = Vec::new();
        let mut instances = Vec::new();
        let mut connects = Vec::new();
        let mut bridges = Vec::new();
        let mut bridge_groups = Vec::new();
        let mut link_groups = Vec::new();
        let mut signals = Vec::new();
        let mut streams = Vec::new();
        let mut flags = Vec::new();
        let mut configs = Vec::new();
        let mut rings = Vec::new();

        for stmt in &self.program.statements {
            match stmt {
                Statement::Use(_) => uses.push(stmt.clone()),
                Statement::Template(t) => {
                    if is_card_template(t) {
                        card_templates.push(stmt.clone());
                    } else {
                        templates.push(stmt.clone());
                    }
                }
                Statement::Instance(_) => instances.push(stmt.clone()),
                Statement::Connect(_) => connects.push(stmt.clone()),
                Statement::Bridge(_) => bridges.push(stmt.clone()),
                Statement::BridgeGroup(_) => bridge_groups.push(stmt.clone()),
                Statement::LinkGroup(_) => link_groups.push(stmt.clone()),
                Statement::Signal(_) => signals.push(stmt.clone()),
                Statement::Stream(_) => streams.push(stmt.clone()),
                Statement::Flag(_) => flags.push(stmt.clone()),
                Statement::Config(_) => configs.push(stmt.clone()),
                Statement::Ring(_) => rings.push(stmt.clone()),
                Statement::Network(_) => {} // network blocks are declarative-only; no ordering needed
                Statement::Error(_) => {} // drop error nodes
            }
        }

        let mut ordered = Vec::new();
        ordered.extend(uses);
        ordered.extend(card_templates);
        ordered.extend(templates);
        ordered.extend(instances);
        ordered.extend(connects);
        ordered.extend(bridges);
        ordered.extend(bridge_groups);
        ordered.extend(link_groups);
        ordered.extend(signals);
        ordered.extend(streams);
        ordered.extend(flags);
        ordered.extend(configs);
        ordered.extend(rings);

        PatchProgram {
            statements: ordered,
        }
    }
}

impl Default for PatchProgramBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Check whether a template has `meta { kind: "card" }`.
fn is_card_template(t: &crate::ast::TemplateDecl) -> bool {
    t.meta.iter().any(|kv| {
        kv.key == "kind"
            && matches!(&kv.value, KvValue::Str { value } if value == "card")
    })
}
