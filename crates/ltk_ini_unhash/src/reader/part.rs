//! Names as slices of parts, and their bytes.

use std::borrow::Cow;

/// One part of a section name or a property name. A name is a slice of parts: `"Mastery"`, `3`,
/// `"Level"` is the name `Mastery3Level`.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Part<'a> {
    Str(&'a str),
    /// An integer in decimal.
    Int(i64),
    /// The parts of another name.
    Name(&'a [Part<'a>]),
}

/// A name as parts. The `name!` macro builds a `Parts`.
pub(crate) type Parts<'a> = &'a [Part<'a>];

/// A value that a part borrows. The `name!` macro takes each variable and each token by
/// reference, and a reference to a value is a value.
pub(crate) trait AsPart {
    fn as_part(&self) -> Part<'_>;
}

impl AsPart for str {
    fn as_part(&self) -> Part<'_> {
        Part::Str(self)
    }
}

impl AsPart for String {
    fn as_part(&self) -> Part<'_> {
        Part::Str(self)
    }
}

impl AsPart for Cow<'_, str> {
    fn as_part(&self) -> Part<'_> {
        Part::Str(self)
    }
}

impl AsPart for i32 {
    fn as_part(&self) -> Part<'_> {
        Part::Int((*self).into())
    }
}

impl AsPart for i64 {
    fn as_part(&self) -> Part<'_> {
        Part::Int(*self)
    }
}

impl AsPart for [Part<'_>] {
    fn as_part(&self) -> Part<'_> {
        Part::Name(self)
    }
}

impl<const N: usize> AsPart for [Part<'_>; N] {
    fn as_part(&self) -> Part<'_> {
        Part::Name(self)
    }
}

impl<T: AsPart + ?Sized> AsPart for &T {
    fn as_part(&self) -> Part<'_> {
        (**self).as_part()
    }
}

/// The value of an expression part. The `name!` macro converts an expression by value: a part
/// of a name that a `let` binds outlives the expression.
impl<'a> From<&'a str> for Part<'a> {
    fn from(text: &'a str) -> Self {
        Part::Str(text)
    }
}

impl<'a> From<&'a String> for Part<'a> {
    fn from(text: &'a String) -> Self {
        Part::Str(text)
    }
}

impl From<i32> for Part<'_> {
    fn from(number: i32) -> Self {
        Part::Int(number.into())
    }
}

impl From<i64> for Part<'_> {
    fn from(number: i64) -> Self {
        Part::Int(number)
    }
}

impl<'a> From<&'a [Part<'a>]> for Part<'a> {
    fn from(parts: &'a [Part<'a>]) -> Self {
        Part::Name(parts)
    }
}

impl<'a, const N: usize> From<&'a [Part<'a>; N]> for Part<'a> {
    fn from(parts: &'a [Part<'a>; N]) -> Self {
        Part::Name(parts)
    }
}

/// Calls `f` with each byte of the name `parts`.
pub(super) fn each_byte(parts: Parts<'_>, f: &mut impl FnMut(u8)) {
    for part in parts {
        match *part {
            Part::Str(text) => text.bytes().for_each(&mut *f),
            Part::Int(number) => {
                let mut digits = [0; 20];
                let mut start = digits.len();
                let mut rest = number.unsigned_abs();
                loop {
                    start -= 1;
                    digits[start] = b'0' + (rest % 10) as u8;
                    rest /= 10;
                    if rest == 0 {
                        break;
                    }
                }
                if number < 0 {
                    f(b'-');
                }
                digits[start..].iter().copied().for_each(&mut *f);
            }
            Part::Name(inner) => each_byte(inner, f),
        }
    }
}

/// Appends the name `parts` to `out`.
pub(super) fn write(parts: Parts<'_>, out: &mut Vec<u8>) {
    each_byte(parts, &mut |byte| out.push(byte));
}

/// Returns `true` if `text` is `prefix` followed by the name `parts`, ignoring ASCII case.
pub(super) fn eq_ignore_case(text: &str, prefix: &str, parts: Parts<'_>) -> bool {
    let Some(rest) = text
        .get(..prefix.len())
        .filter(|head| head.eq_ignore_ascii_case(prefix))
        .map(|_| &text.as_bytes()[prefix.len()..])
    else {
        return false;
    };
    let mut index = 0;
    let mut equal = true;
    each_byte(parts, &mut |byte| {
        equal &= rest
            .get(index)
            .is_some_and(|b| b.eq_ignore_ascii_case(&byte));
        index += 1;
    });
    equal && index == rest.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_byte_writes_integers_in_decimal() {
        let mut text = Vec::new();
        write(
            &[
                Part::Str("a"),
                Part::Int(-12),
                Part::Int(0),
                Part::Int(i64::MIN),
            ],
            &mut text,
        );
        assert_eq!(text, b"a-120-9223372036854775808");
    }

    #[test]
    fn eq_ignore_case_compares_prefix_and_parts() {
        let parts = [Part::Str("Group"), Part::Str("Part"), Part::Int(3)];
        assert!(eq_ignore_case("'GROUPPART3", "'", &parts));
        assert!(!eq_ignore_case("'GroupPart30", "'", &parts));
        assert!(!eq_ignore_case("GroupPart3", "'", &parts));
    }
}
