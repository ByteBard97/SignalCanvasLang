//! Level 2 roundtrip tests: build via builder -> format -> parse -> compare AST.
//!
//! These tests verify the invariant that `format()` always produces parseable
//! PatchLang source, and that the reparsed AST matches the original structurally.

use std::collections::HashMap;

use crate::ast::{
    BusEntry, BusOutput, IndexElement, IndexSpec, InstanceDecl, PortDef, PortDirection,
    PortRef, RangeSpec, SignalDecl, Statement, TemplateDecl,
};
use crate::builder::PatchProgramBuilder;
use crate::error::Span;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn span() -> Span {
    Span {
        start: 0,
        end: 0,
        file: None,
    }
}

fn port_ref(instance: Option<&str>, port: &str, index: Option<IndexSpec>) -> PortRef {
    PortRef {
        instance: instance.map(|s| s.to_string()),
        port: port.to_string(),
        index,
    }
}

fn single_index(value: u32) -> IndexSpec {
    IndexSpec {
        elements: vec![IndexElement::Single { value }],
    }
}

fn range_index(start: u32, end: u32) -> IndexSpec {
    IndexSpec {
        elements: vec![IndexElement::Range { start, end }],
    }
}

/// Stagebox template: Dante_Out[1..32] out, Dante_In[1..32] in, Mic_In[1..32] in + bridge.
fn make_stagebox() -> TemplateDecl {
    TemplateDecl {
        name: "Stagebox_32".to_string(),
        params: Vec::new(),
        version: None,
        meta: Vec::new(),
        ports: vec![
            PortDef {
                name: "Dante_Out".to_string(),
                range: Some(RangeSpec { start: 1, end: 32 }),
                direction: PortDirection::Out,
                connector: Some("etherCON".to_string()),
                attributes: vec!["Dante".to_string()],
                named_attributes: Vec::new(),
                span: span(),
            },
            PortDef {
                name: "Dante_In".to_string(),
                range: Some(RangeSpec { start: 1, end: 32 }),
                direction: PortDirection::In,
                connector: Some("etherCON".to_string()),
                attributes: vec!["Dante".to_string()],
                named_attributes: Vec::new(),
                span: span(),
            },
            PortDef {
                name: "Mic_In".to_string(),
                range: Some(RangeSpec { start: 1, end: 32 }),
                direction: PortDirection::In,
                connector: Some("XLR".to_string()),
                attributes: vec!["Analog".to_string()],
                named_attributes: Vec::new(),
                span: span(),
            },
        ],
        bridges: vec![],
        instances: Vec::new(),
        connects: Vec::new(),
        slots: Vec::new(),
        span: span(),
    }
}

/// Console template: Dante_In[1..72] in, Dante_Out[1..24] out.
fn make_console() -> TemplateDecl {
    TemplateDecl {
        name: "Console_SD7".to_string(),
        params: Vec::new(),
        version: None,
        meta: Vec::new(),
        ports: vec![
            PortDef {
                name: "Dante_In".to_string(),
                range: Some(RangeSpec { start: 1, end: 72 }),
                direction: PortDirection::In,
                connector: Some("etherCON".to_string()),
                attributes: vec!["Dante".to_string()],
                named_attributes: Vec::new(),
                span: span(),
            },
            PortDef {
                name: "Dante_Out".to_string(),
                range: Some(RangeSpec { start: 1, end: 24 }),
                direction: PortDirection::Out,
                connector: Some("etherCON".to_string()),
                attributes: vec!["Dante".to_string()],
                named_attributes: Vec::new(),
                span: span(),
            },
        ],
        bridges: vec![],
        instances: Vec::new(),
        connects: Vec::new(),
        slots: Vec::new(),
        span: span(),
    }
}

fn make_instance(name: &str, template: &str) -> InstanceDecl {
    InstanceDecl {
        name: name.to_string(),
        template_name: template.to_string(),
        args: Vec::new(),
        version_constraint: None,
        properties: Vec::new(),
        routes: Vec::new(),
        buses: Vec::new(),
        slot_assignments: Vec::new(),
        span: span(),
    }
}

/// Parse source and assert zero errors, returning the parsed program.
fn parse_ok(source: &str) -> crate::ast::PatchProgram {
    let result = crate::parser::parse(source);
    assert!(
        result.errors.is_empty(),
        "parse errors: {:?}\n\nSource:\n{source}",
        result.errors
    );
    result.program
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn roundtrip_empty_program() {
    let b = PatchProgramBuilder::new();
    let source = b.format();
    let result = crate::parser::parse(&source);
    assert!(
        result.errors.is_empty(),
        "parse errors: {:?}\n\nSource:\n{source}",
        result.errors
    );
    let stmts = &result.program.statements;
    assert_eq!(stmts.len(), 0, "empty program should have zero statements");
}

#[test]
fn roundtrip_templates_only() {
    let mut b = PatchProgramBuilder::new();
    b.add_template(make_stagebox()).unwrap();
    b.add_template(make_console()).unwrap();

    let source = b.format();
    let program = parse_ok(&source);

    let templates: Vec<_> = program
        .statements
        .iter()
        .filter_map(|s| match s {
            Statement::Template(t) => Some(t),
            _ => None,
        })
        .collect();

    assert_eq!(
        templates.len(),
        2,
        "expected 2 templates, got {}.\n\nSource:\n{source}",
        templates.len()
    );

    // Verify names survive the roundtrip
    let names: Vec<&str> = templates.iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains(&"Stagebox_32"), "missing Stagebox_32");
    assert!(names.contains(&"Console_SD7"), "missing Console_SD7");

    // Verify port counts survive
    let sb = templates.iter().find(|t| t.name == "Stagebox_32").unwrap();
    assert_eq!(sb.ports.len(), 3, "Stagebox_32 should have 3 ports");

    let console = templates.iter().find(|t| t.name == "Console_SD7").unwrap();
    assert_eq!(console.ports.len(), 2, "Console_SD7 should have 2 ports");
}

#[test]
fn roundtrip_with_instances_and_connections() {
    let mut b = PatchProgramBuilder::new();
    b.add_template(make_stagebox()).unwrap();
    b.add_template(make_console()).unwrap();
    b.add_instance(make_instance("SL_Rack", "Stagebox_32"))
        .unwrap();
    b.add_instance(make_instance("FOH_Console", "Console_SD7"))
        .unwrap();

    // Connect Stagebox_32.Dante_Out[1..32] -> Console_SD7.Dante_In[1..32]
    b.add_connect(
        port_ref(Some("SL_Rack"), "Dante_Out", Some(range_index(1, 32))),
        port_ref(
            Some("FOH_Console"),
            "Dante_In",
            Some(range_index(1, 32)),
        ),
        Vec::new(),
    )
    .unwrap();

    let source = b.format();
    let program = parse_ok(&source);

    let template_count = program
        .statements
        .iter()
        .filter(|s| matches!(s, Statement::Template(_)))
        .count();
    let instance_count = program
        .statements
        .iter()
        .filter(|s| matches!(s, Statement::Instance(_)))
        .count();
    let connect_count = program
        .statements
        .iter()
        .filter(|s| matches!(s, Statement::Connect(_)))
        .count();

    assert_eq!(template_count, 2, "expected 2 templates\n\nSource:\n{source}");
    assert_eq!(instance_count, 2, "expected 2 instances\n\nSource:\n{source}");
    assert_eq!(connect_count, 1, "expected 1 connect\n\nSource:\n{source}");
}

#[test]
fn roundtrip_preserves_routes_and_buses() {
    let mut b = PatchProgramBuilder::new();
    b.add_template(make_stagebox()).unwrap();
    b.add_instance(make_instance("SL_Rack", "Stagebox_32"))
        .unwrap();

    // Add a route: Dante_In[1] -> Dante_Out[1]
    b.add_route("SL_Rack", "Dante_In", 1, "Dante_Out", 1)
        .unwrap();

    // Add a bus
    let bus = BusEntry {
        name: "PA_Matrix".to_string(),
        label: None,
        inputs: vec![port_ref(None, "Dante_In", Some(single_index(2)))],
        outputs: vec![BusOutput {
            label: "PA Out".to_string(),
            destinations: vec![port_ref(None, "Dante_Out", Some(single_index(2)))],
            span: span(),
        }],
        span: span(),
    insert_send: vec![],
    insert_return: vec![],
    };
    b.add_bus("SL_Rack", bus).unwrap();

    let source = b.format();
    let program = parse_ok(&source);

    // Find the instance in the reparsed program
    let instances: Vec<_> = program
        .statements
        .iter()
        .filter_map(|s| match s {
            Statement::Instance(i) if i.name == "SL_Rack" => Some(i),
            _ => None,
        })
        .collect();
    assert_eq!(instances.len(), 1, "expected 1 SL_Rack instance");

    let inst = instances[0];
    assert_eq!(
        inst.routes.len(),
        1,
        "expected 1 route on SL_Rack, got {}\n\nSource:\n{source}",
        inst.routes.len()
    );
    assert_eq!(
        inst.buses.len(),
        1,
        "expected 1 bus on SL_Rack, got {}\n\nSource:\n{source}",
        inst.buses.len()
    );
    assert_eq!(
        inst.buses[0].outputs[0].label,
        "PA Out",
        "bus output label should survive roundtrip\n\nSource:\n{source}"
    );
    assert_eq!(
        inst.buses[0].outputs[0].destinations[0].port,
        "Dante_Out",
        "bus output destination port should survive roundtrip\n\nSource:\n{source}"
    );
}

#[test]
fn roundtrip_with_signals_and_config() {
    let mut b = PatchProgramBuilder::new();
    b.add_template(make_stagebox()).unwrap();
    b.add_instance(make_instance("SL_Rack", "Stagebox_32"))
        .unwrap();

    // Add a signal with origin
    b.add_signal(SignalDecl {
        name: "Lead_Vocal".to_string(),
        properties: vec![],
        origin: Some(port_ref(Some("SL_Rack"), "Mic_In", Some(single_index(1)))),
        span: span(),
    })
    .unwrap();

    // Add a config label
    b.set_label("SL_Rack", "Mic_In", 1, "Lead Vocal", HashMap::new())
        .unwrap();

    let source = b.format();
    let program = parse_ok(&source);

    let signal_count = program
        .statements
        .iter()
        .filter(|s| matches!(s, Statement::Signal(_)))
        .count();
    let config_count = program
        .statements
        .iter()
        .filter(|s| matches!(s, Statement::Config(_)))
        .count();

    assert_eq!(
        signal_count, 1,
        "expected 1 signal\n\nSource:\n{source}"
    );
    assert_eq!(
        config_count, 1,
        "expected 1 config\n\nSource:\n{source}"
    );

    // Verify signal name survived
    let signal = program
        .statements
        .iter()
        .find_map(|s| match s {
            Statement::Signal(sig) => Some(sig),
            _ => None,
        })
        .unwrap();
    assert_eq!(signal.name, "Lead_Vocal");
    assert!(signal.origin.is_some(), "signal origin should survive roundtrip");

    // Verify config name survived
    let config = program
        .statements
        .iter()
        .find_map(|s| match s {
            Statement::Config(c) => Some(c),
            _ => None,
        })
        .unwrap();
    assert_eq!(config.name, "SL_Rack");
    assert_eq!(config.labels.len(), 1);
    assert_eq!(config.labels[0].label, "Lead Vocal");
}
