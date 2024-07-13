use crate::core::animation::{joint, Joint};
use glam::Mat4;
use std::path::Iter;
use std::process::id;

pub struct Builder {
    pub name: String,
    pub flags: u16,
    pub is_influence: bool,
    pub radius: f32,
    pub local_transform: Mat4,
    pub inverse_bind_transform: Mat4,
    pub children: Vec<Box<Builder>>,
}

impl Builder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            flags: 0,
            is_influence: false,
            radius: 2.1,
            local_transform: Default::default(),
            inverse_bind_transform: Default::default(),
            children: vec![],
        }
    }

    pub fn with_flags(mut self, flags: u16) -> Self {
        self.flags = flags;
        self
    }

    pub fn with_influence(mut self, is_influence: bool) -> Self {
        self.is_influence = is_influence;
        self
    }
    pub fn with_local_transform(mut self, local_transform: Mat4) -> Self {
        self.local_transform = local_transform;
        self
    }
    pub fn with_inverse_bind_transform(mut self, inverse_bind_transform: Mat4) -> Self {
        self.inverse_bind_transform = inverse_bind_transform;
        self
    }

    pub fn with_children<const N: usize>(mut self, children: [impl Into<Box<Builder>>; N]) -> Self {
        self.children.extend(children.map(|c| c.into()));
        self
    }

    pub fn build(self, id: i16, parent_id: i16) -> (Joint, Vec<Box<Builder>>) {
        (
            Joint::new(
                self.name,
                self.flags,
                id,
                parent_id,
                self.radius,
                self.local_transform,
                self.inverse_bind_transform,
            ),
            self.children,
        )
    }
}
