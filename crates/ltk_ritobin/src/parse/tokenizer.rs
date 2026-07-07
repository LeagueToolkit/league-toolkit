use std::fmt::Display;

use crate::parse::Span;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[rustfmt::skip]
pub enum TokenKind {
  Unknown, Eof, Newline,

  LParen, RParen, LCurly, RCurly,
  LBrack, RBrack,
  Eq, Comma, Colon, SemiColon,
  Star, Slash,
  Quote,

  String, UnterminatedString,

  Comment,

  // keywords
  True, False, Null,

  Name, Number, HexLit,
}

impl TokenKind {
    /// Whether we are a string/unterminated string
    pub fn is_string(&self) -> bool {
        matches!(self, Self::String | Self::UnterminatedString)
    }

    /// Maps a punctuation byte -> its token kind, inverse of [`Self::print_value`] for single-byte punctuation
    pub fn from_punct_byte(b: u8) -> Option<Self> {
        Some(match b {
            b'(' => Self::LParen,
            b')' => Self::RParen,
            b'{' => Self::LCurly,
            b'}' => Self::RCurly,
            b'[' => Self::LBrack,
            b']' => Self::RBrack,
            b'=' => Self::Eq,
            b',' => Self::Comma,
            b':' => Self::Colon,
            b';' => Self::SemiColon,
            b'*' => Self::Star,
            b'/' => Self::Slash,
            _ => return None,
        })
    }

    pub fn print_value(&self) -> Option<&'static str> {
        match self {
            TokenKind::Unknown => None,
            TokenKind::Eof => None,
            TokenKind::Newline => None,
            TokenKind::LParen => Some("("),
            TokenKind::RParen => Some(")"),
            TokenKind::LCurly => Some("{"),
            TokenKind::RCurly => Some("}"),
            TokenKind::LBrack => Some("["),
            TokenKind::RBrack => Some("]"),
            TokenKind::Eq => Some("="),
            TokenKind::Comma => Some(","),
            TokenKind::Colon => Some(":"),
            TokenKind::SemiColon => Some(";"),
            TokenKind::Star => Some("*"),
            TokenKind::Slash => Some("/"),
            TokenKind::Quote => Some("\""),
            TokenKind::String => None,
            TokenKind::UnterminatedString => None,
            TokenKind::Comment => None,
            TokenKind::True => Some("true"),
            TokenKind::False => Some("false"),
            TokenKind::Null => Some("null"),
            TokenKind::Name => None,
            TokenKind::Number => None,
            TokenKind::HexLit => None,
        }
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
            TokenKind::Star => "'*'",
            TokenKind::Slash => "'/'",
            TokenKind::Quote => "'\"'",
            TokenKind::String => "string literal",
            TokenKind::UnterminatedString => "unterminated string literal",
            TokenKind::True => "'true'",
            TokenKind::False => "'false'",
            TokenKind::Null => "'null'",
            TokenKind::Name => "keyword",
            TokenKind::Number => "number",
            TokenKind::HexLit => "hexadecimal literal",
            TokenKind::Comment => "comment",
        })
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}
pub fn lex(source: &str) -> Vec<Token> {
    use TokenKind::*;

    // real bins run ~6.82 (hashes) to ~7.25 (resolved) bytes/token; /6 stays under both so big files don't realloc
    let mut result = Vec::with_capacity(source.len() / 6);
    let mut cur = Cursor::new(source);

    while !cur.at_end() {
        let start = cur.pos;

        // whitespace: eat the run, emitting a synthetic Newline between value-ending tokens
        // when it spans a line break
        if cur.consume_while(|b| b.is_ascii_whitespace()) > 0 {
            if ends_line(result.last(), cur.consumed(start)) {
                result.push(Token {
                    kind: Newline,
                    span: cur.span(start),
                });
            }

            continue;
        }

        let mut kind = cur.consume_token();
        assert!(cur.pos > start);

        if kind == Name {
            kind = keyword_or_name(&source[start..cur.pos]);
        }

        result.push(Token {
            kind,
            span: cur.span(start),
        });
    }

    result
}

/// A byte cursor over the source. `pos` is always on a UTF-8 char boundary: every token
/// ends on an ASCII byte (or swallows whole multi-byte sequences), so slicing at `pos`
/// never splits a char.
struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(source: &'a str) -> Self {
        Cursor {
            bytes: source.as_bytes(),
            pos: 0,
        }
    }

    fn at_end(&self) -> bool {
        self.pos >= self.bytes.len()
    }

    fn rest(&self) -> &'a [u8] {
        &self.bytes[self.pos..]
    }

    fn consumed(&self, start: usize) -> &'a [u8] {
        &self.bytes[start..self.pos]
    }

    fn span(&self, start: usize) -> Span {
        Span::new(start as u32, self.pos as u32)
    }

    fn starts_with(&self, prefix: &[u8]) -> bool {
        self.rest().starts_with(prefix)
    }

    #[inline]
    fn consume_while(&mut self, pred: impl Fn(u8) -> bool) -> usize {
        let n = scan_run(self.rest(), pred);
        self.pos += n;
        n
    }

    #[inline]
    fn consume_token(&mut self) -> TokenKind {
        use TokenKind::*;
        let b = self.bytes[self.pos];

        if let Some(k) = TokenKind::from_punct_byte(b) {
            self.pos += 1;
            return k;
        }

        if b == b'#' {
            self.pos += 1;
            self.consume_while(|b| !matches!(b, b'\n' | b'\r'));
            return Comment;
        }

        if self.starts_with(b"0x") {
            self.pos += 2;
            self.consume_while(|b| b.is_ascii_hexdigit());
            return HexLit;
        }

        if let Some(len) = scan_number(self.rest()) {
            self.pos += len;
            return Number;
        }

        if matches!(b, b'\'' | b'"') {
            return self.consume_string();
        }

        if self.consume_while(is_name_char) > 0 {
            return Name;
        }

        // unrecognized: consume up to the next whitespace
        self.consume_while(|b| !b.is_ascii_whitespace());
        Unknown
    }

    #[inline]
    fn consume_string(&mut self) -> TokenKind {
        use TokenKind::*;
        let quote = self.bytes[self.pos];
        self.pos += 1;

        let mut escaped = false;
        while let Some(&b) = self.bytes.get(self.pos) {
            self.pos += 1;
            match b {
                b'\\' => escaped = true,
                b'\n' | b'\r' => return UnterminatedString,
                _ if b == quote && !escaped => return String,
                _ => escaped = false,
            }
        }

        UnterminatedString
    }
}

fn is_name_char(b: u8) -> bool {
    matches!(b, b'_' | b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9')
}

fn keyword_or_name(word: &str) -> TokenKind {
    use TokenKind::*;
    match word {
        "true" => True,
        "false" => False,
        "null" => Null,
        _ => Name,
    }
}

fn ends_value(kind: TokenKind) -> bool {
    use TokenKind::*;
    matches!(
        kind,
        Name | HexLit | True | False | Number | RCurly | String | Eq | Comment
    )
}

fn ends_line(last: Option<&Token>, ws: &[u8]) -> bool {
    last.is_some_and(|t| ends_value(t.kind)) && ws.iter().any(|&b| matches!(b, b'\n' | b'\r'))
}

#[inline]
fn scan_run(bytes: &[u8], pred: impl Fn(u8) -> bool) -> usize {
    bytes.iter().position(|&b| !pred(b)).unwrap_or(bytes.len())
}

#[inline]
fn scan_number_segment(bytes: &[u8]) -> Option<usize> {
    let len = scan_run(bytes, |b| matches!(b, b'0'..=b'9' | b'_'));
    bytes[..len]
        .split(|&b| b == b'_')
        .all(|group| !group.is_empty())
        .then_some(len)
}

#[inline]
fn scan_number(bytes: &[u8]) -> Option<usize> {
    let mut i = (bytes.first() == Some(&b'-')) as usize;

    // ".5"
    if bytes.get(i) == Some(&b'.') {
        return Some(i + 1 + scan_number_segment(&bytes[i + 1..])?);
    }

    // "123" / "123.45"
    i += scan_number_segment(&bytes[i..])?;
    if bytes.get(i) == Some(&b'.') {
        i += 1 + scan_number_segment(&bytes[i + 1..])?;
    }

    Some(i)
}
