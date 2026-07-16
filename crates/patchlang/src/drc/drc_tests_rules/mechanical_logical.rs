#[cfg(test)]
mod mechanical {
    use crate::builder::LibraryContext;
    use crate::drc::{self, DRCLayer, Severity};
    use crate::parser::parse;

    fn check(source: &str) -> Vec<crate::drc::Diagnostic> {
        drc::run_all(&parse(source).program, &LibraryContext::empty())
    }

    const HDR: &str = "
        template A { ports { Out: out(XLR) } }
        template B { ports { In: in(BNC_75) } }
        template C { ports { In: in(XLR) } }
        template V { ports { Out: out(virtual) In: in(virtual) } }
        instance X is A
        instance Y is B
        instance Z is C
        instance W is V
    ";

    #[test]
    fn m01_xlr_to_bnc_is_error() {
        let src = format!("{HDR}\nconnect X.Out -> Y.In");
        let diags = check(&src);
        assert!(diags.iter().any(|d| {
            d.layer == DRCLayer::Mechanical
                && d.severity == Severity::Error
                && d.message.contains("XLR")
                && d.message.contains("BNC_75")
        }));
    }

    #[test]
    fn m01_same_connector_no_diagnostic() {
        let src = format!("{HDR}\nconnect X.Out -> Z.In");
        let diags = check(&src);
        assert!(!diags.iter().any(|d| d.layer == DRCLayer::Mechanical));
    }

    #[test]
    fn m01_virtual_ports_skipped() {
        let src = format!("{HDR}\nconnect W.Out -> W.In");
        let diags = check(&src);
        assert!(!diags.iter().any(|d| d.layer == DRCLayer::Mechanical));
    }

    #[test]
    fn m01_suppress_mechanical_skips_check() {
        let src = format!("{HDR}\nconnect X.Out -> Y.In {{ @suppress(mechanical) }}");
        let diags = check(&src);
        assert!(!diags.iter().any(|d| d.layer == DRCLayer::Mechanical));
    }

    #[test]
    fn m01_no_connector_skipped() {
        let src = "template A { ports { Out: out } } template B { ports { In: in } }
                   instance X is A  instance Y is B  connect X.Out -> Y.In";
        let diags = check(src);
        assert!(!diags.iter().any(|d| d.layer == DRCLayer::Mechanical));
    }

    #[test]
    fn m01_opticalcon_duo_to_quad_is_error() {
        // DUO (2-fiber) and QUAD (4-fiber) are distinct Neutrik housings — cannot mate.
        let src = "template D { ports { Out: out(opticalCON_DUO) } }
                   template Q { ports { In: in(opticalCON_QUAD) } }
                   instance X is D  instance Y is Q  connect X.Out -> Y.In";
        let diags = check(src);
        assert!(diags.iter().any(|d| {
            d.layer == DRCLayer::Mechanical && d.severity == Severity::Error
        }));
    }

    #[test]
    fn m01_opticalcon_duo_to_generic_is_clean() {
        // The specific DUO/QUAD housings mate the generic `opticalCON` family connector.
        let src = "template D { ports { Out: out(opticalCON_DUO) } }
                   template G { ports { In: in(opticalCON) } }
                   instance X is D  instance Y is G  connect X.Out -> Y.In";
        let diags = check(src);
        assert!(!diags.iter().any(|d| d.layer == DRCLayer::Mechanical));
    }
}

#[cfg(test)]
mod logical {
    use crate::builder::LibraryContext;
    use crate::drc::{self, DRCLayer, Severity};
    use crate::parser::parse;

    fn check(source: &str) -> Vec<crate::drc::Diagnostic> {
        drc::run_all(&parse(source).program, &LibraryContext::empty())
    }

    const HDR: &str = "
        template Dante { ports { Out: out(etherCON) [Dante] In: in(etherCON) [Dante] } }
        template MADI  { ports { Out: out(BNC_75) [MADI]   In: in(BNC_75) [MADI] } }
        template AES67 { ports { Out: out(etherCON) [AES67] In: in(etherCON) [AES67] } }
        instance D is Dante
        instance M is MADI
        instance A is AES67
    ";

    #[test]
    fn l01_dante_to_madi_is_error() {
        let src = format!("{HDR}\nconnect D.Out -> M.In");
        let diags = check(&src);
        assert!(diags.iter().any(|d| {
            d.layer == DRCLayer::Logical
                && d.severity == Severity::Error
                && d.message.contains("Dante")
                && d.message.contains("MADI")
        }));
    }

    #[test]
    fn dante_aes67_compatible() {
        let src = format!("{HDR}\nconnect D.Out -> A.In");
        let diags = check(&src);
        assert!(!diags.iter().any(|d| d.layer == DRCLayer::Logical));
    }

    #[test]
    fn same_protocol_no_diagnostic() {
        let src = format!("{HDR}\nconnect D.Out -> D.In");
        let diags = check(&src);
        assert!(!diags.iter().any(|d| d.layer == DRCLayer::Logical));
    }

    #[test]
    fn suppress_logical_skips_check() {
        let src = format!("{HDR}\nconnect D.Out -> M.In {{ @suppress(logical) }}");
        let diags = check(&src);
        assert!(!diags.iter().any(|d| d.layer == DRCLayer::Logical));
    }

    #[test]
    fn no_protocol_tag_skipped() {
        let src = "template A { ports { Out: out(etherCON) } } template B { ports { In: in(etherCON) } }
                   instance X is A  instance Y is B  connect X.Out -> Y.In";
        let diags = check(src);
        assert!(!diags.iter().any(|d| d.layer == DRCLayer::Logical));
    }
}
