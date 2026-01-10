use std::fmt::Display;

use crate::parse::Span;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[rustfmt::skip]
pub enum TokenKind {
  Unknown, Eof, Newline,

  LParen, RParen, LCurly, RCurly,
  LBrack, RBrack,
  Eq, Comma, Colon, SemiColon,
  Minus, Star, Slash,
  Quote,

  String, UnterminatedString,


  True, False,

  Name, Int, HexLit,
}

impl TokenKind {
    /// Whether we are a string/unterminated string
    pub fn is_string(&self) -> bool {
        matches!(self, Self::String | Self::UnterminatedString)
    }
}

impl Display for TokenKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            TokenKind::Unknown => "unknown text",
            TokenKind::Eof => "end of file",
            TokenKind::Newline => "new line",
            TokenKind::LParen => "'('",
            TokenKind::RParen => "')'",
            TokenKind::LCurly => "'{'",
            TokenKind::RCurly => "'}'",
            TokenKind::LBrack => "'['",
            TokenKind::RBrack => "']'",
            TokenKind::Eq => "'='",
            TokenKind::Comma => "','",
            TokenKind::Colon => "':'",
            TokenKind::SemiColon => "';'",
            TokenKind::Minus => "'-'",
            TokenKind::Star => "'*'",
            TokenKind::Slash => "'/'",
            TokenKind::Quote => "'\"'",
            TokenKind::String => "string literal",
            TokenKind::UnterminatedString => "unterminated string literal",
            TokenKind::True => "'true'",
            TokenKind::False => "'false'",
            TokenKind::Name => "keyword",
            TokenKind::Int => "number",
            TokenKind::HexLit => "hexadecimal literal",
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}
pub fn lex(mut text: &str) -> Vec<Token> {
    use TokenKind::*;
    let punctuation = (
        "( ) { } [ ] = , : ; - * /",
        [
            LParen, RParen, LCurly, RCurly, LBrack, RBrack, Eq, Comma, Colon, SemiColon, Minus,
            Star, Slash,
        ],
    );

    let keywords = ("true false", [True, False]);

    let source = text;

    let mut result: Vec<Token> = Vec::new();
    while !text.is_empty() {
        if let Some(rest) = trim(text, |it| it.is_ascii_whitespace()) {
            let eaten = &source[source.len() - text.len()..source.len() - rest.len()];
            if let Some(last_token) = result.last() {
                if matches!(
                    last_token.kind,
                    TokenKind::Name
                        | TokenKind::Int
                        | TokenKind::RCurly
                        | TokenKind::String
                        | TokenKind::Eq
                ) && eaten.find(['\n', '\r']).is_some()
                {
                    let start = source.len() - text.len();
                    let end = source.len() - rest.len();
                    let span = Span::new(start as _, end as _);
                    result.push(Token {
                        span,
                        kind: TokenKind::Newline,
                    });
                }
            }

            text = rest;
            continue;
        }
        let text_orig = text;
        let mut kind = 'kind: {
            for (i, symbol) in punctuation.0.split_ascii_whitespace().enumerate() {
                if let Some(rest) = text.strip_prefix(symbol) {
                    text = rest;
                    break 'kind punctuation.1[i];
                }
            }

            if let Some(rest) = text.strip_prefix("0x") {
                text = rest;
                if let Some(rest) = trim(text, |it| matches!(it, 'a'..'f' | 'A'..'F' | '0'..='9')) {
                    text = rest;
                }
                break 'kind HexLit;
            }

            if let Some(rest) = trim(text, |it| it.is_ascii_digit()) {
                text = rest;
                break 'kind Int;
            }

            if let Some(rest) = text.strip_prefix(['\'', '"']) {
                text = rest;
                let mut skip = false;
                loop {
                    let Some(c) = text.chars().next() else {
                        break 'kind UnterminatedString;
                    };

                    text = &text[c.len_utf8()..];
                    match c {
                        '\\' => {
                            skip = true;
                        }
                        '\'' | '"' => match skip {
                            true => {
                                skip = false;
                            }
                            false => {
                                break 'kind String;
                            }
                        },
                        '\n' | '\r' => break 'kind UnterminatedString,
                        _ => {}
                    }
                }
            }
            if let Some(rest) = trim(text, name_char) {
                text = rest;
                break 'kind Name;
            }

            let error_index = text
                .find(|it: char| it.is_ascii_whitespace())
                .unwrap_or(text.len());
            text = &text[error_index..];
            Unknown
        };
        assert!(text.len() < text_orig.len());
        let token_text = &text_orig[..text_orig.len() - text.len()];

        let start = source.len() - text_orig.len();
        let end = source.len() - text.len();

        let span = Span {
            start: start as u32,
            end: end as u32,
        };
        if kind == Name {
            for (i, symbol) in keywords.0.split_ascii_whitespace().enumerate() {
                if token_text == symbol {
                    kind = keywords.1[i];
                    break;
                }
            }
        }
        result.push(Token { kind, span });
    }
    return result;

    fn name_char(c: char) -> bool {
        matches!(c, '_' | 'a'..='z' | 'A'..='Z' | '0'..='9')
    }

    fn trim(text: &str, predicate: impl std::ops::Fn(char) -> bool) -> Option<&str> {
        let index = text.find(|it: char| !predicate(it)).unwrap_or(text.len());
        if index == 0 {
            None
        } else {
            Some(&text[index..])
        }
    }
}
