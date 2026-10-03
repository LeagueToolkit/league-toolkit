# ADR-0022: Ryu float digits

- **Status:** Accepted
- **Date:** 2026-10-02
- **Crates:** `ltk_ritobin`
- **Related:** #242, the rule as `ltk_ritobin::print::canonical::F32Text` states it
  (`crates/ltk_ritobin/src/print/canonical.rs`)

## Context and problem statement

C++ ritobin writes an `f32` with `std::to_chars(float)` and no format argument
(`bin_numconv.hpp`, `from_num`). `ltk_ritobin` prints a bin byte for byte as `ritobin_cli -k`
prints it, floats included.

`to_chars` writes the fewest digits that read back to the same float. Of the candidates with that
many digits it takes the closest to the value, and of two equally close candidates the even one.
MSVC's `to_chars` computes these digits with Ryu (Ulf Adams, 2018).

Rust's `{}` and `{:e}` also write the fewest digits, and resolve an exact tie differently:
-28936.8125 prints as `-28936.813` in Rust and as `-28936.812` through `to_chars`. A set of
70,065 chosen and random floats holds several such ties.

The `ryu` crate is a port of the same algorithm, by the author of `serde_json`. It has no
dependencies.

With `ryu`, 570,065 floats (500,000 random bit patterns and 70,065 chosen values) and 16,242
`PROP` bins from an installed client print byte-identical to `ritobin_cli` `368b413`.

## Decision drivers

- Byte parity with `ritobin_cli`, every float included.
- The digits come from the algorithm the reference implementation runs, not from a rule written
  to imitate it.
- A dependency stays small, and adds no transitive crate.

## Considered options

1. **The `ryu` crate** for the digits, and the fixed-or-scientific layout written in
   `ltk_ritobin`.
2. **Rust's `{:e}` with a re-rounding pass** - format the shortest digits, format again at that
   precision with ties to even, and keep the second when it reads back to the same float.
3. **A hand port of Ryu's `f2s`** into `ltk_ritobin`.

## Decision

**`F32Text` takes its digits and exponent from `ryu::Buffer::format_finite`, and lays them out
fixed or scientific as `to_chars` does.** `ryu` is a workspace dependency, and `ltk_ritobin` the
crate that uses it.

The layout rules - the shorter of fixed and scientific, fixed on a tie, an exponent of a sign and
at least two digits, an integer written with its exact digits, MSVC's spellings of the non-finite
values - are `F32Text`'s, in `ltk_ritobin`.

## Consequences

- **Positive:** a float prints as the reference prints it, by the reference's algorithm. One call
  per float, with no second format and no reparse.
- **Negative:** one more crate in the tree. `ryu` writes its own text format (`100000.0`,
  `1.5e-7`), and `F32Text` parses the digits back out of it - a dependence on that text format,
  which `ryu` does not promise across major versions.
- **Revisit when:** the standard library exposes the shortest digits with ties to even, or `ryu`
  changes its output format.

## Pros and cons of the options

### The `ryu` crate

- Good: the same algorithm as MSVC's `to_chars`. Validated against `ritobin_cli` on 570,065
  floats and 16,242 bins.
- Good: no dependencies, small.
- Bad: a new dependency, and its output text is parsed for the digits.

### `{:e}` with a re-rounding pass

- Good: no dependency. It also matched `ritobin_cli` on the 70,065 chosen and random floats.
- Bad: two formats and a parse per float.
- Bad: the rule it implements - correctly rounded at the shortest length, ties to even, when that
  reads back - matches Ryu by argument, not by construction. A float near a power of two, where
  the rounding interval is asymmetric, is where the two can part.

### A hand port of `f2s`

- Good: no dependency, and the digits come out as integers with no text to parse.
- Bad: a few hundred lines of table-driven arithmetic to own and test, duplicating a maintained
  crate.
