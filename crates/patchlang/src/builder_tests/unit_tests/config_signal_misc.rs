//! Builder unit tests: config/labels, signals, cards, mapped connects, RF.
use std::collections::HashMap;

use crate::ast::{
    FlagDecl,
    KeyValue, KvValue, PortDef, PortDirection, RangeSpec, RingDecl,
    SignalDecl, SlotDef, Statement, StreamDecl, TemplateDecl,
};
use crate::builder::{BuilderError, PatchProgramBuilder};
use super::helpers::*;

#[test]
fn set_label_creates_config_block() {
    let mut b = PatchProgramBuilder::new();
    b.add_template(make_simple_template("Dev")).unwrap();
    b.add_instance(make_instance("SL", "Dev")).unwrap();
    b.set_label("SL", "Dante_In", 1, "Lead Vocal", HashMap::new()).unwrap();

    let configs: Vec<_> = b.program().statements.iter().filter_map(|s| match s {
        Statement::Config(c) => Some(c),
        _ => None,
    }).collect();
    assert_eq!(configs.len(), 1);
    assert_eq!(configs[0].name, "SL");
    assert_eq!(configs[0].labels[0].label, "Lead Vocal");
}

#[test]
fn set_label_adds_to_existing_config() {
    let mut b = PatchProgramBuilder::new();
    b.add_template(make_simple_template("Dev")).unwrap();
    b.add_instance(make_instance("SL", "Dev")).unwrap();
    b.set_label("SL", "Dante_In", 1, "Lead Vocal", HashMap::new()).unwrap();
    b.set_label("SL", "Dante_In", 2, "Bass DI", HashMap::new()).unwrap();

    let configs: Vec<_> = b.program().statements.iter().filter_map(|s| match s {
        Statement::Config(c) if c.name == "SL" => Some(c),
        _ => None,
    }).collect();
    assert_eq!(configs.len(), 1);
    assert_eq!(configs[0].labels.len(), 2);
}

#[test]
fn remove_label_works() {
    let mut b = PatchProgramBuilder::new();
    b.add_template(make_simple_template("Dev")).unwrap();
    b.add_instance(make_instance("SL", "Dev")).unwrap();
    b.set_label("SL", "Dante_In", 1, "Lead Vocal", HashMap::new()).unwrap();
    b.remove_label("SL", "Dante_In", 1).unwrap();
    // Config block should be removed when empty
    let config_count = b.program().statements.iter()
        .filter(|s| matches!(s, Statement::Config(_))).count();
    assert_eq!(config_count, 0);
}

#[test]
fn remove_config_removes_entire_block() {
    let mut b = PatchProgramBuilder::new();
    b.add_template(make_simple_template("Dev")).unwrap();
    b.add_instance(make_instance("SL", "Dev")).unwrap();
    b.set_label("SL", "Dante_In", 1, "Lead Vocal", HashMap::new()).unwrap();
    b.set_label("SL", "Dante_In", 2, "Bass DI", HashMap::new()).unwrap();
    b.remove_config("SL").unwrap();
    let config_count = b.program().statements.iter()
        .filter(|s| matches!(s, Statement::Config(_))).count();
    assert_eq!(config_count, 0);
}


#[test]
fn add_and_remove_signal() {
    let mut b = PatchProgramBuilder::new();
    b.add_signal(SignalDecl {
        name: "Lead_Vocal".to_string(),
        properties: vec![],
        origin: Some(make_port_ref("SL", "Dante_In", Some(1))),
        span: default_span(),
    }).unwrap();
    b.remove_signal("Lead_Vocal").unwrap();
    let count = b.program().statements.iter()
        .filter(|s| matches!(s, Statement::Signal(_))).count();
    assert_eq!(count, 0);
}

#[test]
fn add_signal_rejects_duplicate() {
    let mut b = PatchProgramBuilder::new();
    b.add_signal(SignalDecl {
        name: "Lead".to_string(), properties: vec![], origin: None, span: default_span(),
    }).unwrap();
    let err = b.add_signal(SignalDecl {
        name: "Lead".to_string(), properties: vec![], origin: None, span: default_span(),
    }).unwrap_err();
    assert!(matches!(err, BuilderError::DuplicateName(_)));
}

#[test]
fn add_and_remove_ring() {
    let mut b = PatchProgramBuilder::new();
    b.add_template(make_simple_template("Dev")).unwrap();
    b.add_instance(make_instance("SL", "Dev")).unwrap();

    b.add_ring(RingDecl {
        name: "OptoCore_Primary".to_string(),
        properties: vec![KeyValue {
            key: "protocol".to_string(),
            value: KvValue::Str { value: "OptoCore".to_string() },
        }],
        members: vec![],
        span: default_span(),
    }).unwrap();

    b.add_ring_member("OptoCore_Primary", "SL", None).unwrap();
    let ring = b.program().statements.iter().find_map(|s| match s {
        Statement::Ring(r) if r.name == "OptoCore_Primary" => Some(r),
        _ => None,
    }).unwrap();
    assert_eq!(ring.members.len(), 1);

    b.remove_ring_member("OptoCore_Primary", "SL").unwrap();
    let ring = b.program().statements.iter().find_map(|s| match s {
        Statement::Ring(r) if r.name == "OptoCore_Primary" => Some(r),
        _ => None,
    }).unwrap();
    assert_eq!(ring.members.len(), 0);

    b.remove_ring("OptoCore_Primary").unwrap();
}

#[test]
fn add_and_remove_stream() {
    let mut b = PatchProgramBuilder::new();
    b.add_stream(StreamDecl {
        name: "Main_Mix".to_string(), properties: vec![], source: None, span: default_span(),
    }).unwrap();
    b.remove_stream("Main_Mix").unwrap();
}

#[test]
fn add_and_remove_flag() {
    let mut b = PatchProgramBuilder::new();
    b.add_flag(FlagDecl {
        name: "rehearsal".to_string(), properties: vec![], span: default_span(),
    }).unwrap();
    b.remove_flag("rehearsal").unwrap();
}


#[test]
fn add_connect_accepts_card_port_after_set_slot() {
    let mut b = PatchProgramBuilder::new();

    // Card template with XLR[1..8]: in
    b.add_template(TemplateDecl {
        name: "VSR_AI8".to_string(),
        params: vec![],
        version: None,
        meta: vec![KeyValue {
            key: "kind".to_string(),
            value: KvValue::Str { value: "card".to_string() },
        }],
        ports: vec![PortDef {
            name: "XLR".to_string(),
            range: Some(RangeSpec { start: 1, end: 8 }),
            direction: PortDirection::In,
            connector: Some("XLR".to_string()),
            attributes: vec![],
            named_attributes: vec![],
            span: default_span(),
        }],
        bridges: vec![],
        instances: vec![],
        connects: vec![],
        slots: vec![],
        span: default_span(),
    }).unwrap();

    // Host template with a slot and an output port
    b.add_template(TemplateDecl {
        name: "Rack".to_string(),
        params: vec![],
        version: None,
        meta: vec![],
        ports: vec![PortDef {
            name: "MADI_Out".to_string(),
            range: Some(RangeSpec { start: 1, end: 48 }),
            direction: PortDirection::Out,
            connector: None,
            attributes: vec![],
            named_attributes: vec![],
            span: default_span(),
        }],
        bridges: vec![],
        instances: vec![],
        connects: vec![],
        slots: vec![SlotDef {
            name: "Input_Slot".to_string(),
            range: None,
            slot_type: "Input_Slot".to_string(),
            properties: vec![],
            span: default_span(),
        }],
        span: default_span(),
    }).unwrap();

    // Source template with output port
    b.add_template(TemplateDecl {
        name: "Splitter".to_string(),
        params: vec![],
        version: None,
        meta: vec![],
        ports: vec![PortDef {
            name: "Output_A".to_string(),
            range: Some(RangeSpec { start: 1, end: 80 }),
            direction: PortDirection::Out,
            connector: None,
            attributes: vec![],
            named_attributes: vec![],
            span: default_span(),
        }],
        bridges: vec![],
        instances: vec![],
        connects: vec![],
        slots: vec![],
        span: default_span(),
    }).unwrap();

    b.add_instance(make_instance("SR", "Rack")).unwrap();
    b.add_instance(make_instance("Split", "Splitter")).unwrap();

    // Install card into slot
    b.set_slot("SR", "Input_Slot", None, "VSR_AI8").unwrap();

    // This MUST succeed — XLR comes from the installed card
    let result = b.add_connect(
        make_port_ref("Split", "Output_A", Some(1)),
        make_port_ref("SR", "XLR", Some(1)),
        vec![],
    );
    assert!(result.is_ok(), "Should accept card port: {:?}", result);
}

#[test]
fn add_stream_appears_in_format() {
    let mut b = PatchProgramBuilder::new();
    b.add_template(make_simple_template("Dev")).unwrap();
    b.add_instance(make_instance("SL", "Dev")).unwrap();
    b.add_stream(StreamDecl {
        name: "Main_Mix".to_string(),
        properties: vec![KeyValue {
            key: "type".to_string(),
            value: KvValue::Str { value: "audio".to_string() },
        }],
        source: Some(make_port_ref("SL", "Dante_Out", Some(1))),
        span: default_span(),
    }).unwrap();
    let source = b.format();
    assert!(source.contains("stream"), "output should contain stream:\n{source}");
}


fn make_large_port_template(name: &str) -> TemplateDecl {
    TemplateDecl {
        name: name.to_string(),
        params: Vec::new(),
        version: None,
        meta: Vec::new(),
        ports: vec![
            PortDef {
                name: "Out".to_string(),
                range: Some(RangeSpec { start: 1, end: 80 }),
                direction: PortDirection::Out,
                connector: None,
                attributes: Vec::new(),
                named_attributes: Vec::new(),
                span: default_span(),
            },
            PortDef {
                name: "In".to_string(),
                range: Some(RangeSpec { start: 1, end: 80 }),
                direction: PortDirection::In,
                connector: None,
                attributes: Vec::new(),
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

#[test]
fn add_connect_mapped_splits_non_contiguous_ranges() {
    let mut b = PatchProgramBuilder::new();
    b.add_template(make_large_port_template("BigDev")).unwrap();
    b.add_instance(make_instance("src", "BigDev")).unwrap();
    b.add_instance(make_instance("dst", "BigDev")).unwrap();

    // Channels 1-3 and 7-9 (gap at 4-6)
    let mappings = vec![
        (1, 1, "Ch1".to_string()),
        (2, 2, "Ch2".to_string()),
        (3, 3, "Ch3".to_string()),
        (7, 7, "Ch7".to_string()),
        (8, 8, "Ch8".to_string()),
        (9, 9, "Ch9".to_string()),
    ];

    let ids = b.add_connect_mapped("src", "Out", "dst", "In", mappings, Vec::new()).unwrap();
    assert_eq!(ids.len(), 2, "should produce 2 connect statements for non-contiguous ranges");

    let source = b.format();
    assert!(source.contains("Out[1..3]"), "should contain Out[1..3], got:\n{source}");
    assert!(source.contains("Out[7..9]"), "should contain Out[7..9], got:\n{source}");
}

#[test]
fn add_connect_mapped_single_contiguous_range() {
    let mut b = PatchProgramBuilder::new();
    b.add_template(make_large_port_template("BigDev")).unwrap();
    b.add_instance(make_instance("src", "BigDev")).unwrap();
    b.add_instance(make_instance("dst", "BigDev")).unwrap();

    let mappings: Vec<(u32, u32, String)> = (1..=8)
        .map(|i| (i, i, format!("Ch{i}")))
        .collect();

    let ids = b.add_connect_mapped("src", "Out", "dst", "In", mappings, Vec::new()).unwrap();
    assert_eq!(ids.len(), 1, "contiguous range should produce single connect");
}

#[test]
fn add_connect_mapped_empty_mappings() {
    let mut b = PatchProgramBuilder::new();
    b.add_template(make_large_port_template("BigDev")).unwrap();
    b.add_instance(make_instance("src", "BigDev")).unwrap();
    b.add_instance(make_instance("dst", "BigDev")).unwrap();

    let ids = b.add_connect_mapped("src", "Out", "dst", "In", Vec::new(), Vec::new()).unwrap();
    assert_eq!(ids.len(), 1, "empty mappings should produce scalar connect");
}


fn make_rf_template(name: &str, subtype: &str) -> TemplateDecl {
    let (port_name, direction) = match subtype {
        "radio-mic" => ("Analog_Out", PortDirection::Out),
        "iem" => ("Analog_In", PortDirection::In),
        _ => ("Analog_Out", PortDirection::Out),
    };
    TemplateDecl {
        name: name.to_string(),
        params: Vec::new(),
        version: None,
        meta: vec![
            KeyValue {
                key: "kind".to_string(),
                value: KvValue::Str { value: "rf-system".to_string() },
            },
            KeyValue {
                key: "rf_subtype".to_string(),
                value: KvValue::Str { value: subtype.to_string() },
            },
        ],
        ports: vec![
            PortDef {
                name: port_name.to_string(),
                range: Some(RangeSpec { start: 1, end: 4 }),
                direction,
                connector: Some("XLR".to_string()),
                attributes: Vec::new(),
                named_attributes: Vec::new(),
                span: default_span(),
            },
            PortDef {
                name: "Dante_Out".to_string(),
                range: Some(RangeSpec { start: 1, end: 4 }),
                direction: PortDirection::Out,
                connector: Some("RJ45".to_string()),
                attributes: Vec::new(),
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

#[test]
fn set_rf_labels_on_radio_mic() {
    let mut b = PatchProgramBuilder::new();
    b.add_template(make_rf_template("AD4Q", "radio-mic")).unwrap();
    b.add_instance(make_instance("rf1", "AD4Q")).unwrap();

    let labels = vec![
        (1, "Vocal 1".to_string(), Vec::new()),
        (2, "Vocal 2".to_string(), Vec::new()),
    ];
    b.set_rf_labels("rf1", labels).unwrap();

    let source = b.format();
    assert!(source.contains(r#"label Analog_Out[1]: "Vocal 1""#),
        "should contain label for channel 1, got:\n{source}");
    assert!(source.contains(r#"label Analog_Out[2]: "Vocal 2""#),
        "should contain label for channel 2, got:\n{source}");
}

#[test]
fn set_rf_labels_rejects_non_rf_device() {
    let mut b = PatchProgramBuilder::new();
    b.add_template(make_simple_template("Dante_AVIO")).unwrap();
    b.add_instance(make_instance("rio", "Dante_AVIO")).unwrap();

    let err = b.set_rf_labels("rio", vec![(1, "Ch1".to_string(), Vec::new())]).unwrap_err();
    assert!(matches!(err, BuilderError::ValidationError(_)));
}
