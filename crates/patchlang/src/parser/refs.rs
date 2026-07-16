//! Key-value / port-ref / index parsers — split from parser.rs.
use crate::ast::*;
use crate::error::ParseError;
use crate::lexer::Token;
use super::Parser;

impl<'a> Parser<'a> {
    // ── Key-value pairs ──────────────────────────────────────

    /// Try to consume a token usable as a property key.
    /// Accepts identifiers and keyword tokens commonly used as property names
    /// (label, stream, route, bus, routing, config).
    pub(crate) fn try_consume_property_key(&mut self) -> Option<String> {
        match self.peek() {
            Some(Token::Identifier(_)) => self.expect_identifier(),
            Some(Token::Label) => { self.advance(); Some("label".to_string()) }
            Some(Token::Stream) => { self.advance(); Some("stream".to_string()) }
            Some(Token::Route) => { self.advance(); Some("route".to_string()) }
            Some(Token::Bus) => { self.advance(); Some("bus".to_string()) }
            Some(Token::Routing) => { self.advance(); Some("routing".to_string()) }
            Some(Token::Config) => { self.advance(); Some("config".to_string()) }
            _ => {
                let span = self.current_span();
                self.errors.push(ParseError {
                    message: "expected property key".to_string(),
                    span,
                    hint: None,
                });
                None
            }
        }
    }

    // ── Port references ─────────────────────────────────────

    pub(crate) fn parse_port_ref(&mut self) -> PortRef {
        let first = self.expect_identifier().unwrap_or_default();

        // Check for Instance.Port
        if self.peek() == Some(&Token::Dot) {
            self.advance(); // consume '.'
            let port = self.expect_identifier().unwrap_or_default();
            let index = self.parse_optional_index();
            PortRef {
                instance: Some(first),
                port,
                index,
            }
        } else {
            let index = self.parse_optional_index();
            PortRef {
                instance: None,
                port: first,
                index,
            }
        }
    }

    pub(crate) fn parse_optional_index(&mut self) -> Option<IndexSpec> {
        if self.peek() != Some(&Token::LBracket) {
            return None;
        }
        self.advance(); // consume '['

        let mut elements = Vec::new();

        loop {
            if self.peek() == Some(&Token::RBracket) || self.at_end() {
                break;
            }
            // Check for contextual 'auto' keyword
            if let Some(Token::Identifier(ident)) = self.peek() {
                if ident == "auto" {
                    let span = self.current_span();
                    self.advance();
                    // auto must be sole element — reject if preceded by numbers or followed by comma
                    if !elements.is_empty() {
                        self.errors.push(ParseError {
                            message: "[auto] must be the sole index element — cannot mix with numeric indices".into(),
                            span,
                            hint: Some("Remove other elements or replace [auto] with explicit indices".into()),
                        });
                        break;
                    }
                    elements.push(IndexElement::Auto);
                    if self.peek() == Some(&Token::Comma) {
                        let span = self.current_span();
                        self.errors.push(ParseError {
                            message: "[auto] must be the sole index element — cannot mix with numeric indices".into(),
                            span,
                            hint: Some("Remove other elements or replace [auto] with explicit indices".into()),
                        });
                    }
                    break;
                }
            }
            if let Some(Token::Number(n)) = self.peek().cloned() {
                self.advance();
                if self.peek() == Some(&Token::DotDot) {
                    self.advance(); // consume '..'
                    if let Some(Token::Number(end)) = self.peek().cloned() {
                        self.advance();
                        elements.push(IndexElement::Range { start: n, end });
                    }
                } else {
                    elements.push(IndexElement::Single { value: n });
                }
            } else {
                break;
            }
            if self.peek() == Some(&Token::Comma) {
                self.advance();
            } else {
                break;
            }
        }
        self.expect(&Token::RBracket);
        Some(IndexSpec { elements })
    }
}
