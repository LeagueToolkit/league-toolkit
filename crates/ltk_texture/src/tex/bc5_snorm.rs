//! Signed-normalized BC5 (`BC5_SNORM`) decoding.
//!
//! Neither `image_dds` (via `bcdec_rs`) nor `texture2ddecoder` implement the *signed* variant
//! of BC4/BC5 — both decode the endpoints as unsigned bytes, which produces garbage for SNORM
//! data (wrong palette values *and* wrong 6-vs-8 entry mode selection, since that comparison is
//! done in the signed domain). This module implements the D3D11 spec (§19.5.2) directly.

/// Decode a BC5_SNORM surface to interleaved RG8 SNORM data (2 bytes per pixel, each byte
/// an `i8` bit pattern in `[-127, 127]`).
pub(crate) fn decode_bc5_snorm(data: &[u8], width: usize, height: usize) -> Vec<u8> {
    let blocks_x = width.div_ceil(4);
    let blocks_y = height.div_ceil(4);
    debug_assert_eq!(data.len(), blocks_x * blocks_y * 16);

    let mut rg = vec![0u8; width * height * 2];

    for (block_i, block) in data.chunks_exact(16).enumerate() {
        let bx = (block_i % blocks_x) * 4;
        let by = (block_i / blocks_x) * 4;

        let red = decode_bc4_snorm_block(block[..8].try_into().unwrap());
        let green = decode_bc4_snorm_block(block[8..].try_into().unwrap());

        for ty in 0..4 {
            for tx in 0..4 {
                let (x, y) = (bx + tx, by + ty);
                if x >= width || y >= height {
                    continue;
                }
                let i = (y * width + x) * 2;
                rg[i] = red[ty * 4 + tx] as u8;
                rg[i + 1] = green[ty * 4 + tx] as u8;
            }
        }
    }

    rg
}

/// Decode a single 8-byte BC4_SNORM block into 16 texels in `[-127, 127]`
fn decode_bc4_snorm_block(block: [u8; 8]) -> [i8; 16] {
    let e0 = block[0] as i8;
    let e1 = block[1] as i8;

    // Endpoint *values* clamp -128 to -127 so the range is symmetric around 0,
    // but mode selection below compares the raw (pre-clamp) endpoints - this matches
    // DirectXTex's BC4_SNORM::DecodeFromIndex
    let r0 = e0.max(-127) as f32;
    let r1 = e1.max(-127) as f32;

    // Mode selection compares the raw endpoints in the *signed* domain
    let palette: [f32; 8] = if e0 > e1 {
        std::array::from_fn(|i| match i {
            0 => r0,
            1 => r1,
            i => (r0 * (8 - i) as f32 + r1 * (i - 1) as f32) / 7.0,
        })
    } else {
        std::array::from_fn(|i| match i {
            0 => r0,
            1 => r1,
            6 => -127.0,
            7 => 127.0,
            i => (r0 * (6 - i) as f32 + r1 * (i - 1) as f32) / 5.0,
        })
    };

    // 16 3-bit palette indices packed LSB-first into the remaining 6 bytes
    let indices = u64::from_le_bytes(block) >> 16;

    std::array::from_fn(|texel| palette[(indices >> (texel * 3)) as usize & 0b111].round() as i8)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bc4_block(e0: u8, e1: u8, indices: [u8; 16]) -> [u8; 8] {
        let mut packed = 0u64;
        for (i, &index) in indices.iter().enumerate() {
            assert!(index < 8);
            packed |= (index as u64) << (i * 3);
        }
        let bits = packed.to_le_bytes();
        [e0, e1, bits[0], bits[1], bits[2], bits[3], bits[4], bits[5]]
    }

    #[test]
    fn endpoints_map_to_full_range() {
        // e0 = 127 (1.0), e1 = -127 (-1.0); e0 > e1 → 8-entry mode
        let block = bc4_block(127, 0x81, [0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1]);
        let texels = decode_bc4_snorm_block(block);
        for pair in texels.chunks_exact(2) {
            assert_eq!(pair, [127, -127]);
        }
    }

    #[test]
    fn neg_128_is_clamped_to_neg_127() {
        let block = bc4_block(0x80, 0x80, [0; 16]);
        assert_eq!(decode_bc4_snorm_block(block), [-127; 16]);
    }

    #[test]
    fn mode_selection_uses_raw_endpoints_not_clamped() {
        // e0 = -127, e1 = -128: raw comparison -127 > -128 selects the 8-entry mode,
        // where every palette entry interpolates between -1.0 and -1.0.
        // Comparing *clamped* endpoints (-127 !> -127) would wrongly select the 6-entry
        // mode, whose index 7 is the constant +1.0. (Matches DirectXTex behavior.)
        let block = bc4_block(0x81, 0x80, [7; 16]);
        assert_eq!(decode_bc4_snorm_block(block), [-127; 16]);
    }

    #[test]
    fn zero_stays_zero() {
        let block = bc4_block(0, 0x81, [0; 16]);
        assert_eq!(decode_bc4_snorm_block(block), [0; 16]);
    }

    #[test]
    fn six_entry_mode_has_explicit_min_max() {
        // e0 = -127 <= e1 = 127 → 6-entry mode; indices 6 and 7 are the constants -1.0 / 1.0
        let block = bc4_block(0x81, 127, [6, 7, 6, 7, 6, 7, 6, 7, 6, 7, 6, 7, 6, 7, 6, 7]);
        let texels = decode_bc4_snorm_block(block);
        for pair in texels.chunks_exact(2) {
            assert_eq!(pair, [-127, 127]);
        }
    }

    #[test]
    fn eight_entry_mode_interpolates() {
        // e0 = 127, e1 = -127 → 8-entry mode.
        // Index 2 = (6 * 127 + 1 * -127) / 7 = 635/7 ≈ 90.7 → 91
        let block = bc4_block(127, 0x81, [2; 16]);
        assert_eq!(decode_bc4_snorm_block(block), [91; 16]);
    }

    #[test]
    fn decodes_rg_channels_and_partial_blocks() {
        // One block, but a 2x2 surface — edge texels outside the surface must be skipped
        let mut block = [0u8; 16];
        block[..8].copy_from_slice(&bc4_block(127, 0x81, [0; 16])); // red = 1.0
        block[8..].copy_from_slice(&bc4_block(0x81, 127, [0; 16])); // green = -1.0

        let rg = decode_bc5_snorm(&block, 2, 2);
        assert_eq!(rg.len(), 2 * 2 * 2);
        for pixel in rg.chunks_exact(2) {
            assert_eq!(pixel, [127i8 as u8, -127i8 as u8]);
        }
    }
}
