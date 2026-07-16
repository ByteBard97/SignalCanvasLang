#[cfg(test)]
mod convention_c05 {
    use crate::builder::LibraryContext;
    use crate::drc::{self, DRCLayer, Severity};
    use crate::parser::parse;

    fn check(source: &str) -> Vec<crate::drc::Diagnostic> {
        let result = parse(source);
        drc::run_all(&result.program, &LibraryContext::empty())
    }

    fn convention_diags(source: &str) -> Vec<crate::drc::Diagnostic> {
        check(source)
            .into_iter()
            .filter(|d| d.layer == DRCLayer::Convention)
            .collect()
    }

    #[test]
    fn c05_redundant_cable_to_aes67_device_emits_info() {
        let diags = convention_diags(r#"
            template T { ports { Out: out(etherCON) [Dante] In: in(etherCON) [Dante] } }
            instance A is T { aes67_mode: true }
            instance B is T
            connect A.Out -> B.In { redundant_cable: "Cat6a_Sec" }
        "#);
        assert!(diags.iter().any(|d| {
            d.severity == Severity::Info
                && d.message.contains("AES67")
                && d.message.contains("Redundancy terminates")
        }), "expected C05 info for redundant cable to AES67 device: {:?}", diags);
    }

    #[test]
    fn c05_redundant_cable_no_aes67_no_warning() {
        let diags = convention_diags(r#"
            template T { ports { Out: out(etherCON) [Dante] In: in(etherCON) [Dante] } }
            instance A is T
            instance B is T
            connect A.Out -> B.In { redundant_cable: "Cat6a_Sec" }
        "#);
        assert!(!diags.iter().any(|d| d.message.contains("AES67") && d.message.contains("Redundancy")),
            "non-AES67 redundant cable should not warn: {:?}", diags);
    }
}

#[cfg(test)]
mod trace {
    use crate::builder::LibraryContext;
    use crate::drc::{self, DRCLayer, Severity};
    use crate::parser::parse;

    fn check(source: &str) -> Vec<crate::drc::Diagnostic> {
        let result = parse(source);
        drc::run_all(&result.program, &LibraryContext::empty())
    }

    fn trace_diags(source: &str) -> Vec<crate::drc::Diagnostic> {
        check(source)
            .into_iter()
            .filter(|d| d.layer == DRCLayer::Trace)
            .collect()
    }

    // --- T01: origin not connected ---

    #[test]
    fn t01_origin_with_no_outgoing_edges_warns() {
        // Signal origin points to an Out port but nothing connects from it.
        let diags = trace_diags(r#"
            template Cam { ports { SDI_Out: out(BNC_75) } }
            instance MyCam is Cam
            signal ISO_Feed {
              origin: MyCam.SDI_Out
            }
        "#);
        assert!(
            diags.iter().any(|d| d.severity == Severity::Warning && d.message.contains("ISO_Feed")),
            "expected T01 warning for disconnected origin: {:?}", diags
        );
    }

    #[test]
    fn t01_origin_in_port_no_outgoing_edges_warns() {
        // Origin is an In port with no template bridge and no outgoing connects/bridges.
        let diags = trace_diags(r#"
            template StageBx { ports { Mic_In: in(XLR) Dante_Out: out(etherCON) } }
            instance Stage is StageBx
            signal Lead_Vocal {
              origin: Stage.Mic_In
            }
        "#);
        assert!(
            diags.iter().any(|d| d.severity == Severity::Warning && d.message.contains("Lead_Vocal")),
            "expected T01 warning for In port with no bridge/connect: {:?}", diags
        );
    }

    // --- T02: no output port reachable ---

    #[test]
    fn t02_origin_only_reaches_in_ports_warns() {
        // Signal has a connect but all reachable ports are In direction.
        let diags = trace_diags(r#"
            template A { ports { Monitor_In: in Aux_In: in } }
            instance DevA is A
            signal Weird {
              origin: DevA.Monitor_In
            }
            bridge DevA.Monitor_In -> DevA.Aux_In
        "#);
        assert!(
            diags.iter().any(|d| d.severity == Severity::Warning && d.message.contains("Weird")),
            "expected T02 warning when only In ports reachable: {:?}", diags
        );
    }

    // --- Valid signals: no Trace diagnostics ---

    #[test]
    fn valid_signal_with_connect_from_out_port_no_diag() {
        // Origin is an Out port with a connect — signal flows to another device.
        let diags = trace_diags(r#"
            template Cam { ports { SDI_Out: out(BNC_75) } }
            template Rtr { ports { SDI_In: in(BNC_75) SDI_Out: out(BNC_75) } }
            instance MyCam is Cam
            instance MyRouter is Rtr
            signal ISO_Feed {
              origin: MyCam.SDI_Out
            }
            connect MyCam.SDI_Out -> MyRouter.SDI_In
        "#);
        assert!(
            diags.is_empty(),
            "valid signal with connect should not trigger Trace diags: {:?}", diags
        );
    }

    #[test]
    fn valid_signal_with_template_bridge_reaches_out_port_no_diag() {
        // Origin is an In port but template has a bridge to an Out port.
        let diags = trace_diags(r#"
            template StageBx {
              ports { Mic_In: in(XLR) Dante_Out: out(etherCON) }
              bridge Mic_In -> Dante_Out
            }
            template Console { ports { Dante_In: in(etherCON) } }
            instance Stage is StageBx
            instance FOH is Console
            signal Lead_Vocal {
              origin: Stage.Mic_In
            }
            bridge Stage.Mic_In -> FOH.Dante_In
        "#);
        assert!(
            diags.is_empty(),
            "In-port origin with template bridge to Out should not warn: {:?}", diags
        );
    }

    #[test]
    fn valid_signal_with_top_level_bridge_to_out_port_no_diag() {
        // Origin reaches an Out port through the target instance's template bridge.
        let diags = trace_diags(r#"
            template StageBx {
              ports { Mic_In: in(XLR) Dante_Out: out(etherCON) }
              bridge Mic_In -> Dante_Out
            }
            template Console { ports { Dante_In: in(etherCON) Dante_Out: out(etherCON) } }
            instance Stage is StageBx
            instance FOH is Console
            signal Vocal {
              origin: Stage.Mic_In
            }
            connect Stage.Dante_Out -> FOH.Dante_In
        "#);
        assert!(
            diags.is_empty(),
            "origin with connect via template bridge to Out should not warn: {:?}", diags
        );
    }

    #[test]
    fn signal_without_origin_no_diag() {
        // Signal with no origin field is purely documentary — no Trace check.
        let diags = trace_diags(r#"
            template T { ports { Out: out } }
            instance D is T
            signal PGM_Output {
              description: "Main program output"
            }
        "#);
        assert!(
            diags.is_empty(),
            "signal without origin should not trigger Trace diags: {:?}", diags
        );
    }

    #[test]
    fn signal_with_unknown_origin_instance_no_double_report() {
        // S08 fires for the unknown instance — Trace rules should not add a second warning.
        let diags = check(r#"
            signal Broken {
              origin: GhostDevice.Out
            }
        "#);
        let trace_count = diags.iter().filter(|d| d.layer == DRCLayer::Trace).count();
        assert_eq!(
            trace_count, 0,
            "unknown instance should not generate Trace diags (S08 already fires): {:?}", diags
        );
    }

    #[test]
    fn valid_io_port_reachable_no_diag() {
        // Trace reaches an Io port — counts as output, no T02.
        let diags = trace_diags(r#"
            template Switch { ports { Port: io(RJ45) } }
            template Src { ports { Out: out(etherCON) } }
            instance MySrc is Src
            instance MySw is Switch
            signal NetFeed {
              origin: MySrc.Out
            }
            connect MySrc.Out -> MySw.Port
        "#);
        assert!(
            diags.is_empty(),
            "reaching an Io port should satisfy T02: {:?}", diags
        );
    }
}
