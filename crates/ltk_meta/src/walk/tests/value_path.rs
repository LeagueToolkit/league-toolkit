//! `ValuePath` from the walk, `value-walk.md` section 7: the trail's owned address, the keys it
//! decodes, and the client path it round-trips through `Bin::resolve`.

use std::collections::HashMap;

use ltk_hash::Hash as _;

use super::*;
use crate::path::{MapKey, UnnameableKind, ValuePath, ValueSegment};
use crate::walk::ChildSegment;

/// At one node: the trail's hash form, the owned address's hash form, the address's class
/// context and the trail's.
type Address = (String, String, Vec<Option<BinHash>>, Vec<BinHash>);

/// An [`Address`] at every node. `Node::value_path` is checked against the trail's as it goes.
#[derive(Default)]
struct Addresses(Vec<Address>);

impl<'a, V: TreeValue<'a>> Visitor<'a, V> for Addresses {
    type Error = Error;

    fn enter_node(&mut self, node: &Node<'_, 'a, V>) -> Result<Visit, Error> {
        let path = node.trail().to_value_path()?;
        assert_eq!(node.value_path()?, path);
        self.0.push((
            node.trail().to_string(),
            path.to_string(),
            path.fields().map(|(_, class)| class).collect(),
            node.trail().classes().to_vec(),
        ));
        Ok(Visit::Continue)
    }
}

#[test]
fn the_owned_address_renders_as_the_trail_does_at_every_node() {
    for bin in [fixture(), keyed_fixture()] {
        let [(owned, _), (viewed, _)] = walk_both(&bin, Addresses::default);
        assert_eq!(owned.0, viewed.0);
        for (trail, path, classes, trail_classes) in &owned.0 {
            assert_eq!(trail, path);
            let known: Vec<_> = trail_classes.iter().copied().map(Some).collect();
            assert_eq!(classes, &known, "{trail}");
        }
    }
}

/// Every key of the root's maps, decoded, with the value it came from.
#[derive(Default)]
struct Keys(Vec<(u32, MapKey, PropertyValueEnum)>);

impl<'a, V: TreeValue<'a>> Visitor<'a, V> for Keys {
    type Error = Error;

    fn enter_property(
        &mut self,
        field: BinHash,
        value: V,
        _node: &Node<'_, 'a, V>,
    ) -> Result<Visit, Error> {
        if value.kind() == Kind::Map {
            for child in value.children()? {
                let (ChildSegment::Key(key), _) = child? else {
                    panic!("a map child is a key");
                };
                self.0.push((field.0, key.map_key()?, key.to_value()?));
            }
        }
        if value.kind() == Kind::Container {
            for child in value.children()? {
                let (_, item) = child?;
                assert!(matches!(
                    item.map_key(),
                    Err(Error::InvalidKeyType(_)) | Ok(_)
                ));
            }
        }
        Ok(Visit::Continue)
    }
}

#[test]
fn map_keys_decode_the_same_over_both_trees_and_round_trip() {
    let [(owned, _), (viewed, _)] = walk_both(&fixture(), Keys::default);
    assert_eq!(owned.0, viewed.0);
    for kind in key_kinds() {
        let field = F_KEYS + kind as u32;
        let (_, key, value) = owned.0.iter().find(|(f, ..)| *f == field).unwrap();
        assert_eq!(key.kind(), kind);
        assert_eq!(&key.to_value(), value, "{kind:?}");
        assert_eq!(value, &leaf_of(kind));
    }
}

#[test]
fn a_value_no_map_is_keyed_by_is_not_a_key() {
    for kind in [Kind::ObjectLink, Kind::BitBool] {
        let value = leaf_of(kind);
        assert!(matches!(
            (&value).map_key(),
            Err(Error::InvalidKeyType(k)) if k == kind
        ));
    }
    let node = PropertyValueEnum::Struct(node(C2, vec![]));
    assert!(matches!(
        (&node).map_key(),
        Err(Error::InvalidKeyType(Kind::Struct))
    ));
}

/// The fixture with every field hash replaced by the hash of a name, and the table naming them.
fn named_fixture() -> (Bin, HashMap<BinHash, String>) {
    fn name(field: BinHash) -> String {
        format!("f{:x}", field.0)
    }
    fn rename(value: &mut PropertyValueEnum, names: &mut HashMap<BinHash, String>) {
        match value {
            PropertyValueEnum::Struct(s) | PropertyValueEnum::Embedded(Embedded(s)) => {
                rename_properties(&mut s.properties, names);
            }
            PropertyValueEnum::Container(c)
            | PropertyValueEnum::UnorderedContainer(UnorderedContainer(c)) => {
                for item in c.items_mut() {
                    rename(item, names);
                }
            }
            PropertyValueEnum::Optional(o) => {
                if let Some(slot) = o.slot() {
                    rename(slot.into_inner(), names);
                }
            }
            PropertyValueEnum::Map(m) => {
                for (_, value) in m.entries_mut() {
                    rename(value, names);
                }
            }
            _ => {}
        }
    }
    fn rename_properties(
        properties: &mut indexmap::IndexMap<BinHash, PropertyValueEnum>,
        names: &mut HashMap<BinHash, String>,
    ) {
        *properties = std::mem::take(properties)
            .into_iter()
            .map(|(field, mut value)| {
                rename(&mut value, names);
                let text = name(field);
                let hash = BinHash::hash_str(&text);
                names.insert(hash, text);
                (hash, value)
            })
            .collect();
    }

    let mut names = HashMap::new();
    let mut bin = fixture();
    let keyed = keyed_fixture();
    let object = bin.objects.get_mut(&BinHash(OBJECT)).unwrap();
    for (field, value) in &keyed.objects[&BinHash(OBJECT)].properties {
        object
            .properties
            .insert(BinHash(field.0 + 0x100), value.clone());
    }
    rename_properties(&mut object.properties, &mut names);
    (bin, names)
}

/// At every node below the root and every leaf property, the client path `to_property_path`
/// spells, or why it cannot.
struct RoundTrip<'n> {
    names: &'n HashMap<BinHash, String>,
    positions: Vec<(
        ValuePath,
        Result<crate::path::PropertyPath, crate::path::Unnameable>,
        PropertyValueEnum,
    )>,
}

impl<'a> Visitor<'a, &'a PropertyValueEnum> for RoundTrip<'_> {
    type Error = Error;

    fn enter_node(&mut self, node: &Node<'_, 'a, &'a PropertyValueEnum>) -> Result<Visit, Error> {
        if !node.is_root() {
            let path = node.value_path()?;
            let value = node.inner().to_struct()?;
            self.positions.push((
                path.clone(),
                path.to_property_path(self.names),
                value.into(),
            ));
        }
        Ok(Visit::Continue)
    }

    fn enter_property(
        &mut self,
        field: BinHash,
        value: &'a PropertyValueEnum,
        node: &Node<'_, 'a, &'a PropertyValueEnum>,
    ) -> Result<Visit, Error> {
        if !value.can_contain_node()? {
            let mut path = node.value_path()?;
            path.push_field(field, node.class_hash());
            self.positions.push((
                path.clone(),
                path.to_property_path(self.names),
                value.clone(),
            ));
        }
        Ok(Visit::Continue)
    }
}

#[test]
fn every_nameable_position_resolves_through_its_client_path() {
    let (bin, names) = named_fixture();
    let mut round_trip = RoundTrip {
        names: &names,
        positions: Vec::new(),
    };
    bin.walk(&mut round_trip).unwrap();

    let mut resolved = 0;
    let mut unnameable = 0;
    for (path, client, value) in round_trip.positions {
        match client {
            Ok(client) => {
                let at = bin
                    .resolve(OBJECT, &client)
                    .unwrap_or_else(|e| panic!("{client}: {e}"));
                let at = match (at, &value) {
                    (PropertyValueEnum::Embedded(Embedded(s)), PropertyValueEnum::Struct(_)) => {
                        PropertyValueEnum::Struct(s.clone())
                    }
                    (at, _) => at.clone(),
                };
                assert_eq!(at, value, "{path} as {client}");
                resolved += 1;
            }
            Err(error) => {
                unnameable += 1;
                let ValueSegment::Key(key) = &path.segments()[error.segment] else {
                    panic!("{path}: only a key is unnameable here, got {error}");
                };
                assert_eq!(error.kind, UnnameableKind::Key(key.kind()), "{path}");
                assert!(
                    matches!(
                        key.kind(),
                        Kind::None
                            | Kind::Vector2
                            | Kind::Vector3
                            | Kind::Vector4
                            | Kind::Matrix44
                            | Kind::Color
                    ),
                    "{path}"
                );
            }
        }
    }
    // 9 nested nodes of the fixture and 13 keyed nodes whose key has a literal; 46 leaf
    // properties: 43 on the root and one on each of C2, C3 and C9. The 6 other keyed nodes sit
    // under a `None`, vector, matrix or colour key.
    assert_eq!((resolved, unnameable), (68, 6));
}
