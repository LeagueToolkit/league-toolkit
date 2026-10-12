//! Parses numbers with the rules of the C functions that the client calls to convert a string:
//! `atoi`, `atof` and `sscanf` with `%f`. Each function skips leading whitespace, reads the
//! longest number at the start of the text and ignores the rest of the text.

fn skip_whitespace(text: &str) -> &str {
    text.trim_start_matches([' ', '\t', '\n', '\x0b', '\x0c', '\r'])
}

fn digit_count(bytes: &[u8]) -> usize {
    bytes
        .iter()
        .take_while(|byte| byte.is_ascii_digit())
        .count()
}

/// Returns the length of the float at the start of `bytes`. Returns 0 if `bytes` does not start
/// with a float.
fn float_len(bytes: &[u8]) -> usize {
    let mut len = usize::from(matches!(bytes.first(), Some(b'+' | b'-')));

    for word in ["infinity", "inf", "nan"] {
        let end = len + word.len();
        if bytes.len() >= end && bytes[len..end].eq_ignore_ascii_case(word.as_bytes()) {
            return end;
        }
    }

    let integer_digits = digit_count(&bytes[len..]);
    len += integer_digits;
    let mut fraction_digits = 0;
    if bytes.get(len) == Some(&b'.') {
        fraction_digits = digit_count(&bytes[len + 1..]);
        if integer_digits + fraction_digits > 0 {
            len += 1 + fraction_digits;
        }
    }
    if integer_digits + fraction_digits == 0 {
        return 0;
    }

    if matches!(bytes.get(len), Some(b'e' | b'E')) {
        let mut exponent = len + 1;
        exponent += usize::from(matches!(bytes.get(exponent), Some(b'+' | b'-')));
        let exponent_digits = digit_count(&bytes[exponent..]);
        if exponent_digits > 0 {
            len = exponent + exponent_digits;
        }
    }
    len
}

/// Returns the integer at the start of `text`. Returns 0 if `text` does not start with an integer.
/// Returns `i64::MAX` or `i64::MIN` if the integer does not fit in an `i64`.
pub(crate) fn atoi(text: &str) -> i64 {
    let bytes = skip_whitespace(text).as_bytes();
    let (negative, digits) = match bytes.first() {
        Some(b'-') => (true, &bytes[1..]),
        Some(b'+') => (false, &bytes[1..]),
        _ => (false, bytes),
    };

    let mut value: i64 = 0;
    for &digit in &digits[..digit_count(digits)] {
        let digit = i64::from(digit - b'0');
        value = value.saturating_mul(10);
        value = if negative {
            value.saturating_sub(digit)
        } else {
            value.saturating_add(digit)
        };
    }
    value
}

/// Returns the float at the start of `text`. Returns 0.0 if `text` does not start with a float.
pub(crate) fn atof(text: &str) -> f32 {
    let text = skip_whitespace(text);
    let len = float_len(text.as_bytes());
    text[..len].parse::<f64>().unwrap_or(0.0) as f32
}

/// Returns the first `N` floats of `text`. Returns `None` if `text` starts with fewer than `N`
/// floats.
pub(crate) fn scan_floats<const N: usize>(text: &str) -> Option<[f32; N]> {
    let mut floats = [0.0; N];
    let mut text = text;
    for float in &mut floats {
        text = skip_whitespace(text);
        let len = float_len(text.as_bytes());
        *float = text[..len].parse().ok()?;
        text = &text[len..];
    }
    Some(floats)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atoi_reads_leading_integer() {
        assert_eq!(atoi("42"), 42);
        assert_eq!(atoi("  -17abc"), -17);
        assert_eq!(atoi("+8"), 8);
        assert_eq!(atoi("3.9"), 3);
    }

    #[test]
    fn atoi_returns_zero_without_leading_integer() {
        assert_eq!(atoi(""), 0);
        assert_eq!(atoi("abc"), 0);
        assert_eq!(atoi("-"), 0);
    }

    #[test]
    fn atoi_clamps_to_i64_range() {
        assert_eq!(atoi("99999999999999999999"), i64::MAX);
        assert_eq!(atoi("-99999999999999999999"), i64::MIN);
    }

    #[test]
    fn atof_reads_leading_float() {
        assert_eq!(atof("1.5"), 1.5);
        assert_eq!(atof(" -.5 1"), -0.5);
        assert_eq!(atof("1."), 1.0);
        assert_eq!(atof("1E+09"), 1e9);
        assert_eq!(atof("2e"), 2.0);
        assert_eq!(atof("5.5.0000"), 5.5);
        assert_eq!(atof("-inf"), f32::NEG_INFINITY);
        assert!(atof("NaN").is_nan());
    }

    #[test]
    fn atof_returns_zero_without_leading_float() {
        assert_eq!(atof(""), 0.0);
        assert_eq!(atof("."), 0.0);
        assert_eq!(atof("true"), 0.0);
    }

    #[test]
    fn scan_floats_reads_whitespace_separated_floats() {
        assert_eq!(scan_floats("0 .2"), Some([0.0, 0.2]));
        assert_eq!(scan_floats("450 0\t0"), Some([450.0, 0.0, 0.0]));
        assert_eq!(scan_floats("1 2 3 4"), Some([1.0, 2.0, 3.0, 4.0]));
    }

    #[test]
    fn scan_floats_ignores_text_after_last_float() {
        assert_eq!(scan_floats("0 .4'"), Some([0.0, 0.4]));
        assert_eq!(scan_floats("1 2 3 4 5"), Some([1.0, 2.0, 3.0]));
    }

    #[test]
    fn scan_floats_returns_none_for_fewer_floats() {
        assert_eq!(scan_floats::<2>("1 ,8"), None);
        assert_eq!(scan_floats::<3>("1 2"), None);
        assert_eq!(scan_floats::<1>(""), None);
    }
}
