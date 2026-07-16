//! Tests for the EasySchematic importer.
use super::*;

const MINIMAL_JSON: &str = r#"{
    "version": 29,
    "name": "Test",
    "nodes": [
        {
            "id": "device-1",
            "type": "device",
            "position": {"x": 100.0, "y": 200.0},
            "data": {
                "label": "My Mixer",
                "templateId": "tmpl-abc",
                "ports": [
                    {"id": "p-1", "label": "Out 1", "signalType": "analog-audio", "direction": "output", "connectorType": "xlr-3"},
                    {"id": "p-2", "label": "In 1",  "signalType": "analog-audio", "direction": "input",  "connectorType": "xlr-3"}
                ]
            }
        }
    ],
    "edges": []
}"#;

#[test]
fn parse_schematic_file() {
    let sf: SchematicFile = serde_json::from_str(MINIMAL_JSON).unwrap();
    assert_eq!(sf.version, 29);
    assert_eq!(sf.name, "Test");
    assert_eq!(sf.nodes.len(), 1);
    assert_eq!(sf.edges.len(), 0);
}

#[test]
fn parse_device_node() {
    let sf: SchematicFile = serde_json::from_str(MINIMAL_JSON).unwrap();
    let node = &sf.nodes[0];
    assert_eq!(node.id, "device-1");
    assert_eq!(node.node_type, "device");
    assert!((node.position.x - 100.0).abs() < f64::EPSILON);
    let dev = EsDeviceData::from_value(&node.data).unwrap();
    assert_eq!(dev.label, "My Mixer");
    assert_eq!(dev.template_id.as_deref(), Some("tmpl-abc"));
    assert_eq!(dev.ports.len(), 2);
    assert_eq!(dev.ports[0].id, "p-1");
    assert_eq!(dev.ports[0].direction, "output");
}

#[test]
fn parse_edge_with_handles() {
    let json = r#"{
        "version":1,"name":"T","nodes":[],
        "edges":[{"id":"e-1","source":"d1","target":"d2",
                  "sourceHandle":"p-1","targetHandle":"p-3",
                  "data":{"signalType":"sdi","cableId":"C001"}}]
    }"#;
    let sf: SchematicFile = serde_json::from_str(json).unwrap();
    let e = &sf.edges[0];
    assert_eq!(e.source_handle.as_deref(), Some("p-1"));
    assert_eq!(e.target_handle.as_deref(), Some("p-3"));
    let data = e.data.as_ref().unwrap();
    assert_eq!(data.signal_type.as_deref(), Some("sdi"));
    assert_eq!(data.cable_id.as_deref(), Some("C001"));
}

#[test]
fn parse_stub_label_node() {
    let json = r#"{
        "version":1,"name":"T",
        "nodes":[{"id":"stub-1","type":"stub-label",
                  "position":{"x":0.0,"y":0.0},
                  "data":{"linkedConnectionId":"conn-42","side":"source","signalType":"dante"}}],
        "edges":[]
    }"#;
    let sf: SchematicFile = serde_json::from_str(json).unwrap();
    let node = &sf.nodes[0];
    assert_eq!(node.node_type, "stub-label");
    assert_eq!(node.data["linkedConnectionId"].as_str().unwrap(), "conn-42");
    assert_eq!(node.data["side"].as_str().unwrap(), "source");
}

#[test]
fn basic_import_two_devices_one_connection() {
    let json = r#"{
        "version": 29, "name": "Simple Test",
        "nodes": [
            {"id": "device-1", "type": "device", "position": {"x": 100.0, "y": 50.0},
             "data": {"label": "Mixer", "model": "SQ6", "templateId": "tmpl-sq6",
                      "ports": [
                          {"id": "p-out", "label": "Dante Out", "signalType": "dante",
                           "direction": "output", "connectorType": "ethercon"},
                          {"id": "p-in",  "label": "Dante In",  "signalType": "dante",
                           "direction": "input",  "connectorType": "ethercon"}
                      ]}},
            {"id": "device-2", "type": "device", "position": {"x": 400.0, "y": 50.0},
             "data": {"label": "Stage Box", "model": "DX168", "templateId": "tmpl-dx168",
                      "ports": [
                          {"id": "p-rx", "label": "Dante In",  "signalType": "dante",
                           "direction": "input",  "connectorType": "ethercon"},
                          {"id": "p-tx", "label": "Dante Out", "signalType": "dante",
                           "direction": "output", "connectorType": "ethercon"}
                      ]}}
        ],
        "edges": [{"id": "e-1", "source": "device-1", "target": "device-2",
                   "sourceHandle": "p-out", "targetHandle": "p-rx",
                   "data": {"signalType": "dante", "cableId": "CAT-001"}}]
    }"#;
    let result = import_easyschematic(json).unwrap();
    assert!(result.patch.contains("template SQ6"));
    assert!(result.patch.contains("template DX168"));
    assert!(result.patch.contains("instance"));
    assert!(result.patch.contains("connect"));
    assert!(result.patch.contains("[Dante]"));
    assert_eq!(result.layout["positions"].as_object().unwrap().len(), 2);
}

#[test]
fn room_nodes_appear_in_annotations_not_patch() {
    let json = r#"{
        "version": 29, "name": "T",
        "nodes": [
            {"id": "room-1", "type": "room", "position": {"x": 0.0, "y": 0.0},
             "data": {"label": "Stage"}},
            {"id": "device-1", "type": "device", "position": {"x": 10.0, "y": 10.0},
             "data": {"label": "Camera", "templateId": "tmpl-cam",
                      "ports": [{"id": "p1", "label": "SDI Out", "signalType": "sdi",
                                 "direction": "output"}]}}
        ],
        "edges": []
    }"#;
    let result = import_easyschematic(json).unwrap();
    assert!(!result.patch.contains("Stage"));
    let annotations = result.layout["annotations"].as_array().unwrap();
    assert_eq!(annotations.len(), 1);
    assert_eq!(annotations[0]["label"].as_str().unwrap(), "Stage");
}

#[test]
fn stub_split_connection_is_rejoined() {
    let json = r#"{
        "version": 29, "name": "T",
        "nodes": [
            {"id": "dev-src", "type": "device", "position": {"x": 0.0, "y": 0.0},
             "data": {"label": "Source", "templateId": "tmpl-src",
                      "ports": [{"id": "p-out", "label": "SDI Out", "signalType": "sdi",
                                 "direction": "output"}]}},
            {"id": "dev-tgt", "type": "device", "position": {"x": 200.0, "y": 0.0},
             "data": {"label": "Display", "templateId": "tmpl-disp",
                      "ports": [{"id": "p-in", "label": "SDI In", "signalType": "sdi",
                                 "direction": "input"}]}},
            {"id": "stub-a", "type": "stub-label", "position": {"x": 80.0, "y": 0.0},
             "data": {"linkedConnectionId": "lc-1", "side": "source", "signalType": "sdi"}},
            {"id": "stub-b", "type": "stub-label", "position": {"x": 120.0, "y": 0.0},
             "data": {"linkedConnectionId": "lc-1", "side": "target", "signalType": "sdi"}}
        ],
        "edges": [
            {"id": "leg-1", "source": "dev-src", "target": "stub-a",
             "sourceHandle": "p-out", "targetHandle": null,
             "data": {"signalType": "sdi", "linkedConnectionId": "lc-1"}},
            {"id": "leg-2", "source": "stub-b", "target": "dev-tgt",
             "sourceHandle": null, "targetHandle": "p-in",
             "data": {"signalType": "sdi", "linkedConnectionId": "lc-1"}}
        ]
    }"#;
    let result = import_easyschematic(json).unwrap();
    assert!(result.patch.contains("connect"));
}

#[test]
fn invalid_json_returns_error() {
    let result = import_easyschematic("not json at all");
    assert!(result.is_err());
    assert!(result.unwrap_err().0.contains("JSON parse error"));
}

#[test]
fn layout_version_is_two() {
    let json = r#"{"version":1,"name":"T","nodes":[],"edges":[]}"#;
    let result = import_easyschematic(json).unwrap();
    assert_eq!(result.layout["version"].as_u64().unwrap(), 2);
}

#[test]
fn generated_patch_is_valid_patchlang() {
    let json = r#"{
        "version": 29, "name": "ValidityCheck",
        "nodes": [
            {"id": "d1", "type": "device", "position": {"x": 0.0, "y": 0.0},
             "data": {"label": "Mixer", "model": "SQ6", "templateId": "tmpl-sq6",
                      "ports": [
                          {"id": "p-out", "label": "Dante Out", "signalType": "dante",
                           "direction": "output", "connectorType": "ethercon"},
                          {"id": "p-in", "label": "Dante In", "signalType": "dante",
                           "direction": "input", "connectorType": "ethercon"}
                      ]}},
            {"id": "d2", "type": "device", "position": {"x": 400.0, "y": 0.0},
             "data": {"label": "Stage Box", "model": "DX168", "templateId": "tmpl-dx168",
                      "ports": [
                          {"id": "p-rx", "label": "Dante In", "signalType": "dante",
                           "direction": "input", "connectorType": "ethercon"},
                          {"id": "p-tx", "label": "Dante Out", "signalType": "dante",
                           "direction": "output", "connectorType": "ethercon"}
                      ]}}
        ],
        "edges": [{"id": "e-1", "source": "d1", "target": "d2",
                   "sourceHandle": "p-out", "targetHandle": "p-rx",
                   "data": {"signalType": "dante"}}]
    }"#;
    let result = import_easyschematic(json).unwrap();
    let check = crate::check(&result.patch);
    assert!(
        check.errors.is_empty(),
        "generated patch has parse errors:\n{}\n\nPatch:\n{}",
        check.errors.iter().map(|e| format!("{e:?}")).collect::<Vec<_>>().join("\n"),
        result.patch
    );
    let error_diags: Vec<_> = check
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, crate::drc::Severity::Error))
        .collect();
    assert!(
        error_diags.is_empty(),
        "generated patch has DRC errors:\n{}\n\nPatch:\n{}",
        error_diags.iter().map(|d| d.message.as_str()).collect::<Vec<_>>().join("\n"),
        result.patch
    );
}

#[test]
fn import_result_includes_device_list() {
    let json = r#"{
        "version": 29, "name": "T",
        "nodes": [
            {"id": "d1", "type": "device", "position": {"x": 0.0, "y": 0.0},
             "data": {"label": "Mixer", "model": "SQ6", "templateId": "tmpl-sq6",
                      "ports": [{"id": "p1", "label": "Dante Out", "signalType": "dante",
                                 "direction": "output"}]}},
            {"id": "d2", "type": "device", "position": {"x": 100.0, "y": 0.0},
             "data": {"label": "Stage Box", "model": "DX168", "templateId": "tmpl-dx168",
                      "ports": [{"id": "p2", "label": "Dante In", "signalType": "dante",
                                 "direction": "input"}]}}
        ],
        "edges": []
    }"#;
    let result = import_easyschematic(json).unwrap();
    assert_eq!(result.devices.len(), 2);
    let mixer = result.devices.iter().find(|d| d.label == "Mixer").unwrap();
    assert_eq!(mixer.instance_name, "Mixer");
    assert_eq!(mixer.template_name, "SQ6");
    assert_eq!(mixer.model.as_deref(), Some("SQ6"));
    let stage = result.devices.iter().find(|d| d.label == "Stage Box").unwrap();
    assert_eq!(stage.model.as_deref(), Some("DX168"));
}

#[test]
fn easyschematic_patch_roundtrips_through_canvas_load() {
    let json = r#"{
        "version": 29, "name": "RoundtripTest",
        "nodes": [
            {"id": "d1", "type": "device", "position": {"x": 0.0, "y": 0.0},
             "data": {"label": "Mixer", "model": "SQ6", "templateId": "tmpl-sq6",
                      "ports": [
                          {"id": "p-out", "label": "Dante Out", "signalType": "dante",
                           "direction": "output", "connectorType": "ethercon"}
                      ]}},
            {"id": "d2", "type": "device", "position": {"x": 400.0, "y": 0.0},
             "data": {"label": "Stage Box", "model": "DX168", "templateId": "tmpl-dx168",
                      "ports": [
                          {"id": "p-rx", "label": "Dante In", "signalType": "dante",
                           "direction": "input", "connectorType": "ethercon"}
                      ]}}
        ],
        "edges": [{"id": "e-1", "source": "d1", "target": "d2",
                   "sourceHandle": "p-out", "targetHandle": "p-rx",
                   "data": {"signalType": "dante"}}]
    }"#;
    let import_result = import_easyschematic(json).unwrap();
    let load_result = crate::builder::canvas_load::load_from_patch(&import_result.patch, "{}");
    match load_result {
        Err(e) => panic!("load_from_patch failed: {e:?}\nPatch:\n{}", import_result.patch),
        Ok(output) => {
            assert!(!output.instances.is_empty(), "expected instances, got none\nPatch:\n{}", import_result.patch);
            assert_eq!(output.instances.len(), 2, "expected 2 instances");
            assert!(!output.connections.is_empty(), "expected connections, got none");
        }
    }
}

#[test]
fn device_in_room_resolves_absolute_position() {
    let json = r#"{
        "version": 1, "name": "T",
        "nodes": [
            {"id": "room-1", "type": "room", "position": {"x": 640.0, "y": 144.0},
             "data": {"label": "Booth"}},
            {"id": "d1", "type": "device", "position": {"x": 100.0, "y": 50.0},
             "parentId": "room-1",
             "data": {"label": "Mixer", "templateId": "tmpl-m",
                      "ports": [{"id": "p1", "label": "Out", "signalType": "sdi",
                                 "direction": "output"}]}}
        ],
        "edges": []
    }"#;
    let result = import_easyschematic(json).unwrap();
    let pos = &result.layout["positions"]["Mixer"];
    assert!((pos["x"].as_f64().unwrap() - 740.0).abs() < 0.01, "x should be 640+100=740, got {}", pos["x"]);
    assert!((pos["y"].as_f64().unwrap() - 194.0).abs() < 0.01, "y should be 144+50=194, got {}", pos["y"]);
}

#[test]
fn device_without_parent_uses_raw_position() {
    let json = r#"{
        "version": 1, "name": "T",
        "nodes": [
            {"id": "d1", "type": "device", "position": {"x": 300.0, "y": 400.0},
             "data": {"label": "Camera", "templateId": "tmpl-c",
                      "ports": [{"id": "p1", "label": "SDI Out", "signalType": "sdi",
                                 "direction": "output"}]}}
        ],
        "edges": []
    }"#;
    let result = import_easyschematic(json).unwrap();
    let pos = &result.layout["positions"]["Camera"];
    assert!((pos["x"].as_f64().unwrap() - 300.0).abs() < 0.01);
    assert!((pos["y"].as_f64().unwrap() - 400.0).abs() < 0.01);
}

#[test]
fn non_signal_flow_ports_are_filtered_out() {
    let json = r#"{
        "version": 1, "name": "T",
        "nodes": [
            {"id": "d1", "type": "device", "position": {"x": 0.0, "y": 0.0},
             "data": {"label": "Mac Studio", "templateId": "tmpl-m",
                      "ports": [
                          {"id": "p1", "label": "HDMI Out",    "signalType": "hdmi",        "direction": "output"},
                          {"id": "p2", "label": "AC Power",    "signalType": "power",       "direction": "input"},
                          {"id": "p3", "label": "USB-A",       "signalType": "usb",         "direction": "bidirectional"},
                          {"id": "p4", "label": "Ethernet",    "signalType": "ethernet",    "direction": "output"},
                          {"id": "p5", "label": "Feed L1",     "signalType": "power-l1",    "direction": "output"},
                          {"id": "p6", "label": "SDI Out",     "signalType": "sdi",         "direction": "output"},
                          {"id": "p7", "label": "TB In",       "signalType": "thunderbolt", "direction": "input"},
                          {"id": "p8", "label": "DP Out",      "signalType": "displayport", "direction": "output"},
                          {"id": "p9", "label": "RS-232",      "signalType": "serial",      "direction": "bidirectional"},
                          {"id": "p10","label": "Composite In","signalType": "composite",   "direction": "input"},
                          {"id": "p11","label": "VGA In",      "signalType": "vga",         "direction": "input"}
                      ]}}
        ],
        "edges": []
    }"#;
    let result = import_easyschematic(json).unwrap();
    let dev = EsDeviceData::from_value(
        &serde_json::from_str::<serde_json::Value>(json).unwrap()["nodes"][0]["data"]
    ).unwrap();
    // only hdmi, ethernet, sdi survive
    assert_eq!(dev.ports.len(), 3);
    assert!(dev.ports.iter().any(|p| p.signal_type == "hdmi"));
    assert!(dev.ports.iter().any(|p| p.signal_type == "ethernet"));
    assert!(dev.ports.iter().any(|p| p.signal_type == "sdi"));
    // filtered types must not appear in the generated patch
    for banned in &["AC_Power", "USB", "thunderbolt", "displayport", "serial", "composite", "vga"] {
        assert!(!result.patch.to_lowercase().contains(&banned.to_lowercase()),
            "banned port type '{banned}' leaked into patch");
    }
}

#[test]
fn device_with_no_model_uses_none() {
    let json = r#"{
        "version": 1, "name": "T",
        "nodes": [
            {"id": "d1", "type": "device", "position": {"x": 0.0, "y": 0.0},
             "data": {"label": "My Gadget", "templateId": "tmpl-g",
                      "ports": [{"id": "p1", "label": "Out", "signalType": "sdi",
                                 "direction": "output"}]}}
        ],
        "edges": []
    }"#;
    let result = import_easyschematic(json).unwrap();
    assert_eq!(result.devices.len(), 1);
    assert!(result.devices[0].model.is_none());
    assert_eq!(result.devices[0].label, "My Gadget");
}
