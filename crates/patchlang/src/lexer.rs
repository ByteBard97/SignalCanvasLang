use logos::Logos;

/// Why a token failed to lex.
///
/// `logos` needs a `Default` variant: callbacks that return `Option` (e.g. the
/// number parser) map `None` onto `Error::default()`.
#[derive(Default, Debug, Clone, PartialEq)]
pub enum LexError {
    /// A character that starts no token at all.
    #[default]
    UnexpectedCharacter,
    /// An unrecognised escape sequence inside a string literal, e.g. `\q`.
    /// Carries the offending two-character sequence so the parse error can name it.
    UnknownEscape(String),
}

/// The escape sequences a quoted string may contain, as (source char, decoded char).
///
/// The single source of truth for the escape set: this module reads it left-to-right
/// to decode, and `formatter_emit::emit_quoted` reads it right-to-left to encode, so
/// the two are inverse by construction rather than by inspection.
pub(crate) const ESCAPES: [(char, char); 5] = [
    ('\\', '\\'),
    ('"', '"'),
    ('n', '\n'),
    ('r', '\r'),
    ('t', '\t'),
];

/// Strip the surrounding quotes from a string-literal slice and decode its escapes.
///
/// Raw newlines/tabs inside the quotes are preserved verbatim — the regex still
/// accepts them, so files written before escaping existed keep parsing.
fn unescape_string_literal(slice: &str) -> Result<String, LexError> {
    // The regex guarantees a leading and trailing quote.
    let inner = &slice[1..slice.len() - 1];
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();

    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        // The regex pairs every backslash with a following character, so `None`
        // here is unreachable; report it as a bare backslash rather than panic.
        let escaped = chars.next();
        match escaped.and_then(|e| ESCAPES.iter().find(|(src, _)| *src == e)) {
            Some((_, decoded)) => out.push(*decoded),
            None => {
                let seq = match escaped {
                    Some(e) => format!("\\{e}"),
                    None => "\\".to_string(),
                };
                return Err(LexError::UnknownEscape(seq));
            }
        }
    }

    Ok(out)
}

#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(error = LexError)]
#[logos(skip r"[ \t\r\n]+")]
#[logos(skip r"#[^\n]*")]
pub enum Token {
    // Keywords
    #[token("template")]
    Template,
    #[token("instance")]
    Instance,
    #[token("is")]
    Is,
    #[token("connect")]
    Connect,
    #[token("bridge")]
    Bridge,
    #[token("bridge_group")]
    BridgeGroup,
    #[token("link_group")]
    LinkGroup,
    #[token("signal")]
    Signal,
    #[token("flag")]
    Flag,
    #[token("stream")]
    Stream,
    #[token("config")]
    Config,
    #[token("ports")]
    Ports,
    #[token("meta")]
    Meta,
    #[token("in")]
    In,
    #[token("out")]
    Out,
    #[token("io")]
    Io,
    #[token("for")]
    For,
    #[token("over")]
    Over,
    #[token("generate")]
    Generate,
    #[token("use")]
    Use,
    #[token("slot")]
    Slot,
    #[token("routing")]
    Routing,
    #[token("route")]
    Route,
    #[token("bus")]
    Bus,
    #[token("label")]
    Label,
    #[token("ring")]
    Ring,
    #[token("network")]
    Network,
    #[token("member")]
    Member,

    // Annotations
    #[token("@suppress")]
    Suppress,
    #[token("@version")]
    Version,

    // Literals
    #[regex(r"0|[1-9][0-9]*", |lex| lex.slice().parse::<u32>().ok())]
    Number(u32),
    #[regex(r#""([^"\\]|\\[\s\S])*""#, |lex| unescape_string_literal(lex.slice()))]
    StringLiteral(String),

    // Punctuation
    #[token("->")]
    Arrow,
    #[token("..")]
    DotDot,
    #[token(".")]
    Dot,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token(":")]
    Colon,
    #[token(",")]
    Comma,
    #[token("*")]
    Star,

    // Identifier (must be after keywords — logos handles longest match)
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    Identifier(String),
}

/// A token with its span in the source text.
#[derive(Debug, Clone)]
pub struct SpannedToken {
    pub token: Token,
    pub span: std::ops::Range<usize>,
}

/// Tokenize source text into a vector of spanned tokens.
pub fn tokenize(source: &str) -> (Vec<SpannedToken>, Vec<crate::error::ParseError>) {
    let mut tokens = Vec::new();
    let mut errors = Vec::new();
    let mut lexer = Token::lexer(source);

    while let Some(result) = lexer.next() {
        let span = lexer.span();
        match result {
            Ok(token) => tokens.push(SpannedToken { token, span }),
            Err(err) => {
                let (message, hint) = match err {
                    LexError::UnexpectedCharacter => (
                        format!("unexpected character '{}'", &source[span.clone()]),
                        None,
                    ),
                    LexError::UnknownEscape(seq) => (
                        format!("unknown escape sequence '{seq}' in string literal"),
                        Some(r#"valid escapes are \\, \", \n, \r and \t"#.to_string()),
                    ),
                };
                errors.push(crate::error::ParseError {
                    message,
                    span: crate::error::Span {
                        start: span.start,
                        end: span.end,
                        file: None,
                    },
                    hint,
                });
            }
        }
    }

    (tokens, errors)
}
