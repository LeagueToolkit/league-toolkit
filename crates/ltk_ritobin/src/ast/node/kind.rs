#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum NodeKind {
    RootEntry,
    Object,
    Property,
    Value,
}
