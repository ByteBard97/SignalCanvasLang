//! Builder unit tests: connect, route, and bus operations.

use crate::ast::{
    BusEntry, BusOutput, IndexElement, IndexSpec, PortRef,
};
use crate::builder::{BuilderError, PatchProgramBuilder};
use super::helpers::*;

#[test]
fn add_connect_returns_deterministic_id() {
    let mut b = builder_with_two_instances();
    let id = b
        .add_connect(
            make_port_ref("rio_1", "Dante_Out", Some(1)),
            make_port_ref("rio_2", "Dante_In", Some(1)),
            Vec::new(),
        )
        .unwrap();
    assert_eq!(id, "connect_rio_1_Dante_Out_rio_2_Dante_In");
}

#[test]
fn add_connect_disambiguates_duplicate_endpoints() {
    let mut b = builder_with_two_instances();
    let id1 = b
        .add_connect(
            make_port_ref("rio_1", "Dante_Out", Some(1)),
            make_port_ref("rio_2", "Dante_In", Some(1)),
            Vec::new(),
        )
        .unwrap();
    let id2 = b
        .add_connect(
            make_port_ref("rio_1", "Dante_Out", Some(2)),
            make_port_ref("rio_2", "Dante_In", Some(2)),
            Vec::new(),
        )
        .unwrap();
    assert_eq!(id1, "connect_rio_1_Dante_Out_rio_2_Dante_In");
    assert_eq!(id2, "connect_rio_1_Dante_Out_rio_2_Dante_In_2");
}

#[test]
fn add_connect_rejects_unknown_instance() {
    let mut b = builder_with_two_instances();
    let err = b
        .add_connect(
            make_port_ref("nonexistent", "Dante_Out", Some(1)),
            make_port_ref("rio_2", "Dante_In", Some(1)),
            Vec::new(),
        )
        .unwrap_err();
    assert!(matches!(err, BuilderError::NotFound(_)));
}

#[test]
fn add_connect_rejects_unknown_port() {
    let mut b = builder_with_two_instances();
    let err = b
        .add_connect(
            make_port_ref("rio_1", "NoSuchPort", Some(1)),
            make_port_ref("rio_2", "Dante_In", Some(1)),
            Vec::new(),
        )
        .unwrap_err();
    assert!(matches!(err, BuilderError::PortNotFound { .. }));
}

#[test]
fn add_connect_rejects_output_to_output() {
    let mut b = builder_with_two_instances();
    let err = b
        .add_connect(
            make_port_ref("rio_1", "Dante_Out", Some(1)),
            make_port_ref("rio_2", "Dante_Out", Some(1)),
            Vec::new(),
        )
        .unwrap_err();
    assert!(matches!(err, BuilderError::DirectionViolation { .. }));
}

#[test]
fn add_connect_rejects_input_to_input() {
    let mut b = builder_with_two_instances();
    let err = b
        .add_connect(
            make_port_ref("rio_1", "Dante_In", Some(1)),
            make_port_ref("rio_2", "Dante_In", Some(1)),
            Vec::new(),
        )
        .unwrap_err();
    assert!(matches!(err, BuilderError::DirectionViolation { .. }));
}

#[test]
fn remove_connect_by_id() {
    let mut b = builder_with_two_instances();
    let id = b
        .add_connect(
            make_port_ref("rio_1", "Dante_Out", Some(1)),
            make_port_ref("rio_2", "Dante_In", Some(1)),
            Vec::new(),
        )
        .unwrap();

    let connect_count = b
        .program()
        .statements
        .iter()
        .filter(|s| matches!(s, crate::ast::Statement::Connect(_)))
        .count();
    assert_eq!(connect_count, 1);

    b.remove_connect(&id).unwrap();

    let connect_count = b
        .program()
        .statements
        .iter()
        .filter(|s| matches!(s, crate::ast::Statement::Connect(_)))
        .count();
    assert_eq!(connect_count, 0);
}


#[test]
fn add_route_to_instance() {
    let mut b = PatchProgramBuilder::new();
    b.add_template(make_simple_template("Dev")).unwrap();
    b.add_instance(make_instance("SL", "Dev")).unwrap();
    b.add_route("SL", "Dante_In", 1, "Dante_Out", 1).unwrap();
    assert_eq!(b.get_instance("SL").unwrap().routes.len(), 1);
}

#[test]
fn add_route_rejects_unknown_instance() {
    let mut b = PatchProgramBuilder::new();
    let err = b.add_route("NonExistent", "A", 1, "B", 1).unwrap_err();
    assert!(matches!(err, BuilderError::NotFound(_)));
}

#[test]
fn set_routes_replaces_all() {
    let mut b = PatchProgramBuilder::new();
    b.add_template(make_simple_template("Dev")).unwrap();
    b.add_instance(make_instance("SL", "Dev")).unwrap();
    b.add_route("SL", "Dante_In", 1, "Dante_Out", 1).unwrap();
    b.add_route("SL", "Dante_In", 2, "Dante_Out", 2).unwrap();
    assert_eq!(b.get_instance("SL").unwrap().routes.len(), 2);
    b.set_routes("SL", vec![]).unwrap();
    assert_eq!(b.get_instance("SL").unwrap().routes.len(), 0);
}

#[test]
fn clear_routes_works() {
    let mut b = PatchProgramBuilder::new();
    b.add_template(make_simple_template("Dev")).unwrap();
    b.add_instance(make_instance("SL", "Dev")).unwrap();
    b.add_route("SL", "Dante_In", 1, "Dante_Out", 1).unwrap();
    b.clear_routes("SL").unwrap();
    assert_eq!(b.get_instance("SL").unwrap().routes.len(), 0);
}

#[test]
fn add_bus_to_instance() {
    let mut b = PatchProgramBuilder::new();
    b.add_template(make_simple_template("Dev")).unwrap();
    b.add_instance(make_instance("SL", "Dev")).unwrap();
    let bus = BusEntry {
        name: "PA_Matrix".to_string(),
        label: None,
        inputs: vec![PortRef {
            instance: None,
            port: "Dante_In".to_string(),
            index: Some(IndexSpec { elements: vec![IndexElement::Single { value: 1 }] }),
        }],
        outputs: vec![BusOutput {
            label: "PA Out".to_string(),
            destinations: vec![PortRef {
                instance: None,
                port: "Dante_Out".to_string(),
                index: Some(IndexSpec { elements: vec![IndexElement::Single { value: 1 }] }),
            }],
            span: default_span(),
        }],
        span: default_span(),
    insert_send: vec![],
    insert_return: vec![],
    };
    b.add_bus("SL", bus).unwrap();
    let buses = &b.get_instance("SL").unwrap().buses;
    assert_eq!(buses.len(), 1);
    assert_eq!(buses[0].outputs[0].label, "PA Out");
    assert_eq!(buses[0].outputs[0].destinations[0].port, "Dante_Out");
}

#[test]
fn bus_with_unrouted_output() {
    let mut b = PatchProgramBuilder::new();
    b.add_template(make_simple_template("Dev")).unwrap();
    b.add_instance(make_instance("SL", "Dev")).unwrap();
    let bus = BusEntry {
        name: "Unrouted".to_string(),
        label: None,
        inputs: vec![],
        outputs: vec![BusOutput {
            label: "Pending Mix".to_string(),
            destinations: vec![],
            span: default_span(),
        }],
        span: default_span(),
    insert_send: vec![],
    insert_return: vec![],
    };
    b.add_bus("SL", bus).unwrap();
    let buses = &b.get_instance("SL").unwrap().buses;
    assert_eq!(buses[0].outputs[0].label, "Pending Mix");
    assert_eq!(buses[0].outputs[0].destinations.len(), 0);
}

#[test]
fn remove_bus_by_name() {
    let mut b = PatchProgramBuilder::new();
    b.add_template(make_simple_template("Dev")).unwrap();
    b.add_instance(make_instance("SL", "Dev")).unwrap();
    let bus = BusEntry {
        name: "PA".to_string(),
        label: None,
        inputs: vec![],
        outputs: vec![],
        span: default_span(),
    insert_send: vec![],
    insert_return: vec![],
    };
    b.add_bus("SL", bus).unwrap();
    b.remove_bus("SL", "PA").unwrap();
    assert_eq!(b.get_instance("SL").unwrap().buses.len(), 0);
}

