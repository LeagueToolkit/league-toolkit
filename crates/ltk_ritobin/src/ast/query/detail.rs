#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum AstObjectDetail {
    Node,
    PathHash,
    Trivia,
}
impl AstObjectDetail {
    pub fn is_node(&self) -> bool {
        matches!(self, Self::Node)
    }
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum AstStructDetail {
    Node,
    ClassHash,
    Trivia,
}
impl AstStructDetail {
    pub fn is_node(&self) -> bool {
        matches!(self, Self::Node)
    }
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum AstPropertyDetail {
    Node,
    Name,
    TypeExpr,
    Trivia,
}
impl AstPropertyDetail {
    pub fn is_node(&self) -> bool {
        matches!(self, Self::Node)
    }
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum NodeDetail {
    Object(AstObjectDetail),
    Struct(AstStructDetail),
    Property(AstPropertyDetail),
    Value,
}
impl NodeDetail {
    pub fn is_node(&self) -> bool {
        match self {
            NodeDetail::Object(d) => d.is_node(),
            NodeDetail::Struct(d) => d.is_node(),
            NodeDetail::Property(d) => d.is_node(),
            NodeDetail::Value => true,
        }
    }
}

impl From<AstObjectDetail> for NodeDetail {
    fn from(value: AstObjectDetail) -> Self {
        Self::Object(value)
    }
}
impl From<AstStructDetail> for NodeDetail {
    fn from(value: AstStructDetail) -> Self {
        Self::Struct(value)
    }
}
impl From<AstPropertyDetail> for NodeDetail {
    fn from(value: AstPropertyDetail) -> Self {
        Self::Property(value)
    }
}
