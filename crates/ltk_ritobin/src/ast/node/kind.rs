#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum NodeKind {
    Root,
    RootEntry,
    Object,
    Property,
    Value,
}
