use crate::core::animation::AnimationAsset;

mod read;
mod write;

#[derive(Clone, Debug)]
pub struct Uncompressed {}

impl From<Uncompressed> for AnimationAsset {
    fn from(val: Uncompressed) -> Self {
        AnimationAsset::Uncompressed(val)
    }
}
