#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum NodeKind {
    Object,
    Struct,
    Property,
    Value,
}
