//! The canonical text: ritobin text as C++ ritobin writes it, every hash as hex.
//!
//! The layout is `bin_io_text_write.cpp`'s, at the indent `ritobin_cli` passes (4). A bin prints
//! byte for byte as `ritobin_cli -k -i bin -o text` prints it:
//!
//! - The roots are `type`, `version`, `linked` and `entries`, in that order. `linked` is absent
//!   below version 2.
//! - An empty container is `{}`. Any other container holds one item per line, and its closing `}`
//!   sits at the indent of the line that opened it. Nothing is folded to a line width.
//! - A vector and an `rgba` are `{ a, b, c }`. An `mtx44` is four lines of four.
//! - A pointer with class hash 0 is `null`.
//! - A hash is `0x` and 8 zero-padded hex digits, 16 for a `file`.
//! - A type with arguments is `list[string]`, `map[hash,embed]`: no space after the comma.
//! - An integer is decimal. A float is [`F32Text`].
//! - A string is quoted, with `\t \n \r \b \f \\ \"` escaped, `\xHH` for any other character below
//!   0x20, and every other character as is.

use std::fmt::{self, Write};

use indexmap::IndexMap;
use ltk_hash::BinHash;
use ltk_meta::{property::values, Bin, BinObject, PropertyValueEnum};

use crate::{escaping, PropertyValueExt as _};

/// An `f32` as `std::to_chars(float)` writes it, with no format argument.
///
/// The digits are the shortest that read back to the same float. The number is written fixed or
/// scientific, whichever is shorter, fixed on a tie. A scientific exponent has a sign and at least
/// two digits: `1e+05`, `2.315781e-05`, `1e-45`.
///
/// The non-finite values are MSVC's spellings: `inf`, `nan`, `nan(snan)` for a signalling NaN and
/// `-nan(ind)` for the negative NaN whose payload is only the quiet bit. A negative value carries a
/// `-`, `-0` included.
#[derive(Debug, Clone, Copy)]
pub struct F32Text(pub f32);

impl fmt::Display for F32Text {
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
///
/// The digits are Ryu's, the algorithm MSVC's `to_chars` runs: the fewest that read back to `v`,
/// of those the closest to `v`, and the even one of two equally close (`28936.812` for
/// 28936.8125).
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
    fn new() -> Self {
        Self {
            buf: [0; N],
            len: 0,
        }
    }

    fn as_str(&self) -> &str {
        // Every write appends a whole `&str`, so the bytes up to `len` are UTF-8.
        std::str::from_utf8(&self.buf[..self.len]).unwrap_or_default()
    }
}

impl<const N: usize> Write for StackStr<N> {
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

/// The indent of one level: 4 spaces, the indent `ritobin_cli` passes.
const INDENT: usize = 4;

/// Writes the canonical text. See the [module docs](self).
pub(crate) struct CanonicalWriter<'a, W: Write + ?Sized> {
    out: &'a mut W,
    indent: usize,
    written: usize,
}

impl<W: Write + ?Sized> Write for CanonicalWriter<'_, W> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.written += s.len();
        self.out.write_str(s)
    }
}

impl<'a, W: Write + ?Sized> CanonicalWriter<'a, W> {
    pub(crate) fn new(out: &'a mut W) -> Self {
        Self {
            out,
            indent: 0,
            written: 0,
        }
    }

    /// Writes `bin`, and returns the number of bytes written.
    pub(crate) fn bin(mut self, bin: &Bin) -> Result<usize, fmt::Error> {
        self.write_str("#PROP_text\ntype: string = \"PROP\"\n")?;
        writeln!(self, "version: u32 = {}", bin.version)?;
        if bin.version >= 2 {
            self.write_str("linked: list[string] = ")?;
            self.items(&bin.dependencies, |w, dep| w.string(dep))?;
            self.write_char('\n')?;
        }
        self.write_str("entries: map[hash,embed] = ")?;
        self.items(bin.objects.values(), |w, object| w.object(object))?;
        self.write_char('\n')?;
        Ok(self.written)
    }

    fn pad(&mut self) -> fmt::Result {
        (0..self.indent).try_for_each(|_| self.write_char(' '))
    }

    /// `{}` when `items` is empty. Otherwise `{`, each item on its own line one indent deeper,
    /// and `}` at the current indent.
    fn items<T>(
        &mut self,
        items: impl IntoIterator<Item = T>,
        mut item: impl FnMut(&mut Self, T) -> fmt::Result,
    ) -> fmt::Result {
        let mut items = items.into_iter().peekable();
        if items.peek().is_none() {
            return self.write_str("{}");
        }
        self.write_str("{\n")?;
        self.indent += INDENT;
        for value in items {
            self.pad()?;
            item(self, value)?;
            self.write_char('\n')?;
        }
        self.indent -= INDENT;
        self.pad()?;
        self.write_char('}')
    }

    fn object(&mut self, object: &BinObject) -> fmt::Result {
        self.hash(object.path_hash)?;
        self.write_str(" = ")?;
        self.class(object.class_hash, &object.properties)
    }

    fn class(
        &mut self,
        class_hash: BinHash,
        properties: &IndexMap<BinHash, PropertyValueEnum>,
    ) -> fmt::Result {
        write!(self, "0x{:08x} ", class_hash.0)?;
        self.items(properties, |w, (name, value)| w.field(*name, value))
    }

    fn field(&mut self, name: BinHash, value: &PropertyValueEnum) -> fmt::Result {
        write!(self, "0x{:08x}: {} = ", name.0, value.rito_type())?;
        self.value(value)
    }

    fn value(&mut self, value: &PropertyValueEnum) -> fmt::Result {
        use PropertyValueEnum as P;
        match value {
            P::None(_) => self.write_str("null"),
            P::Bool(b) => self.bool(**b),
            P::BitBool(b) => self.bool(**b),
            P::U8(n) => write!(self, "{}", **n),
            P::U16(n) => write!(self, "{}", **n),
            P::U32(n) => write!(self, "{}", **n),
            P::U64(n) => write!(self, "{}", **n),
            P::I8(n) => write!(self, "{}", **n),
            P::I16(n) => write!(self, "{}", **n),
            P::I32(n) => write!(self, "{}", **n),
            P::I64(n) => write!(self, "{}", **n),
            P::F32(n) => write!(self, "{}", F32Text(**n)),
            P::Vector2(v) => self.vector(&v.to_array()),
            P::Vector3(v) => self.vector(&v.to_array()),
            P::Vector4(v) => self.vector(&v.to_array()),
            P::Color(c) => write!(self, "{{ {}, {}, {}, {} }}", c.r, c.g, c.b, c.a),
            // ritobin text lists a matrix row by row, glam::Mat4 stores it column by column.
            P::Matrix44(m) => self.matrix(&m.transpose().to_cols_array()),
            P::String(s) => self.string(s),
            P::Hash(h) => self.hash(**h),
            P::ObjectLink(h) => self.hash(**h),
            P::WadChunkLink(h) => write!(self, "0x{:016x}", h.0),
            P::Container(c) | P::UnorderedContainer(values::UnorderedContainer(c)) => {
                self.items(c.items(), |w, item| w.value(item))
            }
            P::Struct(s) if s.class_hash.0 == 0 => self.write_str("null"),
            P::Struct(s) | P::Embedded(values::Embedded(s)) => {
                self.class(s.class_hash, &s.properties)
            }
            P::Optional(o) => self.items(o.value(), |w, item| w.value(item)),
            P::Map(m) => self.items(m.entries(), |w, (key, value)| {
                w.value(key)?;
                w.write_str(" = ")?;
                w.value(value)
            }),
        }
    }

    fn bool(&mut self, b: bool) -> fmt::Result {
        self.write_str(if b { "true" } else { "false" })
    }

    fn string(&mut self, s: &str) -> fmt::Result {
        self.write_char('"')?;
        escaping::escape_into(self, s)?;
        self.write_char('"')
    }

    fn hash(&mut self, hash: BinHash) -> fmt::Result {
        write!(self, "0x{:08x}", hash.0)
    }

    /// `{ a, b, c }`.
    fn vector(&mut self, values: &[f32]) -> fmt::Result {
        self.write_str("{ ")?;
        for (i, v) in values.iter().enumerate() {
            if i > 0 {
                self.write_str(", ")?;
            }
            write!(self, "{}", F32Text(*v))?;
        }
        self.write_str(" }")
    }

    /// Four rows of four, one indent deeper, and `}` at the current indent.
    fn matrix(&mut self, values: &[f32; 16]) -> fmt::Result {
        self.write_str("{\n")?;
        self.indent += INDENT;
        for row in values.chunks_exact(4) {
            self.pad()?;
            for (i, v) in row.iter().enumerate() {
                if i > 0 {
                    self.write_str(", ")?;
                }
                write!(self, "{}", F32Text(*v))?;
            }
            self.write_char('\n')?;
        }
        self.indent -= INDENT;
        self.pad()?;
        self.write_char('}')
    }
}

#[cfg(test)]
mod tests {
    use super::F32Text;

    #[test]
    fn floats_print_as_to_chars_writes_them() {
        for (v, text) in [
            (0.0f32, "0"),
            (-0.0, "-0"),
            (1.0, "1"),
            (0.5, "0.5"),
            (-2.25, "-2.25"),
            (100.0, "100"),
            (1e5, "1e+05"),
            (-1e5, "-1e+05"),
            (123456.0, "123456"),
            (1e-5, "1e-05"),
            (0.001, "0.001"),
            (1e-4, "1e-04"),
            (1.5e-7, "1.5e-07"),
            (2.315781e-05, "2.315781e-05"),
            (-8.742278e-08, "-8.742278e-08"),
            (3.4028235e38, "3.4028235e+38"),
            (f32::from_bits(1), "1e-45"),
            (0.1, "0.1"),
            (0.14117648, "0.14117648"),
            (1e10, "1e+10"),
        ] {
            assert_eq!(F32Text(v).to_string(), text, "{v:e}");
        }
    }

    #[test]
    fn non_finite_floats_print_with_msvc_spellings() {
        for (bits, text) in [
            (0x7f80_0000u32, "inf"),
            (0xff80_0000, "-inf"),
            (0x7fc0_0000, "nan"),
            (0xffc0_0000, "-nan(ind)"),
            (0x7fc0_0001, "nan"),
            (0xffc0_0001, "-nan"),
            (0x7f80_0001, "nan(snan)"),
            (0xff80_0001, "-nan(snan)"),
        ] {
            assert_eq!(F32Text(f32::from_bits(bits)).to_string(), text, "{bits:#x}");
        }
    }
}
