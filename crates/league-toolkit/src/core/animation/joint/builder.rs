use glam::Mat4;

pub struct Builder {
    name: String,
    flags: u16,
    is_influence: bool,
    parent: Option<()>,
    radius: f32,
    local_transform: Mat4,
    inverse_bind_transform: Mat4,
    children: Vec<()>,
}

impl Builder {
    pub fn new(name: String) -> Self {
        Self {
            name,
            flags: 0,
            is_influence: false,
            parent: None,
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
    pub fn with_local_transform(mut self, is_influence: bool) -> Self {
        self.is_influence = is_influence;
        self
    }
    pub fn with_inverse_bind_transform(mut self, is_influence: bool) -> Self {
        self.is_influence = is_influence;
        self
    }

    pub fn new_child(&mut self, name: String) -> Self {
        let child = Self::new(name);
        // self.children.push(child);
        child
    }
}