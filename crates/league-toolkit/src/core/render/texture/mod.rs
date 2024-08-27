pub mod format;

mod read;

#[derive(Debug)]
pub enum CompressedTexture {
    Dds(ddsfile::Dds),
    Tex(image_dds::Surface<Vec<u8>>),
}

#[derive(Debug)]
pub enum UncompressedTexture {
    Dds(ddsfile::Dds),
    Tex(image_dds::SurfaceRgba8<Vec<u8>>),
}
