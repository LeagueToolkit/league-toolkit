use std::collections::HashMap;

mod object;
pub use object::*;

mod read;
mod write;

#[cfg(test)]
mod tests;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, PartialEq)]
/// The top level tree of a bin file
pub struct BinTree {
    pub is_override: bool,
    pub version: u32,

    pub objects: HashMap<u32, BinTreeObject>,
    /// List of other property bins we depend on.
    ///
    /// Property bins can depend on other property bins in a similar fashion to importing code libraries
    pub dependencies: Vec<String>,

    data_overrides: Vec<()>,
}

impl BinTree {
    pub fn new(
        objects: impl IntoIterator<Item = BinTreeObject>,
        dependencies: impl IntoIterator<Item = String>,
    ) -> Self {
        Self {
            version: 3,
            is_override: false,
            objects: objects
                .into_iter()
                .map(|o: BinTreeObject| (o.path_hash, o))
                .collect(),
            dependencies: dependencies.into_iter().collect(),
            data_overrides: Vec::new(),
        }
    }
}
