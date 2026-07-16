//! Builder unit tests: template + instance operations.
use std::collections::HashMap;

use crate::ast::{
    PortDef, PortDirection,
};
use crate::builder::{BuilderError, PatchProgramBuilder};
use super::helpers::*;

#[test]
fn add_template_stores_declaration() {
    let mut b = PatchProgramBuilder::new();
    b.add_template(make_simple_template("Dante_AVIO"))
        .unwrap();
    assert_eq!(b.template_names().len(), 1);
    assert_eq!(b.template_names()[0], "Dante_AVIO");
}

#[test]
fn add_template_rejects_duplicate_name() {
    let mut b = PatchProgramBuilder::new();
    b.add_template(make_simple_template("Dante_AVIO"))
        .unwrap();
    let err = b
        .add_template(make_simple_template("Dante_AVIO"))
        .unwrap_err();
    assert!(matches!(err, BuilderError::DuplicateName(_)));
}

#[test]
fn get_template_returns_declaration() {
    let mut b = PatchProgramBuilder::new();
    b.add_template(make_simple_template("Dante_AVIO"))
        .unwrap();
    let t = b.get_template("Dante_AVIO").unwrap();
    assert_eq!(t.ports.len(), 2);
}

#[test]
fn get_template_returns_none_for_missing() {
    let b = PatchProgramBuilder::new();
    assert!(b.get_template("Nonexistent").is_none());
}

#[test]
fn remove_template_succeeds_when_unreferenced() {
    let mut b = PatchProgramBuilder::new();
    b.add_template(make_simple_template("Dante_AVIO"))
        .unwrap();
    b.remove_template("Dante_AVIO").unwrap();
    assert!(b.template_names().is_empty());
}

#[test]
fn remove_template_fails_when_instances_reference_it() {
    let mut b = PatchProgramBuilder::new();
    b.add_template(make_simple_template("Dante_AVIO"))
        .unwrap();
    b.add_instance(make_instance("rio_1", "Dante_AVIO"))
        .unwrap();
    let err = b.remove_template("Dante_AVIO").unwrap_err();
    assert!(matches!(err, BuilderError::InUse(_)));
}

#[test]
fn update_template_replaces_declaration() {
    let mut b = PatchProgramBuilder::new();
    b.add_template(make_simple_template("Dante_AVIO"))
        .unwrap();
    assert_eq!(b.get_template("Dante_AVIO").unwrap().ports.len(), 2);

    // Replace with a template that has 3 ports
    let mut updated = make_simple_template("Dante_AVIO");
    updated.ports.push(PortDef {
        name: "WordClock".to_string(),
        range: None,
        direction: PortDirection::Io,
        connector: Some("BNC".to_string()),
        attributes: Vec::new(),
        named_attributes: Vec::new(),
        span: default_span(),
    });
    b.update_template("Dante_AVIO", updated).unwrap();
    assert_eq!(b.get_template("Dante_AVIO").unwrap().ports.len(), 3);
}

#[test]
fn update_template_fails_for_missing() {
    let mut b = PatchProgramBuilder::new();
    let err = b
        .update_template("Nonexistent", make_simple_template("Nonexistent"))
        .unwrap_err();
    assert!(matches!(err, BuilderError::NotFound(_)));
}


#[test]
fn add_instance_stores_declaration() {
    let mut b = PatchProgramBuilder::new();
    b.add_template(make_simple_template("Dante_AVIO")).unwrap();
    b.add_instance(make_instance("rio_1", "Dante_AVIO")).unwrap();
    assert!(b.get_instance("rio_1").is_some());
    assert_eq!(b.get_instance("rio_1").unwrap().template_name, "Dante_AVIO");
}

#[test]
fn add_instance_rejects_unknown_template() {
    let mut b = PatchProgramBuilder::new();
    let err = b
        .add_instance(make_instance("rio_1", "Nonexistent"))
        .unwrap_err();
    assert!(matches!(err, BuilderError::NotFound(_)));
}

#[test]
fn add_instance_rejects_duplicate_name() {
    let mut b = PatchProgramBuilder::new();
    b.add_template(make_simple_template("Dante_AVIO")).unwrap();
    b.add_instance(make_instance("rio_1", "Dante_AVIO")).unwrap();
    let err = b
        .add_instance(make_instance("rio_1", "Dante_AVIO"))
        .unwrap_err();
    assert!(matches!(err, BuilderError::DuplicateName(_)));
}

#[test]
fn remove_instance_succeeds() {
    let mut b = PatchProgramBuilder::new();
    b.add_template(make_simple_template("Dante_AVIO")).unwrap();
    b.add_instance(make_instance("rio_1", "Dante_AVIO")).unwrap();
    b.remove_instance("rio_1").unwrap();
    assert!(b.get_instance("rio_1").is_none());
}

#[test]
fn remove_instance_cascades_connections() {
    let mut b = PatchProgramBuilder::new();
    b.add_template(make_simple_template("Dante_AVIO")).unwrap();
    b.add_instance(make_instance("rio_1", "Dante_AVIO")).unwrap();
    b.add_instance(make_instance("rio_2", "Dante_AVIO")).unwrap();

    // Insert a connect: rio_1.Dante_Out -> rio_2.Dante_In
    push_connect(
        &mut b,
        make_port_ref("rio_1", "Dante_Out", Some(1)),
        make_port_ref("rio_2", "Dante_In", Some(1)),
    );

    let connect_count_before = b
        .program()
        .statements
        .iter()
        .filter(|s| matches!(s, crate::ast::Statement::Connect(_)))
        .count();
    assert_eq!(connect_count_before, 1);

    let cascade = b.remove_instance("rio_1").unwrap();
    assert_eq!(cascade.removed_connects.len(), 1);

    let connect_count_after = b
        .program()
        .statements
        .iter()
        .filter(|s| matches!(s, crate::ast::Statement::Connect(_)))
        .count();
    assert_eq!(connect_count_after, 0);
}

#[test]
fn remove_instance_fails_for_missing() {
    let mut b = PatchProgramBuilder::new();
    let err = b.remove_instance("nonexistent").unwrap_err();
    assert!(matches!(err, BuilderError::NotFound(_)));
}

#[test]
fn update_instance_properties_works() {
    let mut b = PatchProgramBuilder::new();
    b.add_template(make_simple_template("Dante_AVIO")).unwrap();
    b.add_instance(make_instance("rio_1", "Dante_AVIO")).unwrap();

    let mut props = HashMap::new();
    props.insert("location".to_string(), "Stage Left".to_string());
    b.update_instance_properties("rio_1", props).unwrap();

    let inst = b.get_instance("rio_1").unwrap();
    assert_eq!(inst.properties.len(), 1);
    assert_eq!(inst.properties[0].key, "location");
}

