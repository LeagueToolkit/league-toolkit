//! The mutable walk of `value-walk.md` section 5.3: edits land where the callbacks leave them.

use super::*;
use crate::walk::TrailStep;

/// Records the node trails a mutable walk visits, and runs `edit` at every callback first.
struct Editor<F> {
    nodes: Vec<(String, u32)>,
    exited: Vec<u32>,
    edit: F,
}

enum At<'e, 't> {
    EnterNode(&'e mut NodeMut<'t>),
    EnterProperty(&'e mut PropertyMut<'t>),
    ExitProperty(&'e mut PropertyMut<'t>),
}

impl<F: FnMut(At<'_, '_>)> Editor<F> {
    fn new(edit: F) -> Self {
        Self {
            nodes: Vec::new(),
            exited: Vec::new(),
            edit,
        }
    }
}

impl<F: FnMut(At<'_, '_>)> VisitorMut for Editor<F> {
    type Error = Error;

    fn enter_node(&mut self, node: &mut NodeMut<'_>) -> Result<Visit, Error> {
        (self.edit)(At::EnterNode(node));
        self.nodes
            .push((node.trail().to_string(), node.class_hash().0));
        Ok(Visit::Continue)
    }

    fn enter_property(&mut self, property: &mut PropertyMut<'_>) -> Result<Visit, Error> {
        (self.edit)(At::EnterProperty(property));
        Ok(Visit::Continue)
    }

    fn exit_property(&mut self, property: &mut PropertyMut<'_>) -> Result<Visit, Error> {
        (self.edit)(At::ExitProperty(property));
        self.exited.push(property.field().0);
        Ok(Visit::Continue)
    }
}

fn root(bin: &Bin) -> &BinObject {
    &bin.objects[&BinHash(OBJECT)]
}

fn reread(bin: &Bin) -> Bin {
    Bin::from_reader(&mut io::Cursor::new(bytes_of(bin))).expect("the edited bin reads")
}

#[test]
fn a_property_inserted_in_enter_node_is_walked_and_a_removed_one_is_not() {
    const F_ADDED: u32 = 0x0F00;
    let mut bin = fixture();
    let mut editor = Editor::new(|at| {
        if let At::EnterNode(node) = at {
            if node.is_root() {
                node.properties_mut().shift_remove(&BinHash(F_STRUCT));
                node.properties_mut()
                    .insert(BinHash(F_ADDED), node_value(C9));
            }
        }
    });
    assert_eq!(bin.walk_mut(&mut editor).unwrap(), WalkOutcome::Completed);

    assert!(editor.nodes.contains(&("00000f00".to_owned(), C9)));
    assert!(!editor
        .nodes
        .iter()
        .any(|(trail, _)| trail.starts_with("00000001")));
    assert!(!root(&bin).properties.contains_key(&BinHash(F_STRUCT)));
    assert_eq!(reread(&bin), bin);
}

fn node_value(class: u32) -> PropertyValueEnum {
    node(class, vec![(F_LEAF, values::I32::new(9).into())]).into()
}

#[test]
fn a_value_replaced_in_enter_property_is_descended_as_replaced() {
    let mut bin = fixture();
    let mut editor = Editor::new(|at| {
        if let At::EnterProperty(property) = at {
            match property.field().0 {
                // A node becomes a list of two nodes.
                F_EMBED if property.trail().is_empty() => {
                    *property.value_mut() =
                        values::Container::from(vec![node(C5, vec![]), node(C6, vec![])]).into();
                }
                // A node becomes a leaf.
                F_STRUCT if property.trail().is_empty() => {
                    *property.value_mut() = values::U8::new(1).into();
                }
                _ => {}
            }
        }
    });
    bin.walk_mut(&mut editor).unwrap();

    assert!(editor.nodes.contains(&("00000002[0]".to_owned(), C5)));
    assert!(editor.nodes.contains(&("00000002[1]".to_owned(), C6)));
    assert!(!editor.nodes.iter().any(|(_, class)| *class == C3));
    assert!(!editor
        .nodes
        .iter()
        .any(|(trail, _)| trail.starts_with("00000001")));
    assert!(!editor.exited.contains(&F_STRUCT), "a leaf has no exit");
    assert!(editor.exited.contains(&F_EMBED));
    assert_eq!(
        root(&bin).properties[&BinHash(F_STRUCT)],
        values::U8::new(1).into()
    );
}

#[test]
fn an_edit_at_a_node_inside_every_holder_writes_and_keeps_its_kind() {
    const F_MARK: u32 = 0x0E00;
    let mut bin = fixture();
    let mut editor = Editor::new(|at| {
        if let At::EnterNode(node) = at {
            if !node.is_root() {
                let mark = values::Hash::new(node.class_hash()).into();
                node.properties_mut().insert(BinHash(F_MARK), mark);
            }
        }
    });
    bin.walk_mut(&mut editor).unwrap();

    let object = root(&bin);
    let marked = |value: &PropertyValueEnum| match value {
        PropertyValueEnum::Struct(node) | PropertyValueEnum::Embedded(Embedded(node)) => {
            node.properties.get(&BinHash(F_MARK))
                == Some(&values::Hash::new(node.class_hash).into())
        }
        _ => false,
    };
    let PropertyValueEnum::Container(items) = &object.properties[&BinHash(F_CONT_STRUCT)] else {
        panic!("the container is a container");
    };
    assert_eq!(items.item_kind(), Kind::Struct);
    assert!(marked(&items.items()[0]) && !marked(&items.items()[1]));
    let PropertyValueEnum::Optional(optional) = &object.properties[&BinHash(F_OPT_STRUCT)] else {
        panic!("the optional is an optional");
    };
    assert!(marked(optional.value().unwrap()));
    let PropertyValueEnum::Map(map) = &object.properties[&BinHash(F_MAP_EMBED)] else {
        panic!("the map is a map");
    };
    assert_eq!(map.value_kind(), Kind::Embedded);
    assert!(marked(&map.entries()[0].1));

    let reread = reread(&bin);
    assert_eq!(reread, bin);
    let mut recorder = Recorder::new(always_continue);
    reread.walk(&mut recorder).unwrap();
    assert_eq!(recorder.nodes(), editor.nodes);
}

#[test]
fn an_edit_in_exit_property_replaces_the_value_after_its_nodes() {
    let mut bin = fixture();
    let mut editor = Editor::new(|at| {
        if let At::ExitProperty(property) = at {
            if property.field().0 == F_MAP_STRUCT {
                *property.value_mut() = values::Bool::new(false).into();
            }
        }
    });
    bin.walk_mut(&mut editor).unwrap();
    assert!(editor
        .nodes
        .contains(&("0000000a{000000aa}".to_owned(), C7)));
    assert_eq!(
        root(&bin).properties[&BinHash(F_MAP_STRUCT)],
        values::Bool::new(false).into()
    );
}

/// Records the trail's capacity at every node of a mutable walk.
#[derive(Default)]
struct CapacitiesMut(Vec<(usize, usize)>);

impl VisitorMut for CapacitiesMut {
    type Error = Error;

    fn enter_node(&mut self, node: &mut NodeMut<'_>) -> Result<Visit, Error> {
        let trail = node.trail();
        self.0
            .push((trail.steps.capacity(), trail.classes.capacity()));
        Ok(Visit::Continue)
    }
}

#[test]
fn a_mutable_walk_over_ten_thousand_entries_grows_the_trail_once() {
    let entries: Vec<_> = (0..10_000u32)
        .map(|i| {
            (
                values::String::from(format!("key {i}")).into(),
                PropertyValueEnum::Struct(node(C7, vec![])),
            )
        })
        .collect();
    let mut bin = Bin::builder()
        .object(
            BinObject::builder(OBJECT, C1)
                .property(
                    F_MAP_STRUCT,
                    values::Map::new(Kind::String, Kind::Struct, entries).unwrap(),
                )
                .build(),
        )
        .build();

    let mut visited = CapacitiesMut::default();
    bin.walk_mut(&mut visited).unwrap();
    assert_eq!(visited.0.len(), 10_001);
    let first = visited.0[1];
    assert!(first.0 <= 4 && first.1 <= 4, "{first:?}");
    assert!(visited.0[1..].iter().all(|c| *c == first));
}

#[test]
fn a_key_in_the_trail_reads_as_the_trees_own_key() {
    /// The decoded key of the last step at every node below a map.
    #[derive(Default)]
    struct Keys(Vec<String>);

    impl VisitorMut for Keys {
        type Error = Error;

        fn enter_node(&mut self, node: &mut NodeMut<'_>) -> Result<Visit, Error> {
            if let Some(TrailStep::Key(key)) = node.trail().steps().last() {
                // The key stays readable while the node's own properties are edited.
                node.properties_mut().clear();
                self.0.push(format!("{:?}", key.leaf()?));
            }
            Ok(Visit::Continue)
        }
    }

    let mut bin = fixture();
    let mut keys = Keys::default();
    bin.walk_mut(&mut keys).unwrap();
    assert_eq!(
        keys.0,
        [
            format!("{:?}", Some(Leaf::Hash(BinHash(KEY_A)))),
            format!("{:?}", Some(Leaf::String("k"))),
        ]
    );
}

#[test]
fn an_override_edits_its_embedded_objects_and_never_a_record() {
    const F_MARK: u32 = 0x0E00;
    let mut patch = BinOverride::builder()
        .objects(fixture().objects.into_values())
        .object(BinObject::builder(OBJECT + 1, C2).build())
        .set(
            OBJECT,
            crate::path::PropertyPath::new("mField").unwrap(),
            node(C3, vec![]),
        )
        .build();
    let records = patch.patches.clone();
    let mut editor = Editor::new(|at| {
        if let At::EnterNode(node) = at {
            if node.is_root() {
                node.properties_mut()
                    .insert(BinHash(F_MARK), values::Bool::new(true).into());
            }
        }
    });
    patch.walk_mut(&mut editor).unwrap();

    assert_eq!(patch.patches, records);
    assert_eq!(patch.objects.len(), 2);
    for object in patch.objects.values() {
        assert_eq!(
            object.properties.get(&BinHash(F_MARK)),
            Some(&values::Bool::new(true).into())
        );
    }
}

#[test]
fn a_mutable_reference_to_a_visitor_is_a_mutable_visitor() {
    let mut bin = fixture();
    let mut recorder = Recorder::new(always_continue);
    let by_ref: &mut dyn VisitorMut<Error = Error> = &mut recorder;
    let mut by_ref = by_ref;
    bin.walk_mut(&mut by_ref).unwrap();
    assert_eq!(recorder.nodes().len(), EXPECTED_NODES.len());
}

#[test]
fn a_shipped_bin_walks_the_same_mutably() {
    let mut bin = Bin::from_reader(&mut io::Cursor::new(UIBASE)).unwrap();
    let mut owned = Recorder::new(always_continue);
    bin.walk(&mut owned).unwrap();
    let mut mutable = Recorder::new(always_continue);
    bin.walk_mut(&mut mutable).unwrap();
    assert_eq!(owned.events, mutable.events);
}
