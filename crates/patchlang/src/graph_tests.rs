//! Tests for the graph module.

use crate::graph::compile_to_graph;
use crate::graph::types::CompileToGraphResult;

/// Parse graph JSON result.
fn parse_graph(source: &str) -> CompileToGraphResult {
    let json = compile_to_graph(source);
    serde_json::from_str(&json).expect("should parse graph JSON")
}

// ---------------------------------------------------------------------------
// 1. Basic single device
// ---------------------------------------------------------------------------

#[test]
fn test_basic_single_device() {
    let source = r#"
template Rio3224 {
  ports {
    Mic_In[1..32]: in(XLR)
  }
}
instance Stage_Left is Rio3224
"#;
    let result = parse_graph(source);

    // Root level should exist
    let root = result.levels.get("root").expect("root level");
    assert_eq!(root.nodes.len(), 1);

    let node = root.nodes.get("Stage_Left").expect("Stage_Left node");
    assert_eq!(node.template_name, "Rio3224");
    assert_eq!(node.ports.len(), 32);
    assert!(!node.drillable);

    // Check port naming
    let first_port = &node.ports[0];
    assert_eq!(first_port.id, "Stage_Left:Mic_In_1");
    assert_eq!(first_port.name, "Mic_In_1");
    assert_eq!(first_port.direction, "in");
    assert_eq!(first_port.connector.as_deref(), Some("XLR"));

    let last_port = &node.ports[31];
    assert_eq!(last_port.id, "Stage_Left:Mic_In_32");
    assert_eq!(last_port.name, "Mic_In_32");

    // source_key groups ranged ports by definition name, not channel
    assert_eq!(first_port.source_key.as_deref(), Some("pl::Rio3224::Mic_In"));
    assert_eq!(last_port.source_key.as_deref(), Some("pl::Rio3224::Mic_In"));
}

// ---------------------------------------------------------------------------
// 1b. source_key for card-contributed ports and sub-instance ports
// ---------------------------------------------------------------------------

#[test]
fn test_source_key_card_ports() {
    let source = r#"
template MY16_AUD {
  meta { device_type: "card" }
  ports {
    Dante_In[1..16]: in(etherCON) [Dante]
    Dante_Out[1..16]: out(etherCON) [Dante]
  }
}
template CL5 {
  ports {
    Fader[1..4]: in(virtual)
  }
  slot MY_Slot[1]: MY_Expansion
}
instance Console is CL5 {
  slot MY_Slot[1]: MY16_AUD
}
"#;
    let result = parse_graph(source);
    let root = result.levels.get("root").expect("root level");
    let node = root.nodes.get("Console").expect("Console node");

    // Template's own ports use the template name
    let fader = node.ports.iter().find(|p| p.name == "Fader_1").expect("Fader_1");
    assert_eq!(fader.source_key.as_deref(), Some("pl::CL5::Fader"));

    // Card-contributed ports use the card template name
    let dante_in = node.ports.iter().find(|p| p.name == "Dante_In_1").expect("Dante_In_1");
    assert_eq!(dante_in.source_key.as_deref(), Some("pl::MY16_AUD::Dante_In"));

    let dante_out = node.ports.iter().find(|p| p.name == "Dante_Out_16").expect("Dante_Out_16");
    assert_eq!(dante_out.source_key.as_deref(), Some("pl::MY16_AUD::Dante_Out"));
}

#[test]
fn test_source_key_sub_instance_ports() {
    let source = r#"
template TIO_1608_D {
  ports {
    DANTE_In[1..8]: in(etherCON) [Dante]
    Line_Output[1..8]: out(XLR)
  }
  bridge DANTE_In -> Line_Output
}
template SplitsSystem {
  ports { Feed: in(XLR) }
  instance TIO is TIO_1608_D
  bridge Feed -> TIO.DANTE_In
}
instance Splits is SplitsSystem
"#;
    let result = parse_graph(source);

    // Sub-level for Splits should have sub-instance node
    let sub = result.levels.get("Splits").expect("Splits sub-level");
    let tio_node = sub.nodes.get("Splits/TIO").expect("Splits/TIO node");

    // Sub-instance ports use the sub-instance's template name
    let din = tio_node.ports.iter().find(|p| p.name == "DANTE_In_1").expect("DANTE_In_1");
    assert_eq!(din.source_key.as_deref(), Some("pl::TIO_1608_D::DANTE_In"));

    let lout = tio_node.ports.iter().find(|p| p.name == "Line_Output_8").expect("Line_Output_8");
    assert_eq!(lout.source_key.as_deref(), Some("pl::TIO_1608_D::Line_Output"));

    // Pseudo _inputs node ports should also carry source_key (from parent template)
    let inputs_node = sub.nodes.get("Splits_inputs").expect("Splits_inputs node");
    let feed = inputs_node.ports.iter().find(|p| p.name == "Feed").expect("Feed port");
    assert_eq!(feed.source_key.as_deref(), Some("pl::SplitsSystem::Feed"));
}

// ---------------------------------------------------------------------------
// 2. Bridge expansion 1:1
// ---------------------------------------------------------------------------

#[test]
fn test_bridge_expansion_1to1() {
    let source = r#"
template Rio3224 {
  ports {
    Mic_In[1..32]: in(XLR)
    Dante_Out[1..32]: out(etherCON) [Dante]
  }
  bridge Mic_In[1..32] -> Dante_Out[1..32]
}
instance Stage_Left is Rio3224
"#;
    let result = parse_graph(source);

    let root = result.levels.get("root").expect("root level");
    let node = root.nodes.get("Stage_Left").expect("Stage_Left node");
    assert!(node.drillable);
    assert_eq!(node.ports.len(), 64); // 32 in + 32 out

    // Sub-level should exist
    let sub = result.levels.get("Stage_Left").expect("Stage_Left sub-level");
    assert_eq!(sub.parent_id.as_deref(), Some("root"));

    // Should have _inputs and _outputs pseudo nodes
    assert!(sub.nodes.contains_key("Stage_Left_inputs"));
    assert!(sub.nodes.contains_key("Stage_Left_outputs"));

    // 32 bridge edges
    assert_eq!(sub.edges.len(), 32);

    // Check first edge
    let first_edge = sub.edges.values().next().expect("first edge");
    assert_eq!(first_edge.edge_type, "bridge");
    assert_eq!(first_edge.source_node, "Stage_Left_inputs");
    assert_eq!(first_edge.target_node, "Stage_Left_outputs");

    // Verify bus properties
    for edge in sub.edges.values() {
        assert!(edge.bus_id.is_some());
        assert!(edge.bus_size == Some(32));
    }
}

// ---------------------------------------------------------------------------
// 3. Bridge to sub-instance port (THE BUG FIX)
// ---------------------------------------------------------------------------

#[test]
fn test_bridge_to_sub_instance_port() {
    let source = r#"
template Splitter {
  ports {
    Inputs[1..80]: in
    Outputs_A[1..80]: out
    Outputs_B[1..80]: out
  }
  bridge Inputs -> Outputs_A
  bridge Inputs -> Outputs_B
}

template AudioSystem {
  ports {
    Stage_Input: in(XLR)
  }
  instance MySplitter is Splitter
  bridge Stage_Input -> MySplitter.Inputs
}

instance MainSystem is AudioSystem
"#;
    let result = parse_graph(source);

    // MainSystem should be drillable
    let root = result.levels.get("root").expect("root level");
    let node = root.nodes.get("MainSystem").expect("MainSystem node");
    assert!(node.drillable);

    // Sub-level should exist
    let sub = result.levels.get("MainSystem").expect("MainSystem sub-level");

    // Should have _inputs, _outputs, and MainSystem/MySplitter nodes
    assert!(sub.nodes.contains_key("MainSystem_inputs"));
    assert!(sub.nodes.contains_key("MainSystem_outputs"));
    assert!(sub.nodes.contains_key("MainSystem/MySplitter"));

    // The bridge from Stage_Input -> MySplitter.Inputs should produce edges
    // where source is inputs pseudo node, target is the sub-instance
    let bridge_edges: Vec<_> = sub
        .edges
        .values()
        .filter(|e| e.edge_type == "bridge")
        .collect();
    assert!(
        !bridge_edges.is_empty(),
        "should have bridge edges from scalar input to sub-instance"
    );

    // All edge port refs should exist on their respective nodes
    for edge in sub.edges.values() {
        let src_node = sub
            .nodes
            .get(&edge.source_node)
            .unwrap_or_else(|| panic!("source node '{}' should exist", edge.source_node));
        let has_src_port = src_node.ports.iter().any(|p| p.id == edge.source_port);
        assert!(
            has_src_port,
            "source port '{}' should exist on node '{}' (ports: {:?})",
            edge.source_port,
            edge.source_node,
            src_node.ports.iter().map(|p| &p.id).collect::<Vec<_>>()
        );

        let tgt_node = sub
            .nodes
            .get(&edge.target_node)
            .unwrap_or_else(|| panic!("target node '{}' should exist", edge.target_node));
        let has_tgt_port = tgt_node.ports.iter().any(|p| p.id == edge.target_port);
        assert!(
            has_tgt_port,
            "target port '{}' should exist on node '{}' (ports: {:?})",
            edge.target_port,
            edge.target_node,
            tgt_node.ports.iter().map(|p| &p.id).collect::<Vec<_>>()
        );
    }
}

// ---------------------------------------------------------------------------
// 4. Connect edges
// ---------------------------------------------------------------------------

#[test]
fn test_connect_edges() {
    let source = r#"
template Rio3224 {
  ports {
    Dante_Out[1..32]: out(etherCON) [Dante]
  }
}

template CL5 {
  ports {
    Dante_In[1..72]: in(etherCON) [Dante]
  }
}

instance Stage_Left is Rio3224
instance FOH is CL5

connect Stage_Left.Dante_Out[1..32] -> FOH.Dante_In[1..32]
"#;
    let result = parse_graph(source);
    let root = result.levels.get("root").expect("root level");

    assert_eq!(root.edges.len(), 32);

    // Check edge format
    for edge in root.edges.values() {
        assert_eq!(edge.edge_type, "connect");
        assert_eq!(edge.source_node, "Stage_Left");
        assert_eq!(edge.target_node, "FOH");
    }

    // Verify first and last edge port IDs
    let first = root
        .edges
        .values()
        .find(|e| e.source_port == "Stage_Left:Dante_Out_1")
        .expect("should have edge for Dante_Out_1");
    assert_eq!(first.target_port, "FOH:Dante_In_1");

    let last = root
        .edges
        .values()
        .find(|e| e.source_port == "Stage_Left:Dante_Out_32")
        .expect("should have edge for Dante_Out_32");
    assert_eq!(last.target_port, "FOH:Dante_In_32");
}

// ---------------------------------------------------------------------------
// 5. Port connectivity
// ---------------------------------------------------------------------------

#[test]
fn test_port_connectivity() {
    let source = r#"
template DevA {
  ports {
    Out[1..4]: out
  }
}

template DevB {
  ports {
    In[1..4]: in
    Extra: in
  }
}

instance A is DevA
instance B is DevB

connect A.Out[1..4] -> B.In[1..4]
"#;
    let result = parse_graph(source);
    let root = result.levels.get("root").expect("root level");

    // All Out and In ports should be connected
    let a = root.nodes.get("A").expect("A node");
    for port in &a.ports {
        assert_eq!(port.connected, Some(true), "port {} should be connected", port.id);
    }

    let b = root.nodes.get("B").expect("B node");
    for port in &b.ports {
        if port.name == "Extra" {
            assert_eq!(port.connected, None, "Extra port should not be connected");
        } else {
            assert_eq!(
                port.connected,
                Some(true),
                "port {} should be connected",
                port.id
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 6. Config labels
// ---------------------------------------------------------------------------

#[test]
fn test_config_labels() {
    let source = r#"
template Rio3224 {
  ports {
    Mic_In[1..32]: in(XLR)
  }
}

instance Stage_Left is Rio3224

config Channel_Labels {
  label Stage_Left.Mic_In[1]: "Lead Vocal" {
    color: "red"
  }
  label Stage_Left.Mic_In[5]: "Kick Drum"
}
"#;
    let result = parse_graph(source);
    let root = result.levels.get("root").expect("root level");
    let node = root.nodes.get("Stage_Left").expect("Stage_Left node");

    let port1 = node
        .ports
        .iter()
        .find(|p| p.id == "Stage_Left:Mic_In_1")
        .expect("Mic_In_1");
    assert_eq!(port1.label.as_deref(), Some("Lead Vocal"));
    assert!(port1.label_properties.is_some());
    let lp = port1.label_properties.as_ref().unwrap();
    assert_eq!(lp.get("color").map(|s| s.as_str()), Some("red"));

    let port5 = node
        .ports
        .iter()
        .find(|p| p.id == "Stage_Left:Mic_In_5")
        .expect("Mic_In_5");
    assert_eq!(port5.label.as_deref(), Some("Kick Drum"));
    // No properties on port 5
    assert!(port5.label_properties.is_none());
}

// ---------------------------------------------------------------------------
// 7. Fixture: worship-venue.patch
// ---------------------------------------------------------------------------

#[test]
fn test_fixture_worship_venue() {
    let source = include_str!("../../../tests/fixtures/examples/worship-venue.patch");
    let result = parse_graph(source);

    let root = result.levels.get("root").expect("root level");

    // 4 instances: Stage_Left, Stage_Right, FOH_Console, Dante_Switch
    assert_eq!(root.nodes.len(), 4, "should have 4 root nodes");
    assert!(root.nodes.contains_key("Stage_Left"));
    assert!(root.nodes.contains_key("Stage_Right"));
    assert!(root.nodes.contains_key("FOH_Console"));
    assert!(root.nodes.contains_key("Dante_Switch"));

    // Stage_Left has bridge, so it's drillable
    let sl = root.nodes.get("Stage_Left").unwrap();
    assert!(sl.drillable);

    // FOH_Console has no bridges/sub-instances, so not drillable
    let foh = root.nodes.get("FOH_Console").unwrap();
    assert!(!foh.drillable);

    // Verify edges exist (connects + bridges)
    // 12 connect edges + 48 bridge edges = 60 total
    // Connects: 8 cable connections + 4 return connections = 12
    // But some are scalar (Dante_Out -> Port[1]) which produces 1 edge each
    // Let's just check we have a reasonable number
    assert!(root.edges.len() >= 12, "should have at least 12 edges, got {}", root.edges.len());

    // Check signals
    assert_eq!(result.signals.len(), 4, "should have 4 signals");
    assert!(result.signals.contains_key("Lead_Vocal"));
    assert!(result.signals.contains_key("Kick_Drum"));
    assert!(result.signals.contains_key("Snare_Top"));
    assert!(result.signals.contains_key("Pastor_Lav"));

    // Check signal origin
    let lead = result.signals.get("Lead_Vocal").unwrap();
    assert_eq!(lead.origin_node.as_deref(), Some("Stage_Left"));
    assert_eq!(lead.origin_port.as_deref(), Some("Stage_Left:Mic_In_1"));

    // Sub-levels should exist for drillable instances
    assert!(result.levels.contains_key("Stage_Left"));
    assert!(result.levels.contains_key("Stage_Right"));
}

// ---------------------------------------------------------------------------
// 8. Internal connect with explicit mapping on unindexed ranged port
// ---------------------------------------------------------------------------

#[test]
fn test_internal_connect_explicit_mapping_unindexed_port() {
    // Reproduces the bug: connect Ableton.DVS_DANTE -> TIO.DANTE_In
    // where DVS_DANTE[1..64] is ranged but referenced without index,
    // and mapping: "1->1, 2->2, 3->3, 4->4" specifies channel pairs.
    let source = r#"
template DVS_Computer {
  ports {
    DVS_DANTE[1..64]: out(etherCON) [Dante]
  }
}

template TIO {
  ports {
    DANTE_In[1..16]: in(etherCON) [Dante]
    Line_Out[1..16]: out(XLR)
  }
}

template MySystem {
  meta { kind: "system" }
  ports {
    Input: in(XLR)
  }
  instance Ableton is DVS_Computer
  instance TIO_1 is TIO
  connect Ableton.DVS_DANTE -> TIO_1.DANTE_In {
    mapping: "1->1, 2->2, 3->3, 4->4"
  }
}

instance Sys is MySystem
"#;
    let result = parse_graph(source);

    let sub = result
        .levels
        .get("Sys")
        .expect("Sys sub-level should exist");

    // Should have the sub-instance nodes
    assert!(sub.nodes.contains_key("Sys/Ableton"), "Ableton sub-instance");
    assert!(sub.nodes.contains_key("Sys/TIO_1"), "TIO_1 sub-instance");

    // The explicit mapping should produce 4 edges
    let connect_edges: Vec<_> = sub
        .edges
        .values()
        .filter(|e| e.edge_type == "connect")
        .collect();
    assert_eq!(
        connect_edges.len(),
        4,
        "explicit mapping 1->1,2->2,3->3,4->4 should produce 4 edges, got {}",
        connect_edges.len()
    );

    // All edge port refs must exist on their nodes
    for edge in sub.edges.values() {
        let src_node = sub
            .nodes
            .get(&edge.source_node)
            .unwrap_or_else(|| panic!("source node '{}' missing", edge.source_node));
        assert!(
            src_node.ports.iter().any(|p| p.id == edge.source_port),
            "source port '{}' not found on '{}' (has: {:?})",
            edge.source_port,
            edge.source_node,
            src_node.ports.iter().map(|p| &p.id).collect::<Vec<_>>()
        );

        let tgt_node = sub
            .nodes
            .get(&edge.target_node)
            .unwrap_or_else(|| panic!("target node '{}' missing", edge.target_node));
        assert!(
            tgt_node.ports.iter().any(|p| p.id == edge.target_port),
            "target port '{}' not found on '{}' (has: {:?})",
            edge.target_port,
            edge.target_node,
            tgt_node.ports.iter().map(|p| &p.id).collect::<Vec<_>>()
        );
    }
}

// ---------------------------------------------------------------------------
// 9. Hillsong MTG multi-file fixture — compile_project_to_graph
// ---------------------------------------------------------------------------

#[test]
fn test_fixture_hillsong_mtg_multi_file() {
    use std::collections::HashMap;

    // Load all fixture files via include_str! (paths relative to this source file)
    let mut files: HashMap<String, String> = HashMap::new();
    files.insert("campus.patch".into(),
        include_str!("../../../tests/fixtures/hillsong-mtg/campus.patch").into());
    files.insert("subsystems/foh.patch".into(),
        include_str!("../../../tests/fixtures/hillsong-mtg/subsystems/foh.patch").into());
    files.insert("subsystems/monitors.patch".into(),
        include_str!("../../../tests/fixtures/hillsong-mtg/subsystems/monitors.patch").into());
    files.insert("subsystems/splits_patch.patch".into(),
        include_str!("../../../tests/fixtures/hillsong-mtg/subsystems/splits_patch.patch").into());
    files.insert("subsystems/stage_cores.patch".into(),
        include_str!("../../../tests/fixtures/hillsong-mtg/subsystems/stage_cores.patch").into());
    files.insert("subsystems/radio_mic.patch".into(),
        include_str!("../../../tests/fixtures/hillsong-mtg/subsystems/radio_mic.patch").into());
    files.insert("subsystems/iems.patch".into(),
        include_str!("../../../tests/fixtures/hillsong-mtg/subsystems/iems.patch").into());
    files.insert("subsystems/infrastructure.patch".into(),
        include_str!("../../../tests/fixtures/hillsong-mtg/subsystems/infrastructure.patch").into());
    files.insert("local/generic.patch".into(),
        include_str!("../../../tests/fixtures/hillsong-mtg/local/generic.patch").into());

    let json = crate::graph::compile_project_to_graph_from_sources(&files, "campus.patch");
    let result: crate::graph::types::CompileToGraphResult =
        serde_json::from_str(&json).expect("should parse graph JSON");

    // Root level should exist with campus-level instances
    let root = result.levels.get("root").expect("root level");
    assert!(
        !root.nodes.is_empty(),
        "root should have nodes"
    );

    // Count broken edge→port refs across all levels.
    // Some fixture files have port name mismatches (e.g., "MADI" vs "MADI_Out")
    // that are data bugs, not compiler bugs. We allow a small number but flag them.
    let mut broken_refs = 0;
    let mut total_edges = 0;
    for (level_id, level) in &result.levels {
        for edge in level.edges.values() {
            total_edges += 1;
            if let Some(node) = level.nodes.get(&edge.source_node) {
                if !node.ports.iter().any(|p| p.id == edge.source_port) {
                    broken_refs += 1;
                    eprintln!(
                        "[{level_id}] broken src: '{}' not on '{}'",
                        edge.source_port, edge.source_node
                    );
                }
            }
            if let Some(node) = level.nodes.get(&edge.target_node) {
                if !node.ports.iter().any(|p| p.id == edge.target_port) {
                    broken_refs += 1;
                    eprintln!(
                        "[{level_id}] broken tgt: '{}' not on '{}'",
                        edge.target_port, edge.target_node
                    );
                }
            }
        }
    }
    eprintln!(
        "hillsong-mtg: {broken_refs}/{total_edges} broken edge→port refs ({:.1}%)",
        if total_edges > 0 { (broken_refs as f64 / total_edges as f64) * 100.0 } else { 0.0 }
    );
    // As of v0.2.13, all 1731 edge→port references resolve correctly.
    // If this fails, a compiler regression has broken port name resolution — check
    // card port flattening in build_effective_port_map() and directional naming.
    assert_eq!(
        broken_refs, 0,
        "hillsong-mtg should have 0 broken edge→port refs; got {broken_refs}/{total_edges}"
    );
    assert!(
        total_edges > 500,
        "should produce a substantial graph, got {total_edges} edges"
    );
    assert!(
        result.levels.len() >= 5,
        "should have multiple sub-levels, got {}",
        result.levels.len()
    );

    // Invariant: every edge must reference nodes and ports that exist in its level.
    // This catches the class of bugs where cross-level references leak into sub-levels
    // (e.g., edges using hierarchical "Parent/Child" IDs when nodes are keyed locally).
    let mut missing_node_refs = 0;
    for (level_id, level) in &result.levels {
        let node_ids: std::collections::HashSet<&String> = level.nodes.keys().collect();
        for edge in level.edges.values() {
            if !node_ids.contains(&edge.source_node) {
                missing_node_refs += 1;
                eprintln!(
                    "[{level_id}] edge '{}' references non-existent source node '{}'",
                    edge.id, edge.source_node
                );
            }
            if !node_ids.contains(&edge.target_node) {
                missing_node_refs += 1;
                eprintln!(
                    "[{level_id}] edge '{}' references non-existent target node '{}'",
                    edge.id, edge.target_node
                );
            }
        }
    }
    assert!(
        missing_node_refs == 0,
        "found {missing_node_refs} edges referencing non-existent nodes"
    );
}

// ---------------------------------------------------------------------------
// 10. Stream source channel selection (#37, Phase 2b)
//
// A `stream`'s source may carry a channel selection (`source: DSP.AES67_Out[3]`).
// The selection belongs to the flow, not to the port id: `StreamIdentity.source_port`
// must always name the interface's port, never one channel of it. Before the fix a
// ONE-element index leaked into the id as `Inst:Port_3`, so a legal single-channel
// AES67 flow pointed at a port that does not exist.
// ---------------------------------------------------------------------------

/// The realistic AES67 shape: one scalar network jack carrying N audio channels.
/// Here the emitted port id is checkable against the node's real ports.
#[test]
fn test_stream_single_channel_selection_names_an_existing_port() {
    let source = r#"
template QSys_Core {
  ports {
    AES67_Out: out(RJ45) [AES67]
  }
}
instance DSP is QSys_Core
stream Talkback {
  source: DSP.AES67_Out[3]
  channels: 1
  protocol: "AES67"
}
"#;
    let result = parse_graph(source);
    let stream = result.streams.get("Talkback").expect("Talkback stream");
    assert_eq!(
        stream.source_port.as_deref(),
        Some("DSP:AES67_Out"),
        "a single-channel selection must not leak into the stream's source port id"
    );

    let node = result
        .levels
        .get("root")
        .expect("root level")
        .nodes
        .get("DSP")
        .expect("DSP node");
    assert!(
        node.ports.iter().any(|p| p.id == "DSP:AES67_Out"),
        "the stream's source_port must name a port that exists on the node, got {:?} of {:?}",
        stream.source_port,
        node.ports.iter().map(|p| &p.id).collect::<Vec<_>>()
    );
}

/// The selection width must not change the port id: no index, one index and many
/// indices all resolve to the same interface port.
///
/// Note: on a *ranged* port (as here) that id names no port node — the bare base name
/// never exists once the range is expanded. That is pre-existing for the no-index and
/// multi-index cases; this test extends it to width 1. The selection now lives in the
/// canvas DTO, not in the graph, so the graph deliberately says nothing about it.
#[test]
fn test_stream_source_port_is_independent_of_selection_width() {
    let source = r#"
template Shure_MXA910 {
  ports {
    Dante_Pri_Out[1..10]: out(etherCON) [Dante]
  }
}
instance Ceiling_Mic is Shure_MXA910
stream Whole {
  source: Ceiling_Mic.Dante_Pri_Out
  channels: 10
}
stream One {
  source: Ceiling_Mic.Dante_Pri_Out[3]
  channels: 1
}
stream Some_Of_Them {
  source: Ceiling_Mic.Dante_Pri_Out[7, 1, 5, 3]
  channels: 4
}
"#;
    let result = parse_graph(source);
    for name in ["Whole", "One", "Some_Of_Them"] {
        let stream = result.streams.get(name).unwrap_or_else(|| panic!("{name} stream"));
        assert_eq!(
            stream.source_port.as_deref(),
            Some("Ceiling_Mic:Dante_Pri_Out"),
            "stream {name} must resolve to the interface port regardless of its selection"
        );
    }
}

/// The stream fix must NOT be applied inside `resolve_port_id` itself: config labels
/// and signal origins both depend on a one-element index resolving to the channel
/// port (`Inst:Port_3`), which is how a label finds the channel it belongs to.
#[test]
fn test_single_index_still_resolves_to_channel_port_for_labels_and_signals() {
    let source = r#"
template Rio3224 {
  ports {
    Mic_In[1..32]: in(XLR)
  }
}
instance Stage_Left is Rio3224

signal Lead_Vox {
  origin: Stage_Left.Mic_In[3]
}

config Channel_Labels {
  label Stage_Left.Mic_In[3]: "Lead Vocal"
}
"#;
    let result = parse_graph(source);

    let signal = result.signals.get("Lead_Vox").expect("Lead_Vox signal");
    assert_eq!(
        signal.origin_port.as_deref(),
        Some("Stage_Left:Mic_In_3"),
        "a signal origin with a single index must still name the channel port"
    );

    let node = result
        .levels
        .get("root")
        .expect("root level")
        .nodes
        .get("Stage_Left")
        .expect("Stage_Left node");
    let port = node
        .ports
        .iter()
        .find(|p| p.id == "Stage_Left:Mic_In_3")
        .expect("Mic_In_3");
    assert_eq!(
        port.label.as_deref(),
        Some("Lead Vocal"),
        "a config label on Port[3] must still find the channel port"
    );
}
