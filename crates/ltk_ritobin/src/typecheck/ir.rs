use ltk_meta::PropertyValueEnum;

use crate::parse::Span;

#[derive(Debug, Clone)]
pub struct IrEntry {
    pub key: PropertyValueEnum<Span>,
    pub value: PropertyValueEnum<Span>,
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
    pub fn into_value(self) -> PropertyValueEnum<Span> {
        match self {
            IrItem::Entry(i) => i.value,
            IrItem::ListItem(i) => i.0,
        }
    }
}
