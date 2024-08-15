pub mod format;

mod read;

#[derive(Debug)]
pub enum Texture {
    Dds(ddsfile::Dds),
    Tex,
}
