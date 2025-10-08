use error::{BinError, ToMietteSpan as _};

use crate::{Literal, Span, Statement};

pub mod error;

pub enum Hash<'a> {
    Hash(u64),
    Unhash(&'a str),
}

fn parse_hash<'a>(span: &'a Span, radix: u32) -> Result<Hash<'a>, BinError> {
    u64::from_str_radix(span.as_ref(), radix)
        .map_err(|inner| BinError::InvalidHash {
            inner,
            span: span.into_miette(),
        })
        .map(Hash::Hash)
}

pub fn validate(statements: Vec<Statement>) -> miette::Result<(), Vec<BinError>> {
    // let mut entries = HashMap::new();
    // let mut value = None;

    let errors: Vec<BinError> = statements
        .iter()
        .map(|stmt| {
            let name: Hash = match &stmt.name {
                Literal::Keyword(span) | Literal::Bool(_, span) => Hash::Unhash(span.as_ref()),

                Literal::Decimal(span) => parse_hash(span, 10)?,
                Literal::Hexadecimal(span) => parse_hash(span, 16)?,
                Literal::Octal(span) => parse_hash(span, 8)?,
                Literal::Binary(span) => parse_hash(span, 2)?,
                name => {
                    return Err(BinError::InvalidRootEntryName {
                        span: name.span().into_miette(),
                        kind: name.kind(),
                        help: match name {
                            Literal::String(_) => Some("try without the double quotes"),
                            _ => None,
                        },
                    })
                }
            };

            let kind = stmt
                .kind
                .as_ref()
                .ok_or_else(|| BinError::RootTypeMissing {
                    span: stmt.name.span().into_miette(),
                })?;

            Ok(())
        })
        .filter_map(|r| r.err())
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
