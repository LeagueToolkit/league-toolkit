use crate::core::animation::joint;

pub struct Builder {
    name: String,
    asset_name: String,
    root_joints: Vec<joint::Builder>,
}

impl Builder {
    pub fn new(name: impl Into<String>, asset_name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            asset_name: asset_name.into(),
            root_joints: vec![],
        }
    }
}