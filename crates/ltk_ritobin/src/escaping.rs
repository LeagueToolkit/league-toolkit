use crate::{ast::diagnostics::Diagnostic, parse::Span};

#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("Invalid escape character - {reason}")]
pub struct InvalidEscape {
    /// byte index of the offending `\`, relative to the start of the unquoted text
    index: usize,
    reason: InvalidEscapeReason,
}

#[derive(Debug, Clone, Copy, thiserror::Error)]
pub enum InvalidEscapeReason {
    #[error("unrecognised escape '\\{0}'")]
    Unrecognised(char),
    #[error("'\\x' takes two hex digits")]
    BadHex,
    #[error("'\\u' takes four hex digits")]
    BadUnicode,
    #[error("unpaired UTF-16 surrogate U+{0:04X}")]
    LoneSurrogate(u16),
    #[error("UTF-16 surrogate too big")]
    SurrogateTooBig,
}

impl InvalidEscape {
    pub(crate) fn into_diagnostic(self, span: Span) -> Diagnostic {
        Diagnostic::InvalidEscape {
            span,
            // +1 for the opening quote the unquoted text sits past
            offset: u32::try_from(self.index).unwrap() + 1,
            reason: self.reason,
        }
    }
}

pub(crate) fn unescape(input: &str) -> Result<String, InvalidEscape> {
    use InvalidEscapeReason::*;

    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] != b'\\' {
            let start = i;
            // consolidate runs of no `\` into one push_str
            while i < bytes.len() && bytes[i] != b'\\' {
                i += 1;
            }
            out.push_str(&input[start..i]);
            continue;
        }

        let index = i + 1;
        let err = |reason| InvalidEscape { index, reason };
        i += 1; // eat '\'

        match bytes.get(i).copied() {
            Some(b'u') => {
                let mut first = true;
                let units = std::iter::from_fn(|| {
                    // first '\' is already eaten, so we must commit cringe
                    if first {
                        first = false;
                        i += 1; // eat 'u'
                    } else if bytes.get(i) == Some(&b'\\') && bytes.get(i + 1) == Some(&b'u') {
                        i += 2;
                    } else {
                        return None;
                    }
                    Some(
                        read_hex(bytes, &mut i, 4)
                            .map(|u| u16::try_from(u).map_err(|_| err(SurrogateTooBig)))
                            .ok_or(err(BadUnicode))
                            .flatten(),
                    )
                });
                itertools::process_results(units, |units| {
                    for unit in char::decode_utf16(units) {
                        match unit {
                            Ok(c) => out.push(c),
                            Err(e) => return Err(err(LoneSurrogate(e.unpaired_surrogate()))),
                        }
                    }
                    Ok(())
                })??;
            }
            Some(b'x') => {
                i += 1;
                // `\x` takes two hex digits, so the value is always a valid scalar
                out.push(read_hex(bytes, &mut i, 2).ok_or(err(BadHex))? as u8 as char);
            }
            Some(c) => {
                i += 1;
                match c {
                    b'\'' => out.push('\''),
                    b'"' => out.push('"'),
                    b'\\' => out.push('\\'),
                    b'a' => out.push('\x07'),
                    b'b' => out.push('\x08'),
                    b'f' => out.push('\x0C'),
                    b'n' | b'\n' => out.push('\n'),
                    b't' => out.push('\t'),
                    b'r' => out.push('\r'),
                    b'\r' => {
                        if bytes.get(i) == Some(&b'\n') {
                            i += 1;
                            out.push('\n');
                        } else {
                            out.push('\r');
                        }
                    }
                    _ => return Err(err(Unrecognised(c as char))),
                }
            }
            // this isn't actually possible (lexer guarantees it)
            None => out.push('\\'),
        }
    }

    Ok(out)
}

pub(crate) fn escape(input: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";

    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0C}' => out.push_str("\\f"),
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            c if (c as u32) < 0x20 => {
                let b = c as u32;
                out.push('\\');
                out.push('x');
                out.push(HEX[((b >> 4) & 0xF) as usize] as char);
                out.push(HEX[(b & 0xF) as usize] as char);
            }
            c => out.push(c),
        }
    }
    out
}

fn read_hex(bytes: &[u8], i: &mut usize, n: usize) -> Option<u32> {
    let mut value = 0;
    for _ in 0..n {
        let digit = (*bytes.get(*i)? as char).to_digit(16)?;
        value = (value << 4) | digit;
        *i += 1;
    }
    Some(value)
}
