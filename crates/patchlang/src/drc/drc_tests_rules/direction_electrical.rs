#[cfg(test)]
mod direction {
    use crate::builder::LibraryContext;
    use crate::drc::{self, DRCLayer, Severity};
    use crate::parser::parse;

    fn check(source: &str) -> Vec<crate::drc::Diagnostic> {
        let result = parse(source);
        drc::run_all(&result.program, &LibraryContext::empty())
    }

    const DEVICE_HEADER: &str = "
        template T {
          ports {
            In[1..4]: in(XLR)
            Out[1..4]: out(XLR)
            BiDir: io(etherCON)
          }
        }
        instance A is T
        instance B is T
    ";

    #[test]
    fn d01_output_to_output_is_error() {
        let src = format!("{DEVICE_HEADER}\nconnect A.Out[1] -> B.Out[1]");
        let diags = check(&src);
        assert!(diags.iter().any(|d| {
            d.layer == DRCLayer::Direction
                && d.severity == Severity::Error
                && d.message.contains("output to output")
        }));
    }

    #[test]
    fn d02_input_to_input_is_error() {
        let src = format!("{DEVICE_HEADER}\nconnect A.In[1] -> B.In[1]");
        let diags = check(&src);
        assert!(diags.iter().any(|d| {
            d.layer == DRCLayer::Direction
                && d.severity == Severity::Error
                && d.message.contains("input to input")
        }));
    }

    #[test]
    fn valid_out_to_in_no_diagnostic() {
        let src = format!("{DEVICE_HEADER}\nconnect A.Out[1] -> B.In[1]");
        let diags = check(&src);
        assert!(!diags.iter().any(|d| d.layer == DRCLayer::Direction));
    }

    #[test]
    fn io_port_always_skipped() {
        let src = format!("{DEVICE_HEADER}\nconnect A.BiDir -> B.BiDir");
        let diags = check(&src);
        assert!(!diags.iter().any(|d| d.layer == DRCLayer::Direction));
    }

    #[test]
    fn suppress_direction_skips_check() {
        let src = format!(
            "{DEVICE_HEADER}\nconnect A.Out[1] -> B.Out[1] {{ @suppress(direction) }}"
        );
        let diags = check(&src);
        assert!(!diags.iter().any(|d| d.layer == DRCLayer::Direction));
    }

    #[test]
    fn ranged_connection_checks_each_pair() {
        let src = format!("{DEVICE_HEADER}\nconnect A.Out[1..4] -> B.Out[1..4]");
        let diags = check(&src);
        let dir_errors: Vec<_> = diags
            .iter()
            .filter(|d| d.layer == DRCLayer::Direction)
            .collect();
        assert_eq!(dir_errors.len(), 4);
    }

    #[test]
    fn direction_check_inside_link_group() {
        let src = format!(
            "{DEVICE_HEADER}\nlink_group Cam1 {{\n  connect A.Out[1] -> B.Out[1]\n}}"
        );
        let diags = check(&src);
        assert!(diags.iter().any(|d| d.layer == DRCLayer::Direction));
    }
}

#[cfg(test)]
mod electrical {
    use crate::builder::LibraryContext;
    use crate::drc::{self, DRCLayer, Severity};
    use crate::parser::parse;

    fn check(source: &str) -> Vec<crate::drc::Diagnostic> {
        drc::run_all(&parse(source).program, &LibraryContext::empty())
    }

    const HDR: &str = "
        template Mic    { ports { Out: out(XLR) [mic_level] } }
        template Line   { ports { Out: out(XLR) [line_level] In: in(XLR) [line_level] } }
        template Speaker{ ports { Out: out(SpeakON) [speaker_level] In: in(SpeakON) [speaker_level] } }
        template Digital{ ports { Out: out(etherCON) [digital] In: in(etherCON) [digital] } }
        instance M is Mic
        instance L is Line
        instance S is Speaker
        instance D is Digital
    ";

    #[test]
    fn e01_speaker_to_line_is_error() {
        let src = format!("{HDR}\nconnect S.Out -> L.In");
        let diags = check(&src);
        assert!(diags.iter().any(|d| {
            d.layer == DRCLayer::Electrical && d.severity == Severity::Error
        }));
    }

    #[test]
    fn e02_line_to_mic_is_warning() {
        let src = "template Src { ports { Out: out(XLR) [line_level] } }
             template Tgt { ports { In: in(XLR) [mic_level] } }
             instance A is Src  instance B is Tgt
             connect A.Out -> B.In";
        let diags = check(src);
        assert!(diags.iter().any(|d| {
            d.layer == DRCLayer::Electrical && d.severity == Severity::Warning
        }));
    }

    #[test]
    fn same_level_no_diagnostic() {
        let src = format!("{HDR}\nconnect L.Out -> L.In");
        let diags = check(&src);
        assert!(!diags.iter().any(|d| d.layer == DRCLayer::Electrical));
    }

    #[test]
    fn lower_source_to_higher_target_safe() {
        let src = "template Src { ports { Out: out(XLR) [mic_level] } }
             template Tgt { ports { In: in(XLR) [line_level] } }
             instance A is Src  instance B is Tgt
             connect A.Out -> B.In";
        let diags = check(src);
        assert!(!diags.iter().any(|d| d.layer == DRCLayer::Electrical));
    }

    #[test]
    fn digital_domain_skipped() {
        let src = format!("{HDR}\nconnect D.Out -> D.In");
        let diags = check(&src);
        assert!(!diags.iter().any(|d| d.layer == DRCLayer::Electrical));
    }

    #[test]
    fn no_level_tag_skipped() {
        let src = "template A { ports { Out: out(XLR) } } template B { ports { In: in(XLR) } }
                   instance X is A  instance Y is B  connect X.Out -> Y.In";
        let diags = check(src);
        assert!(!diags.iter().any(|d| d.layer == DRCLayer::Electrical));
    }

    #[test]
    fn suppress_electrical_skips_check() {
        let src = format!("{HDR}\nconnect S.Out -> L.In {{ @suppress(electrical) }}");
        let diags = check(&src);
        assert!(!diags.iter().any(|d| d.layer == DRCLayer::Electrical));
    }
}
