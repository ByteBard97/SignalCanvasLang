//! Statement parsers — additional `impl Parser` block split from parser.rs.
use crate::ast::*;
use crate::error::ParseError;
use crate::lexer::Token;
use super::Parser;

impl<'a> Parser<'a> {
    pub(super) fn parse_instance(&mut self) -> InstanceDecl {
        let start = self.current_span().start;
        self.advance(); // consume 'instance'
        let name = self.expect_identifier().unwrap_or_default();
        self.expect(&Token::Is);
        let template_name = self.expect_identifier().unwrap_or_default();

        let args = self.parse_optional_arg_list();
        let version_constraint = self.parse_optional_version();

        let mut properties = Vec::new();
        let mut routes = Vec::new();
        let mut buses = Vec::new();
        let mut slot_assignments = Vec::new();

        if self.peek() == Some(&Token::LBrace) {
            self.advance();
            while self.peek() != Some(&Token::RBrace) && !self.at_end() {
                match self.peek() {
                    Some(Token::Route) => routes.push(self.parse_route_entry()),
                    Some(Token::Bus) => buses.push(self.parse_bus_entry()),
                    Some(Token::Slot) => slot_assignments.push(self.parse_slot_assignment()),
                    _ if self.is_property_key() => {
                        properties.push(self.parse_key_value_full());
                    }
                    _ => { self.advance(); }
                }
            }
            self.expect(&Token::RBrace);
        }

        InstanceDecl {
            name,
            template_name,
            args,
            version_constraint,
            properties,
            routes,
            buses,
            slot_assignments,
            span: self.span_from(start),
        }
    }

    pub(super) fn parse_connect(&mut self) -> ConnectDecl {
        let start = self.current_span().start;
        self.advance(); // consume 'connect'
        let source = self.parse_port_ref();
        self.expect(&Token::Arrow);
        let target = self.parse_port_ref();

        let mut properties = Vec::new();
        let mut suppressions = Vec::new();
        let mut mapping = None;

        if self.peek() == Some(&Token::LBrace) {
            self.advance();
            if self.peek() == Some(&Token::Suppress) {
                suppressions = self.parse_suppress_annotation();
            }
            while self.peek() != Some(&Token::RBrace) && !self.at_end() {
                if self.is_property_key() {
                    let kv = self.parse_key_value_full();
                    if kv.key == "mapping" {
                        if let KvValue::Str { ref value } = kv.value {
                            mapping = Some(value.clone());
                        }
                    } else {
                        properties.push(kv);
                    }
                } else {
                    self.advance();
                }
            }
            self.expect(&Token::RBrace);
        }

        ConnectDecl { source, target, properties, suppressions, mapping, span: self.span_from(start) }
    }

    pub(super) fn parse_bridge(&mut self) -> BridgeDecl {
        let start = self.current_span().start;
        self.advance(); // consume 'bridge'
        let source = self.parse_port_ref();
        self.expect(&Token::Arrow);
        let target = self.parse_port_ref();
        BridgeDecl {
            source,
            target,
            span: self.span_from(start),
        }
    }

    /// Parse `bridge_group Target.Port { Source1.Port Source2.Port ... }`
    pub(super) fn parse_bridge_group(&mut self) -> BridgeGroupDecl {
        let start = self.current_span().start;
        self.advance(); // consume 'bridge_group'
        let target = self.parse_port_ref();
        let mut sources = Vec::new();
        if self.expect(&Token::LBrace) {
            while !self.at_end() && self.peek() != Some(&Token::RBrace) {
                sources.push(self.parse_port_ref());
            }
            self.expect(&Token::RBrace);
        }
        BridgeGroupDecl {
            target,
            sources,
            span: self.span_from(start),
        }
    }

    /// Parse `link_group Name { connect A -> B  key: "value" ... }`
    pub(super) fn parse_link_group(&mut self) -> LinkGroupDecl {
        let start = self.current_span().start;
        self.advance(); // consume 'link_group'
        let name = self.expect_identifier().unwrap_or_default();
        let mut connects = Vec::new();
        let mut properties = Vec::new();
        if self.expect(&Token::LBrace) {
            while !self.at_end() && self.peek() != Some(&Token::RBrace) {
                if self.peek() == Some(&Token::Connect) {
                    connects.push(self.parse_connect());
                } else if self.is_property_key() {
                    properties.push(self.parse_key_value_full());
                } else {
                    self.advance();
                }
            }
            self.expect(&Token::RBrace);
        }
        LinkGroupDecl {
            name,
            connects,
            properties,
            span: self.span_from(start),
        }
    }

    pub(super) fn parse_signal(&mut self) -> SignalDecl {
        let start = self.current_span().start;
        self.advance(); // consume 'signal'
        let name = self.expect_identifier().unwrap_or_default();
        let (properties, origin) = self.parse_body_with_port_ref_key("origin");
        SignalDecl { name, properties, origin, span: self.span_from(start) }
    }

    pub(super) fn parse_flag(&mut self) -> FlagDecl {
        let start = self.current_span().start;
        self.advance(); // consume 'flag'
        let name = self.expect_identifier().unwrap_or_default();
        let properties = self.parse_optional_kv_body();
        FlagDecl { name, properties, span: self.span_from(start) }
    }

    pub(super) fn parse_stream(&mut self) -> StreamDecl {
        let start = self.current_span().start;
        self.advance(); // consume 'stream'
        let name = self.expect_identifier().unwrap_or_default();
        let (properties, source) = self.parse_body_with_port_ref_key("source");
        StreamDecl { name, properties, source, span: self.span_from(start) }
    }

    pub(super) fn parse_config(&mut self) -> ConfigDecl {
        let start = self.current_span().start;
        self.advance(); // consume 'config'
        let name = self.expect_identifier().unwrap_or_default();
        let mut labels = Vec::new();
        if self.peek() == Some(&Token::LBrace) {
            self.advance();
            while self.peek() != Some(&Token::RBrace) && !self.at_end() {
                if self.peek() == Some(&Token::Label) {
                    labels.push(self.parse_config_label());
                } else {
                    self.advance();
                }
            }
            self.expect(&Token::RBrace);
        }
        ConfigDecl { name, labels, span: self.span_from(start) }
    }

    /// Parse `use ns.sub { T1, T2 }` or `use ns.sub.*` or `use ns`
    pub(super) fn parse_use(&mut self) -> UseDecl {
        let start = self.current_span().start;
        self.advance(); // consume 'use'

        // Collect dotted namespace parts, stopping at '*' or '{'
        let mut parts = Vec::new();
        if let Some(first) = self.expect_identifier() {
            parts.push(first);
        }
        let mut wildcard = false;
        while self.peek() == Some(&Token::Dot) {
            self.advance(); // consume '.'
            if self.peek() == Some(&Token::Star) {
                self.advance(); // consume '*'
                wildcard = true;
                break;
            }
            if let Some(ident) = self.expect_identifier() {
                parts.push(ident);
            } else {
                break;
            }
        }

        let namespace = parts.join(".");

        // If not wildcard, check for optional braced template list
        let mut templates = Vec::new();
        if !wildcard && self.peek() == Some(&Token::LBrace) {
            self.advance(); // consume '{'
            // Parse comma-separated identifiers
            while !self.at_end() && self.peek() != Some(&Token::RBrace) {
                if let Some(tmpl) = self.expect_identifier() {
                    templates.push(tmpl);
                }
                if self.peek() == Some(&Token::Comma) {
                    self.advance();
                } else {
                    break;
                }
            }
            self.expect(&Token::RBrace);
        }

        UseDecl {
            namespace,
            templates,
            wildcard,
            span: self.span_from(start),
        }
    }

    // ── Ring ────────────────────────────────────────────────

    /// Parse `ring Name { protocol: "OptoCore"  member Console  member Rack.Port_B }`
    pub(super) fn parse_ring(&mut self) -> RingDecl {
        let start = self.current_span().start;
        self.advance(); // consume 'ring'
        let name = self.expect_identifier().unwrap_or_default();

        let mut properties = Vec::new();
        let mut members = Vec::new();

        if self.expect(&Token::LBrace) {
            while !self.at_end() && self.peek() != Some(&Token::RBrace) {
                if self.peek() == Some(&Token::Member) {
                    members.push(self.parse_ring_member());
                } else if self.is_property_key() {
                    properties.push(self.parse_key_value_full());
                } else {
                    self.advance(); // skip unknown token, avoid infinite loop
                }
            }
            self.expect(&Token::RBrace);
        }

        RingDecl {
            name,
            properties,
            members,
            span: self.span_from(start),
        }
    }

    /// Parse `member InstanceName` or `member InstanceName.PortName`
    pub(super) fn parse_ring_member(&mut self) -> RingMember {
        let start = self.current_span().start;
        self.advance(); // consume 'member'
        let instance_name = self.expect_identifier().unwrap_or_default();
        let port_name = if self.peek() == Some(&Token::Dot) {
            self.advance(); // consume '.'
            Some(self.expect_identifier().unwrap_or_default())
        } else {
            None
        };
        RingMember {
            instance_name,
            port_name,
            span: self.span_from(start),
        }
    }

    // ── Network ─────────────────────────────────────────────

    /// Parse `network Name { protocol: "Dante"  member Inst  member Inst.PortGroup  member Inst.slot[N] }`
    pub(super) fn parse_network(&mut self) -> NetworkDecl {
        let start = self.current_span().start;
        self.advance(); // consume 'network'
        let name = self.expect_identifier().unwrap_or_default();

        let mut properties = Vec::new();
        let mut members = Vec::new();

        if self.expect(&Token::LBrace) {
            while !self.at_end() && self.peek() != Some(&Token::RBrace) {
                if self.peek() == Some(&Token::Member) {
                    members.push(self.parse_network_member());
                } else if self.is_property_key() {
                    properties.push(self.parse_key_value_full());
                } else {
                    self.advance(); // skip unknown token, avoid infinite loop
                }
            }
            self.expect(&Token::RBrace);
        }

        NetworkDecl {
            name,
            properties,
            members,
            span: self.span_from(start),
        }
    }

    /// Parse `member Inst`, `member Inst.PortGroup`, or `member Inst.slot[N]`
    pub(super) fn parse_network_member(&mut self) -> NetworkMember {
        let start = self.current_span().start;
        self.advance(); // consume 'member'
        let instance = self.expect_identifier().unwrap_or_default();

        if self.peek() == Some(&Token::Dot) {
            self.advance(); // consume '.'

            if self.peek() == Some(&Token::Slot) {
                self.advance(); // consume 'slot'
                self.expect(&Token::LBracket);
                let index = if let Some(&Token::Number(n)) = self.peek() {
                    self.advance();
                    n
                } else {
                    let span = self.current_span();
                    self.errors.push(ParseError {
                        message: "expected slot index number".to_string(),
                        span,
                        hint: Some("Use: member Instance.slot[1]".to_string()),
                    });
                    0
                };
                self.expect(&Token::RBracket);
                NetworkMember::SlotRef {
                    instance,
                    index,
                    span: self.span_from(start),
                }
            } else {
                let port_group = self.expect_identifier().unwrap_or_default();
                NetworkMember::PortGroup {
                    instance,
                    port_group,
                    span: self.span_from(start),
                }
            }
        } else {
            NetworkMember::DeviceLevel {
                instance,
                span: self.span_from(start),
            }
        }
    }
}
