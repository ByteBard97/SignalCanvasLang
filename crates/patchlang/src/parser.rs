use crate::ast::*;
use crate::error::{ParseError, ParseResult, Span};
use crate::lexer::{SpannedToken, Token, tokenize};
use crate::template_parser::TemplateParserExt;

/// Parse PatchLang source text into a program with errors.
pub fn parse(source: &str) -> ParseResult {
    let (tokens, mut errors) = tokenize(source);
    let mut parser = Parser::new(&tokens, source);
    let program = parser.parse_program();
    errors.extend(parser.errors);
    ParseResult { program, errors }
}

/// Top-level keywords that start a new statement — used for error recovery.
const RECOVERY_TOKENS: &[Token] = &[
    Token::Template,
    Token::Instance,
    Token::Connect,
    Token::Bridge,
    Token::BridgeGroup,
    Token::LinkGroup,
    Token::Signal,
    Token::Flag,
    Token::Stream,
    Token::Config,
    Token::Use,
    Token::Ring,
    Token::Network,
];

pub(crate) struct Parser<'a> {
    tokens: &'a [SpannedToken],
    pos: usize,
    source: &'a str,
    pub(crate) errors: Vec<ParseError>,
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [SpannedToken], source: &'a str) -> Self {
        Self {
            tokens,
            pos: 0,
            source,
            errors: Vec::new(),
        }
    }

    // ── Helpers ──────────────────────────────────────────────

    pub(crate) fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos).map(|t| &t.token)
    }

    pub(crate) fn at_end(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    pub(crate) fn advance(&mut self) -> &SpannedToken {
        let t = &self.tokens[self.pos];
        self.pos += 1;
        t
    }

    pub(crate) fn current_span(&self) -> Span {
        if let Some(t) = self.tokens.get(self.pos) {
            Span {
                start: t.span.start,
                end: t.span.end,
                file: None,
            }
        } else {
            let end = self.source.len();
            Span { start: end, end, file: None }
        }
    }

    pub(crate) fn span_from(&self, start: usize) -> Span {
        let end = if self.pos > 0 {
            self.tokens[self.pos - 1].span.end
        } else {
            start
        };
        Span { start, end, file: None }
    }

    pub(crate) fn expect(&mut self, expected: &Token) -> bool {
        if self.peek() == Some(expected) {
            self.advance();
            true
        } else {
            let span = self.current_span();
            self.errors.push(ParseError {
                message: format!("expected {expected:?}"),
                span,
                hint: None,
            });
            false
        }
    }

    pub(crate) fn expect_identifier(&mut self) -> Option<String> {
        match self.peek().cloned() {
            Some(Token::Identifier(name)) => {
                self.advance();
                Some(name)
            }
            _ => {
                let span = self.current_span();
                self.errors.push(ParseError {
                    message: "expected identifier".to_string(),
                    span,
                    hint: None,
                });
                None
            }
        }
    }

    /// Check if the current token can serve as a property key (identifier or keyword).
    pub(crate) fn is_property_key(&self) -> bool {
        matches!(
            self.peek(),
            Some(Token::Identifier(_))
                | Some(Token::Label)
                | Some(Token::Stream)
                | Some(Token::Route)
                | Some(Token::Bus)
                | Some(Token::Routing)
                | Some(Token::Config)
        )
    }

    fn is_recovery_token(&self) -> bool {
        match self.peek() {
            Some(t) => RECOVERY_TOKENS.iter().any(|r| std::mem::discriminant(r) == std::mem::discriminant(t)),
            None => true, // EOF is also a recovery point
        }
    }

    /// Skip tokens until we find a recovery point (top-level keyword or EOF).
    fn recover(&mut self) -> Span {
        let start = self.current_span().start;
        while !self.at_end() && !self.is_recovery_token() {
            self.advance();
        }
        self.span_from(start)
    }

    // ── Program ─────────────────────────────────────────────

    fn parse_program(&mut self) -> PatchProgram {
        let mut statements = Vec::new();
        while !self.at_end() {
            match self.parse_statement() {
                Some(stmt) => statements.push(stmt),
                None => {
                    let span = self.recover();
                    self.errors.push(ParseError {
                        message: "unexpected token, expected a statement".to_string(),
                        span: span.clone(),
                        hint: Some("statements start with: template, instance, connect, bridge, signal, flag, stream, config, use, ring, network".to_string()),
                    });
                    statements.push(Statement::Error(span));
                }
            }
        }
        PatchProgram { statements }
    }

    fn parse_statement(&mut self) -> Option<Statement> {
        match self.peek()? {
            Token::Template => Some(Statement::Template(self.parse_template_decl())),
            Token::Instance => Some(Statement::Instance(self.parse_instance())),
            Token::Connect => Some(Statement::Connect(self.parse_connect())),
            Token::Bridge => Some(Statement::Bridge(self.parse_bridge())),
            Token::BridgeGroup => Some(Statement::BridgeGroup(self.parse_bridge_group())),
            Token::LinkGroup => Some(Statement::LinkGroup(self.parse_link_group())),
            Token::Signal => Some(Statement::Signal(self.parse_signal())),
            Token::Flag => Some(Statement::Flag(self.parse_flag())),
            Token::Stream => Some(Statement::Stream(self.parse_stream())),
            Token::Config => Some(Statement::Config(self.parse_config())),
            Token::Use => Some(Statement::Use(self.parse_use())),
            Token::Ring => Some(Statement::Ring(self.parse_ring())),
            Token::Network => Some(Statement::Network(self.parse_network())),
            _ => None,
        }
    }
}

mod statements;
mod refs;


// ── TemplateParserExt trait implementation ───────────────────

impl<'a> TemplateParserExt for Parser<'a> {
    fn peek_token(&self) -> Option<&Token> { self.peek() }
    fn at_end_of_input(&self) -> bool { self.at_end() }
    fn advance_token(&mut self) -> &SpannedToken { self.advance() }
    fn current_span_ext(&self) -> Span { self.current_span() }
    fn span_from_ext(&self, start: usize) -> Span { self.span_from(start) }
    fn expect_tok(&mut self, expected: &Token) -> bool { self.expect(expected) }
    fn expect_ident(&mut self) -> Option<String> { self.expect_identifier() }

    fn push_error(&mut self, message: String, span: Span, hint: Option<String>) {
        self.errors.push(ParseError { message, span, hint });
    }

    fn parse_port_ref(&mut self) -> PortRef {
        Parser::parse_port_ref(self)
    }

    fn parse_optional_index(&mut self) -> Option<IndexSpec> {
        Parser::parse_optional_index(self)
    }

    fn parse_arg_list(&mut self) -> Vec<KeyValue> {
        self.parse_optional_arg_list()
    }

    fn parse_optional_version_constraint(&mut self) -> Option<String> {
        self.parse_optional_version()
    }

    fn parse_route_entry_ext(&mut self) -> RouteEntry {
        self.parse_route_entry()
    }

    fn parse_bus_entry_ext(&mut self) -> BusEntry {
        self.parse_bus_entry()
    }

    fn parse_slot_assignment_ext(&mut self) -> SlotAssignment {
        self.parse_slot_assignment()
    }

    fn parse_suppress_annotation_ext(&mut self) -> Vec<String> {
        self.parse_suppress_annotation()
    }

    fn is_property_key_ext(&self) -> bool {
        self.is_property_key()
    }

    fn parse_key_value_full_ext(&mut self) -> KeyValue {
        self.parse_key_value_full()
    }
}

#[cfg(test)]
mod test_helpers;
#[cfg(test)]
mod tests_basic;
#[cfg(test)]
mod tests_declarations;
#[cfg(test)]
mod tests_instance_body;
