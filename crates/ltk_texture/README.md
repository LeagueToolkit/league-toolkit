# ltk_texture

Decoding and encoding for League of Legends textures: the proprietary **`.tex`** format and **DDS**.

`.tex` is a thin container around block-compressed (or raw BGRA8) pixel data, with mipmaps stored smallest-first. This crate parses and writes the container, decodes every format the game ships, and can encode new textures from any `image::RgbaImage`.

Both 2D textures and volume (3D) textures are supported - a handful of map WADs ship `ResourceType::VolumeTexture` files whose `depth` z-slices are stored sequentially per mip. `decode_mipmap` decodes slice 0; `decode_mipmap_slice(level, slice)` decodes the rest.

## Feature flags

- `intel-tex`: enables BC7 encoding via [`intel_tex_2`](https://crates.io/crates/intel_tex_2)'s ISPC kernels (x86/x86_64 only). BC1/BC3 encoding is always available through [`texpresso`](https://crates.io/crates/texpresso) (pure Rust, any target). Decoding never requires any feature.

## Supported `.tex` formats

| ID | Format | Decode | Encode |
|-----|-------------------|--------|-----------------------|
| 1 | ETC1 | ✅ | ❌ |
| 2, 3 | ETC2/EAC | ✅ | ❌ |
| 10, 11 | BC1 | ✅ | ✅ |
| 12 | BC3 | ✅ | ✅ |
| 13 | BC7 (`BC7_UNORM_SRGB`) | ✅ | ✅ (`intel-tex`) |
| 14 | BC5 (`BC5_SNORM`) | ✅ | ❌ |
| 20 | Uncompressed BGRA8 | ✅ | ✅ |
| 21 | Uncompressed RGBA16 half-float | ✅ | ✅ |
| 22 | Uncompressed RGBA32 float | ✅ | ✅ |

BC5_SNORM is decoded by this crate's own implementation, verified against DirectXTex and the D3D11 functional spec - general-purpose decoders (`image_dds`, `texture2ddecoder`) only implement the unsigned variant, which silently corrupts signed data.

## Decoding

```rust
use ltk_texture::Tex;
use std::fs::File;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let tex = Tex::from_reader(&mut File::open("texture.tex")?)?;
    println!("{}x{} {:?}, {} mips", tex.width, tex.height, tex.format, tex.mip_count);

    // Decode the full-resolution mip and save it as a PNG
    let surface = tex.decode_mipmap(0)?;
    surface.into_rgba_image()?.save("output.png")?;
    
    Ok(())
}
```

If you don't know whether a file is `.tex` or `.dds` (e.g. when pulling assets out of a WAD), use the format-agnostic `Texture`:

```rust
use ltk_texture::Texture;
use std::fs::File;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let texture = Texture::from_reader(&mut File::open("some_texture")?)?;
    let image = texture.decode_mipmap(0)?.into_rgba_image()?;

    Ok(())
}
```

## Decoded surfaces

`decode_mipmap` returns a `TexSurface`: tightly-packed, row-major pixel data tagged with the `PixelFormat` it naturally decodes to.

- Color formats (BC1/BC3/BC7) decode to `Rgba8Unorm`; ETC and raw data decode to `Bgra8Unorm`.
- BC5_SNORM decodes to `Rg8Snorm` with the signed data **intact** - nothing is lost to an RGBA remap.
- RGBA16F decodes to `Rgba16Float` (little-endian `half::f16` bit patterns; `half` is re-exported). The shipped files are shader textures holding values far outside `[0, 1]`, so use `as_pixels::<[half::f16; 4]>()` when the actual values matter - `into_rgba_image()` clamps.

`into_rgba_image()` is a *presentation* conversion: signed-normalized channels are remapped from `[-1, 1]` to `[0, 255]`, missing channels are filled with 0 (alpha with 255). When you need the real values - normal maps being the typical case - read them directly:

```rust
use ltk_texture::Tex;
use ltk_texture::tex::PixelFormat;
use std::fs::File;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let tex = Tex::from_reader(&mut File::open("normal_map.tex")?)?;

    let surface = tex.decode_mipmap(0)?;
    if surface.format == PixelFormat::Rg8Snorm {
        // typed access to the signed normal-map channels
        let pixels: &[[i8; 2]] = surface.as_pixels().unwrap();
        let [x, y] = pixels[0];
    }

    Ok(())
}
```

## Encoding

BC1/BC3 encoding always works (texpresso cluster fit, parallelized with rayon); BC7 additionally requires the `intel-tex` feature (ISPC texture compressor bindings):

```rust
use ltk_texture::Tex;
use ltk_texture::tex::{EncodeFormat, EncodeOptions, MipmapFilter};
use std::fs::File;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let img = image::open("input.png")?;
    let tex = Tex::encode_dynamic_image(
        img,
        EncodeOptions::new(EncodeFormat::Bc3 { weigh_colour_by_alpha: false })
            .with_mipmaps()
            .with_mipmap_filter(MipmapFilter::Lanczos3),
    )?;
    tex.write(&mut File::create("output.tex")?)?;

    Ok(())
}
```

For alpha-blended textures, `EncodeFormat::Bc3 { weigh_colour_by_alpha: true }` weighs each pixel's contribution to the BC1/BC3 endpoint fit by its alpha, which can significantly improve perceived quality.

Input dimensions don't need to be multiples of 4; partial edge blocks are handled correctly and the true dimensions go in the header.

For textures headed back into the game, keep the base dimensions a multiple of 4: D3D11 forbids block-compressed textures with a non-aligned top mip. Uncompressed formats (BGRA8, RGBA16F) have no such restriction.

## Related crates

- [`ltk_wad`](../ltk_wad): WAD archives, where game textures actually live.
- [`league-toolkit`](../../): umbrella crate that re-exports everything behind feature flags.

## License

Licensed under either of MIT or Apache-2.0 at your option.
