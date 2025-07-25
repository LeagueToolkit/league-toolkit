use crate::AnimationAsset;

mod read;
mod write;

#[derive(Clone, Debug)]
pub struct Uncompressed {}

impl Into<AnimationAsset> for Uncompressed {
    fn into(self) -> AnimationAsset {
        AnimationAsset::Uncompressed(self)
    }
}
