use std::ops::{Index, IndexMut};

use bumpalo::collections;

use crate::{parse::Token, Node};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct NodeId(pub(crate) u32);

impl<'arena> Index<NodeId> for [Node<'arena>] {
    type Output = Node<'arena>;

    fn index(&self, index: NodeId) -> &Self::Output {
        &self[index.0 as usize]
    }
}

impl<'arena> IndexMut<NodeId> for [Node<'arena>] {
    fn index_mut(&mut self, index: NodeId) -> &mut Self::Output {
        &mut self[index.0 as usize]
    }
}

impl<'arena> Index<NodeId> for collections::Vec<'arena, Node<'arena>> {
    type Output = Node<'arena>;

    fn index(&self, index: NodeId) -> &Self::Output {
        &self[index.0 as usize]
    }
}

impl<'arena> IndexMut<NodeId> for collections::Vec<'arena, Node<'arena>> {
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

impl<'arena> Index<TokenId> for collections::Vec<'arena, Token> {
    type Output = Token;

    fn index(&self, index: TokenId) -> &Self::Output {
        &self[index.0 as usize]
    }
}

impl<'arena> IndexMut<TokenId> for collections::Vec<'arena, Token> {
    fn index_mut(&mut self, index: TokenId) -> &mut Self::Output {
        &mut self[index.0 as usize]
    }
}
