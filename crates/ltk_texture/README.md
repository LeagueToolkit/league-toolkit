# ltk_texture

Decoding and encoding for League of Legends textures: the proprietary **`.tex`** format and **DDS**.

`.tex` is a thin container around block-compressed (or raw BGRA8) pixel data, with mipmaps stored smallest-first. This crate parses and writes the container, decodes every format the game ships, and can encode new textures from any `image::RgbaImage`.

Both 2D textures and volume (3D) textures are supported - a handful of map WADs ship `ResourceType::VolumeTexture` files whose `depth` z-slices are stored sequentially per mip. `decode_mipmap` decodes slice 0; `decode_mipmap_slice(level, slice)` decodes the rest.

## Feature flags

- `intel-tex`: enables BC1/BC3/BC7 encoding via [`intel_tex_2`](https://crates.io/crates/intel_tex_2). Decoding never requires it.

## Supported `.tex` formats

| ID | Format | Decode | Encode |
|-----|-------------------|--------|-----------------------|
| 1 | ETC1 | ✅ | ❌ |
| 2, 3 | ETC2/EAC | ✅ | ❌ |
| 10, 11 | BC1 | ✅ | ✅ (`intel-tex`) |
| 12 | BC3 | ✅ | ✅ (`intel-tex`) |
| 13 | BC7 (`BC7_UNORM_SRGB`) | ✅ | ✅ (`intel-tex`) |
| 14 | BC5 (`BC5_SNORM`) | ✅ | ❌ |
| 20 | Uncompressed BGRA8 | ✅ | ✅ |

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

Encoding to the block-compressed formats requires the `intel-tex` feature (ISPC texture compressor bindings):

```rust
use ltk_texture::Tex;
use ltk_texture::tex::{EncodeOptions, Format, MipmapFilter};
use std::fs::File;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let img = image::open("input.png")?;
    let tex = Tex::encode_dynamic_image(
        img,
        EncodeOptions::new(Format::Bc7)
            .with_mipmaps()
            .with_mipmap_filter(MipmapFilter::Lanczos3),
    )?;
    tex.write(&mut File::create("output.tex")?)?;

    Ok(())
}
```

BC1/BC3 encodes apply Floyd–Steinberg dithering toward RGB565 to reduce banding; BC7 encodes from the full 8-bit data.

## Related crates

- [`ltk_wad`](../ltk_wad): WAD archives, where game textures actually live.
- [`league-toolkit`](../../): umbrella crate that re-exports everything behind feature flags.

## License

Licensed under either of MIT or Apache-2.0 at your option.
