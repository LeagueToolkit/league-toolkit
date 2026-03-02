# Matrix44 Transpose Bug Fix

## Problem
The text output for `transform: mtx44` was transposed. Translation value `-4500` appeared at `[row 1, col 3]` instead of `[row 3, col 1]`.

**Wrong (before):**
```
transform: mtx44 = {
    1, 0, 0, 0
    0, 1, 0, -4500    <-- wrong position
    0, 0, 1, 0
    0, 0, 0, 1
}
```

**Correct (after):**
```
transform: mtx44 = {
    1, 0, 0, 0
    0, 1, 0, 0
    0, 0, 1, 0
    0, -4500, 0, 1    <-- correct position
}
```

## Root Cause
`Matrix44Value` wrapped `glam::Mat4`, which stores data in column-major order internally. The reader (`read_mat4_row_major`) called `from_cols(...).transpose()` and the text writer called `v.0.transpose().to_cols_array()` — but these two transposes did not cancel out correctly when outputting to text, resulting in a transposed matrix.

The reference C++ ritobin simply stores the matrix as a raw `std::array<float, 16>` and writes it back directly. No column-major/row-major conversion needed.

## Fix
Stop using `glam::Mat4` for `Matrix44Value`. Use `[f32; 16]` instead — read 16 floats, store 16 floats, write 16 floats. No transpose logic needed.

### Changed Files

#### 1. `ltk_meta/src/property/value/primitives.rs`
- Replaced `impl_prim!(Matrix44Value, Mat4, [], mat4_row_major::<LE>, 0)` with a manual struct + impl
- `Matrix44Value` now wraps `[f32; 16]` instead of `Mat4`
- `ReadProperty::from_reader` reads 16 `f32` values sequentially
- `WriteProperty::to_writer` writes 16 `f32` values sequentially

#### 2. `ltk_ritobin/src/writer.rs` (line 225)
- `let arr = v.0.transpose().to_cols_array()` → `let arr = v.0`
- Since `v.0` is now `[f32; 16]` in binary order, just iterate directly

#### 3. `ltk_ritobin/src/parser.rs` (text parser)
- `parse_mtx44` return type: `Mat4` → `[f32; 16]`
- `Mat4::from_cols_array(&values).transpose()` → `values`

## Testing
Standalone test project at `test_matrix/`:
```bash
cd test_matrix && cargo run 2>/dev/null | grep -A 6 "mtx44"
```

Verified output matches reference `ritobin_cli.exe` output.
