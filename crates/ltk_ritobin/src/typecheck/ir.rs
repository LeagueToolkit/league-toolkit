use ltk_meta::{traits::PropertyExt as _, PropertyValueEnum};

use crate::parse::Span;

#[derive(Debug, Clone)]
pub struct IrEntry {
    pub key: PropertyValueEnum<Span>,
    pub value: PropertyValueEnum<Span>,
    /// Span of the type expression this entry's value type came from, if any.
    pub type_span: Option<Span>,
}

#[derive(Debug, Clone)]
pub struct IrListItem(pub PropertyValueEnum<Span>);

#[derive(Debug, Clone)]
pub enum IrItem {
    Entry(IrEntry),
    ListItem(IrListItem),
}

impl IrItem {
    pub fn is_entry(&self) -> bool {
        matches!(self, Self::Entry { .. })
    }

    pub fn as_entry(&self) -> Option<&IrEntry> {
        match self {
            IrItem::Entry(i) => Some(i),
            _ => None,
        }
    }
    pub fn is_list_item(&self) -> bool {
        matches!(self, Self::ListItem { .. })
    }
    pub fn as_list_item(&self) -> Option<&IrListItem> {
        match self {
            IrItem::ListItem(i) => Some(i),
            _ => None,
        }
    }
    pub fn value(&self) -> &PropertyValueEnum<Span> {
        match self {
            IrItem::Entry(i) => &i.value,
            IrItem::ListItem(i) => &i.0,
        }
    }
    pub fn value_mut(&mut self) -> &mut PropertyValueEnum<Span> {
        match self {
            IrItem::Entry(i) => &mut i.value,
            IrItem::ListItem(i) => &mut i.0,
        }
    }

    /// Span of the whole item, an entry's key through its value.
    ///
    /// An [`IrListItem`] has no key, so there it is just the value. A parent that rejects an item
    /// rejects all of it, so this is what a diagnostic about the item underlines.
    pub fn span(&self) -> Span {
        match self {
            IrItem::Entry(IrEntry { key, value, .. }) => {
                let (key, value) = (*key.meta(), *value.meta());
                // a recovered tree can hand us a value that starts before its own key
                Span::new(key.start, value.end.max(key.end))
            }
            IrItem::ListItem(IrListItem(value)) => *value.meta(),
        }
    }

    /// Span of the type expression this item's type came from, if any.
    ///
    /// An [`IrListItem`] takes its type from its parent's subtype rather than declaring one, so it
    /// never has a type expression of its own to point at.
    pub fn type_span(&self) -> Option<Span> {
        match self {
            IrItem::Entry(entry) => entry.type_span,
            IrItem::ListItem(_) => None,
        }
    }

    pub fn into_value(self) -> PropertyValueEnum<Span> {
        match self {
            IrItem::Entry(i) => i.value,
            IrItem::ListItem(i) => i.0,
        }
    }
}
