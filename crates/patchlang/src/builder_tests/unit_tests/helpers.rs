//! Shared builders for PatchProgramBuilder unit tests.

use crate::ast::{
    ConnectDecl, InstanceDecl, PortDef, PortDirection, PortRef, RangeSpec, TemplateDecl,
};
use crate::builder::PatchProgramBuilder;
use crate::error::Span;

pub(super) fn default_span() -> Span {
    Span {
        start: 0,
        end: 0,
        file: None,
    }
}

/// Template with Dante_Out[1..8]: out(etherCON) [Dante] and
/// Dante_In[1..8]: in(etherCON) [Dante].
pub(super) fn make_simple_template(name: &str) -> TemplateDecl {
    TemplateDecl {
        name: name.to_string(),
        params: Vec::new(),
        version: None,
        meta: Vec::new(),
        ports: vec![
            PortDef {
                name: "Dante_Out".to_string(),
                range: Some(RangeSpec { start: 1, end: 8 }),
                direction: PortDirection::Out,
                connector: Some("etherCON".to_string()),
                attributes: vec!["Dante".to_string()],
                named_attributes: Vec::new(),
                span: default_span(),
            },
            PortDef {
                name: "Dante_In".to_string(),
                range: Some(RangeSpec { start: 1, end: 8 }),
                direction: PortDirection::In,
                connector: Some("etherCON".to_string()),
                attributes: vec!["Dante".to_string()],
                named_attributes: Vec::new(),
                span: default_span(),
            },
        ],
        bridges: Vec::new(),
        instances: Vec::new(),
        connects: Vec::new(),
        slots: Vec::new(),
        span: default_span(),
    }
}

/// Basic instance with no body.
pub(super) fn make_instance(name: &str, template: &str) -> InstanceDecl {
    InstanceDecl {
        name: name.to_string(),
        template_name: template.to_string(),
        args: Vec::new(),
        version_constraint: None,
        properties: Vec::new(),
        routes: Vec::new(),
        buses: Vec::new(),
        slot_assignments: Vec::new(),
        span: default_span(),
    }
}


pub(super) fn make_port_ref(instance: &str, port: &str, index: Option<u32>) -> PortRef {
    use crate::ast::{IndexElement, IndexSpec};
    PortRef {
        instance: Some(instance.to_string()),
        port: port.to_string(),
        index: index.map(|v| IndexSpec {
            elements: vec![IndexElement::Single { value: v }],
        }),
    }
}

/// Helper to push a connect statement directly into the builder program.
pub(super) fn push_connect(b: &mut PatchProgramBuilder, src: PortRef, tgt: PortRef) {
    b.program_mut().statements.push(crate::ast::Statement::Connect(
        ConnectDecl {
            source: src,
            target: tgt,
            properties: Vec::new(),
            suppressions: Vec::new(),
            mapping: None,
            span: default_span(),
        },
    ));
}


pub(super) fn builder_with_two_instances() -> PatchProgramBuilder {
    let mut b = PatchProgramBuilder::new();
    b.add_template(make_simple_template("Dante_AVIO")).unwrap();
    b.add_instance(make_instance("rio_1", "Dante_AVIO")).unwrap();
    b.add_instance(make_instance("rio_2", "Dante_AVIO")).unwrap();
    b
}

