use std::{
    fmt::{self, Write as _},
    num::ParseFloatError,
    str::FromStr,
};

/// f32 newtype, providing a FromStr & Display impl that parses/outputs valid ritobin float literals
pub struct FloatLiteral(pub f32);

impl From<f32> for FloatLiteral {
    fn from(value: f32) -> Self {
        Self(value)
    }
}

impl FromStr for FloatLiteral {
    type Err = ParseFloatError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (neg, body) = s.strip_prefix('-').map_or((false, s), |rest| (true, rest));

        let magnitude = if body.eq_ignore_ascii_case("inf") {
            f32::INFINITY
        } else if body.eq_ignore_ascii_case("nan") {
            f32::NAN
        } else if body.eq_ignore_ascii_case("nan(snan)") {
            f32::from_bits(0x7f80_0001)
        } else if body.eq_ignore_ascii_case("nan(ind)") {
            f32::from_bits(0x7fc0_0000)
        } else {
            body.parse::<f32>()?
        };

        Ok(Self(if neg { -magnitude } else { magnitude }))
    }
}

impl fmt::Display for FloatLiteral {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let v = self.0;
        if v.is_sign_negative() {
            f.write_char('-')?;
        }
        if v.is_infinite() {
            return f.write_str("inf");
        }
        if v.is_nan() {
            const QUIET: u32 = 0x0040_0000;
            let mantissa = v.to_bits() & 0x007f_ffff;
            return f.write_str(match mantissa {
                m if m & QUIET == 0 => "nan(snan)",
                QUIET if v.is_sign_negative() => "nan(ind)",
                _ => "nan",
            });
        }

        let mut buf = StackStr::<32>::new();
        let (digits, exp) = shortest_digits(v.abs(), &mut buf)?;
        let (lead, frac) = digits.split_at(1);
        let n = digits.len() as i32;

        // The digits, a point after the first, `e`, the sign and two exponent digits.
        let sci_len = n + i32::from(n > 1) + 4;
        let fixed_len = match exp {
            e if e >= n - 1 => e + 1,
            e if e >= 0 => n + 1,
            e => n + 1 - e,
        };

        if fixed_len > sci_len {
            f.write_str(lead)?;
            if !frac.is_empty() {
                f.write_char('.')?;
                f.write_str(frac)?;
            }
            let sign = if exp < 0 { '-' } else { '+' };
            return write!(f, "e{sign}{:02}", exp.abs());
        }

        let digits = || lead.chars().chain(frac.chars());
        match exp {
            // An integer. Its exact digits are as many as the shortest digits padded with zeros,
            // and among representations of one length `to_chars` takes the closest: `4294967296`,
            // not `4294967300`. A fixed integer has at most 14 digits, below `u64::MAX`.
            e if e >= n - 1 => write!(f, "{}", v.abs() as u64),
            e if e >= 0 => {
                let point = e as usize + 1;
                digits().take(point).try_for_each(|c| f.write_char(c))?;
                f.write_char('.')?;
                digits().skip(point).try_for_each(|c| f.write_char(c))
            }
            e => {
                f.write_str("0.")?;
                (0..-e - 1).try_for_each(|_| f.write_char('0'))?;
                digits().try_for_each(|c| f.write_char(c))
            }
        }
    }
}

/// The shortest digits of a finite, non-negative `v`, with no leading or trailing zero, and the
/// power of ten of the first digit: `("2315781", -5)` for `2.315781e-05`, `("0", 0)` for zero.
fn shortest_digits<const N: usize>(
    v: f32,
    buf: &mut StackStr<N>,
) -> Result<(&str, i32), fmt::Error> {
    let mut ryu = ryu::Buffer::new();
    // `ryu` writes `100000.0`, `0.001` or `1.5e-7`.
    let text = ryu.format_finite(v);
    let (mantissa, exp) = text.split_once('e').unwrap_or((text, "0"));
    let exp: i32 = exp.parse().map_err(|_| fmt::Error)?;
    let (int, frac) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    buf.write_str(int)?;
    buf.write_str(frac)?;

    let all = buf.as_str();
    let significant = all.trim_start_matches('0');
    let leading = (all.len() - significant.len()) as i32;
    match significant.trim_end_matches('0') {
        "" => Ok(("0", 0)),
        digits => Ok((digits, int.len() as i32 - 1 - leading + exp)),
    }
}

/// A fixed-capacity string on the stack. A write past its capacity is a [`fmt::Error`].
struct StackStr<const N: usize> {
    buf: [u8; N],
    len: usize,
}

impl<const N: usize> StackStr<N> {
    pub fn new() -> Self {
        Self {
            buf: [0; N],
            len: 0,
        }
    }

    pub fn as_str(&self) -> &str {
        // Every write appends entire `&str`'s, so the first `len` bytes are always UTF-8.
        std::str::from_utf8(&self.buf[..self.len]).unwrap_or_default()
    }
}

impl<const N: usize> fmt::Write for StackStr<N> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let end = self.len + s.len();
        self.buf
            .get_mut(self.len..end)
            .ok_or(fmt::Error)?
            .copy_from_slice(s.as_bytes());
        self.len = end;
        Ok(())
    }
}
