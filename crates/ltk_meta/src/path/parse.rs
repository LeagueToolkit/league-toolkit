//! The property path tokenizer.
//!
//! Grammar (see the module docs of [`super`] for the semantics):
//!
//! ```text
//! path      = segment *( "." segment )
//! segment   = name [ subscript ]
//! name      = 1*name-char        ; anything but "." "[" "]" "{" "}" "(" ")" and controls
//! subscript = "[" index "]" / "{" key "}"
//! index     = dec-int / "0x" hex-int / "0" oct-int      ; strtol(base 0), non-negative
//! key       = *ws ( json-number / json-string / "true" / "false" ) *ws
//! ```

use std::borrow::Cow;

use super::{
    KeyLiteral, PropertyPath, PropertyPathError as Error, PropertyPathErrorKind as Kind, Segment,
    Subscript,
};

/// Checks that `path` is a well formed property path.
pub(super) fn validate(path: &str) -> Result<(), Error> {
    if path.len() > PropertyPath::MAX_LEN {
        return Err(Error::new(PropertyPath::MAX_LEN, Kind::TooLong(path.len())));
    }

    let mut parser = Parser::new(path);
    while let Some(segment) = parser.next_segment() {
        segment?;
    }
    Ok(())
}

/// Walks a path one segment at a time.
#[derive(Clone, Debug)]
pub(super) struct Parser<'a> {
    src: &'a str,
    pos: usize,
    done: bool,
}

impl<'a> Parser<'a> {
    pub(super) fn new(src: &'a str) -> Self {
        Self {
            src,
            pos: 0,
            done: false,
        }
    }

    pub(super) fn next_segment(&mut self) -> Option<Result<Segment<'a>, Error>> {
        if self.done {
            return None;
        }

        let segment = self.parse_segment();
        if segment.is_err() {
            self.done = true;
        }
        Some(segment)
    }

    fn parse_segment(&mut self) -> Result<Segment<'a>, Error> {
        let start = self.pos;
        let mut end = start;
        for c in self.src[start..].chars() {
            if !is_name_char(c) {
                break;
            }
            end += c.len_utf8();
        }
        if end == start {
            return Err(Error::new(start, Kind::EmptySegment));
        }
        let name = &self.src[start..end];

        let subscript = match self.src[end..].chars().next() {
            Some('[') => {
                let (index, next) = parse_index(self.src, end)?;
                end = next;
                Some(Subscript::Index(index))
            }
            Some('{') => {
                let (key, next) = parse_key(self.src, end)?;
                end = next;
                Some(Subscript::Key(key))
            }
            _ => None,
        };

        match self.src[end..].chars().next() {
            None => {
                self.pos = end;
                self.done = true;
            }
            Some('.') => self.pos = end + 1,
            Some('[' | '{') if subscript.is_some() => {
                return Err(Error::new(end, Kind::DoubleSubscript))
            }
            Some(c) => return Err(Error::new(end, Kind::UnexpectedCharacter(c))),
        }

        Ok(Segment { name, subscript })
    }
}

pub(super) fn is_name_char(c: char) -> bool {
    !matches!(c, '.' | '[' | ']' | '{' | '}' | '(' | ')') && !c.is_control()
}

/// Parses `[index]` starting at the `[` in `open`, returning the index and the offset past `]`.
fn parse_index(src: &str, open: usize) -> Result<(u32, usize), Error> {
    let Some(offset) = src[open + 1..].find(']') else {
        return Err(Error::new(open, Kind::UnbalancedBracket));
    };
    let close = open + 1 + offset;

    let index =
        parse_int(&src[open + 1..close]).ok_or_else(|| Error::new(open + 1, Kind::InvalidIndex))?;
    Ok((index, close + 1))
}

/// `strtol` with base 0, restricted to non-negative values that fill the whole text.
fn parse_int(text: &str) -> Option<u32> {
    let (radix, digits) =
        if let Some(rest) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
            (16, rest)
        } else if text.len() > 1 && text.starts_with('0') {
            (8, &text[1..])
        } else {
            (10, text)
        };

    if digits.is_empty() || !digits.chars().all(|c| c.is_digit(radix)) {
        return None;
    }
    u32::from_str_radix(digits, radix).ok()
}

/// Parses `{key}` starting at the `{` in `open`, returning the key and the offset past `}`.
fn parse_key(src: &str, open: usize) -> Result<(KeyLiteral<'_>, usize), Error> {
    let bytes = src.as_bytes();

    let start = skip_whitespace(src, open + 1);
    let (key, after) = match bytes.get(start) {
        None => return Err(Error::new(open, Kind::UnbalancedBracket)),
        Some(b'"') => parse_json_string(src, start)?,
        Some(b't') if src[start..].starts_with("true") => (KeyLiteral::Bool(true), start + 4),
        Some(b'f') if src[start..].starts_with("false") => (KeyLiteral::Bool(false), start + 5),
        Some(b'-' | b'0'..=b'9') => parse_json_number(src, start)?,
        Some(_) => return Err(Error::new(start, Kind::InvalidKey)),
    };

    let close = skip_whitespace(src, after);
    match bytes.get(close) {
        Some(b'}') => Ok((key, close + 1)),
        Some(_) => Err(Error::new(close, Kind::InvalidKey)),
        None => Err(Error::new(open, Kind::UnbalancedBracket)),
    }
}

fn skip_whitespace(src: &str, mut at: usize) -> usize {
    let bytes = src.as_bytes();
    while matches!(bytes.get(at), Some(b' ' | b'\t' | b'\n' | b'\r')) {
        at += 1;
    }
    at
}

/// Parses a JSON string starting at the quote in `start`.
fn parse_json_string(src: &str, start: usize) -> Result<(KeyLiteral<'_>, usize), Error> {
    let bytes = src.as_bytes();

    let mut at = start + 1;
    let mut escaped = false;
    loop {
        match bytes.get(at) {
            None => return Err(Error::new(start, Kind::UnbalancedBracket)),
            Some(b'"') => break,
            Some(b'\\') => {
                escaped = true;
                at += 2;
            }
            Some(&b) if b < 0x20 => return Err(Error::new(at, Kind::InvalidKey)),
            Some(_) => at += 1,
        }
    }

    let text = &src[start + 1..at];
    let key = match escaped {
        true => KeyLiteral::String(Cow::Owned(unescape(text, start + 1)?)),
        false => KeyLiteral::String(Cow::Borrowed(text)),
    };
    Ok((key, at + 1))
}

/// Resolves the JSON escapes in a string body. `origin` is where the body starts in the path.
fn unescape(text: &str, origin: usize) -> Result<String, Error> {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut at = 0;

    while at < bytes.len() {
        if bytes[at] != b'\\' {
            let c = text[at..]
                .chars()
                .next()
                .ok_or_else(|| Error::new(origin + at, Kind::InvalidKey))?;
            out.push(c);
            at += c.len_utf8();
            continue;
        }

        let escape = at;
        at += 1;
        let Some(&kind) = bytes.get(at) else {
            return Err(Error::new(origin + escape, Kind::InvalidKey));
        };
        at += 1;

        out.push(match kind {
            b'"' => '"',
            b'\\' => '\\',
            b'/' => '/',
            b'b' => '\u{8}',
            b'f' => '\u{c}',
            b'n' => '\n',
            b'r' => '\r',
            b't' => '\t',
            b'u' => {
                let high = parse_hex4(text, at, origin)?;
                at += 4;
                match high {
                    // A high surrogate is only valid as the first half of a pair.
                    0xd800..=0xdbff => {
                        if text[at..].starts_with("\\u") {
                            at += 2;
                            let low = parse_hex4(text, at, origin)?;
                            at += 4;
                            if !(0xdc00..=0xdfff).contains(&low) {
                                return Err(Error::new(origin + at, Kind::InvalidKey));
                            }
                            let code = 0x10000 + ((high - 0xd800) << 10) + (low - 0xdc00);
                            char::from_u32(code)
                                .ok_or_else(|| Error::new(origin + at, Kind::InvalidKey))?
                        } else {
                            return Err(Error::new(origin + at, Kind::InvalidKey));
                        }
                    }
                    _ => char::from_u32(high)
                        .ok_or_else(|| Error::new(origin + at, Kind::InvalidKey))?,
                }
            }
            _ => return Err(Error::new(origin + escape, Kind::InvalidKey)),
        });
    }

    Ok(out)
}

fn parse_hex4(text: &str, at: usize, origin: usize) -> Result<u32, Error> {
    text.get(at..at + 4)
        .and_then(|hex| u32::from_str_radix(hex, 16).ok())
        .ok_or_else(|| Error::new(origin + at, Kind::InvalidKey))
}

/// Parses a JSON number starting at `start`.
fn parse_json_number(src: &str, start: usize) -> Result<(KeyLiteral<'_>, usize), Error> {
    let bytes = src.as_bytes();
    let mut at = start;

    if bytes.get(at) == Some(&b'-') {
        at += 1;
    }

    match bytes.get(at) {
        // JSON forbids leading zeros, so a `0` integer part ends right there.
        Some(b'0') => at += 1,
        Some(b'1'..=b'9') => at = skip_digits(bytes, at),
        _ => return Err(Error::new(start, Kind::InvalidKey)),
    }

    if bytes.get(at) == Some(&b'.') {
        at += 1;
        let end = skip_digits(bytes, at);
        if end == at {
            return Err(Error::new(at, Kind::InvalidKey));
        }
        at = end;
    }

    if matches!(bytes.get(at), Some(b'e' | b'E')) {
        at += 1;
        if matches!(bytes.get(at), Some(b'+' | b'-')) {
            at += 1;
        }
        let end = skip_digits(bytes, at);
        if end == at {
            return Err(Error::new(at, Kind::InvalidKey));
        }
        at = end;
    }

    Ok((KeyLiteral::Number(&src[start..at]), at))
}

fn skip_digits(bytes: &[u8], mut at: usize) -> usize {
    while matches!(bytes.get(at), Some(b'0'..=b'9')) {
        at += 1;
    }
    at
}
