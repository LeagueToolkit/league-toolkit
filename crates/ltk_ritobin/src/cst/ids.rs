use std::ops::{Index, IndexMut};

use crate::{
    cst::Child,
    parse::{Error, Token},
    Cst, Node,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct NodeId(pub(crate) u32);

impl Index<NodeId> for [Node] {
    type Output = Node;

    fn index(&self, index: NodeId) -> &Self::Output {
        &self[index.0 as usize]
    }
}

impl IndexMut<NodeId> for [Node] {
    fn index_mut(&mut self, index: NodeId) -> &mut Self::Output {
        &mut self[index.0 as usize]
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct TokenId(pub(crate) u32);

impl Index<TokenId> for [Token] {
    type Output = Token;

    fn index(&self, index: TokenId) -> &Self::Output {
        &self[index.0 as usize]
    }
}

impl IndexMut<TokenId> for [Token] {
    fn index_mut(&mut self, index: TokenId) -> &mut Self::Output {
        &mut self[index.0 as usize]
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct ErrorId(pub(crate) u32);

impl Index<ErrorId> for [Error] {
    type Output = Error;

    fn index(&self, index: ErrorId) -> &Self::Output {
        &self[index.0 as usize]
    }
}

impl IndexMut<ErrorId> for [Error] {
    fn index_mut(&mut self, index: ErrorId) -> &mut Self::Output {
        &mut self[index.0 as usize]
    }
}

impl<'arena> Index<ErrorId> for collections::Vec<'arena, Error> {
    type Output = Error;

    fn index(&self, index: ErrorId) -> &Self::Output {
        &self[index.0 as usize]
    }
}

impl<'arena> IndexMut<ErrorId> for collections::Vec<'arena, Error> {
    fn index_mut(&mut self, index: ErrorId) -> &mut Self::Output {
        &mut self[index.0 as usize]
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Copy, Debug)]
pub struct ChildRange {
    pub(crate) start: u32,
    pub(crate) len: u32,
}
impl ChildRange {
    pub fn get<'a>(&self, cst: &'a Cst) -> &'a [Child] {
        let start = self.start as usize;
        let end = start + self.len as usize;
        &cst.children[start..end]
    }

    pub fn empty() -> Self {
        Self { start: 0, len: 0 }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Copy, Debug)]
pub struct ErrorRange {
    pub(crate) start: u32,
    pub(crate) len: u32,
}
impl ErrorRange {
    pub fn get<'a>(&self, cst: &'a Cst) -> &'a [Error] {
        let start = self.start as usize;
        let end = start + self.len as usize;
        &cst.errors[start..end]
    }

    pub fn empty() -> Self {
        Self { start: 0, len: 0 }
    }
}
