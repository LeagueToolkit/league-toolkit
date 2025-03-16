use crate::core::animation::AnimationAsset;

mod frame;
mod read;
mod write;

pub use frame::*;

#[derive(Clone, Debug)]
pub struct Uncompressed {}

impl From<Uncompressed> for AnimationAsset {
    fn from(val: Uncompressed) -> Self {
        AnimationAsset::Uncompressed(val)
    }
}
