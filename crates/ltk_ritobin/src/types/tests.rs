use ltk_hash::{HashAlgorithm, HashWidth};
use ltk_meta::PropertyKind as K;

use super::*;
use crate::rito;

#[test]
fn a_type_prints_its_ritobin_spelling() {
    let xxh3 = ScalarType::Hash(HashType::new(HashWidth::W8, HashAlgorithm::Xxh3));
    let cases = [
        (rito!(U32), "u32"),
        (rito!(Hash), "hash"),
        (rito!(Container[String]), "list[string]"),
        (rito!(UnorderedContainer[Embedded]), "list2[embed]"),
        (rito!(Optional[Vector3]), "option[vec3]"),
        (rito!(Map[Hash, Struct]), "map[hash,pointer]"),
        (RitoType::list(xxh3), "list[hash[u64,xxh3]]"),
        (RitoType::from_property_kind(K::Container), "list"),
    ];
    for (rito, text) in cases {
        assert_eq!(rito.to_string(), text);
    }
}

#[test]
fn a_map_with_one_usable_subtype_has_neither() {
    let bare = RitoType::from_property_kind(K::Map);
    assert_eq!(
        RitoType::from_parts(K::Map, [Some(K::Container), Some(K::U32)]),
        bare
    );
    assert_eq!(RitoType::from_parts(K::Map, [Some(K::U32), None]), bare);
    assert_eq!(bare.to_string(), "map");
}

#[test]
fn a_missing_subtype_matches_any_subtype() {
    let bare = RitoType::from_property_kind(K::Container);
    assert!(bare.equivalent_to(&rito!(Container[U32])));
    assert!(rito!(Container[U32]).equivalent_to(&bare));
    assert!(!rito!(Container[U32]).equivalent_to(&rito!(Container[U8])));
    assert!(!bare.equivalent_to(&RitoType::from_property_kind(K::UnorderedContainer)));
}

#[test]
fn hash_types_compare_in_full() {
    let xxh3 = ScalarType::Hash(HashType::new(HashWidth::W8, HashAlgorithm::Xxh3));
    assert!(!RitoType::from(xxh3).equivalent_to(&rito!(Hash)));
    assert!(RitoType::from(xxh3)
        .with_default_hash_types()
        .equivalent_to(&rito!(Hash)));
}

#[test]
fn a_type_from_parts_rejects_a_shape_ritobin_has_no_spelling_for() {
    use BadContainerType as C;
    use BadRitoType as R;
    let cases = [
        (
            (K::Container, [None, None]),
            R::BadScalarType(BadScalarType(K::Container)),
        ),
        (
            (K::U32, [Some(K::U8), None]),
            R::BadContainerType(C::NonContainerBase { got: K::U32 }),
        ),
        (
            (K::Container, [Some(K::Map), None]),
            R::BadContainerType(C::NonScalarSubtype {
                base: K::Container,
                got: K::Map,
            }),
        ),
        (
            (K::Optional, [Some(K::U8), Some(K::U8)]),
            R::BadContainerType(C::BadSubtypeCount {
                base: K::Optional,
                got: 2,
                expected: 1,
            }),
        ),
        (
            (K::Map, [Some(K::U32), None]),
            R::BadContainerType(C::BadSubtypeCount {
                base: K::Map,
                got: 1,
                expected: 2,
            }),
        ),
        (
            (K::Map, [Some(K::Embedded), Some(K::U32)]),
            R::BadContainerType(C::BadMapKey { got: K::Embedded }),
        ),
    ];
    for ((base, subtypes), error) in cases {
        assert_eq!(RitoType::try_from_parts(base, subtypes), Err(error));
    }
}

#[test]
fn a_type_from_parts_leaves_out_a_subtype_that_does_not_fit() {
    assert_eq!(
        RitoType::from_parts(K::Container, [Some(K::Map), None]),
        RitoType::from_property_kind(K::Container)
    );
    assert_eq!(
        RitoType::from_parts(K::U32, [Some(K::U8), None]),
        rito!(U32)
    );
}
