//! Text writer for ritobin format.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hashes::HashMapProvider;

    #[test]
    fn test_write_simple() {
        let tree = Bin::new([], std::iter::empty::<&str>());
        let text = print(&tree).unwrap();
        assert!(text.contains("#PROP_text"));
        assert!(text.contains("type: string = \"PROP\""));
        assert!(text.contains("version: u32 = 3"));
    }

    #[test]
    fn test_write_with_dependencies() {
        let tree = Bin::new(
            std::iter::empty(),
            vec![
                "path/to/dep1.bin".to_string(),
                "path/to/dep2.bin".to_string(),
            ],
        );
        let text = print(&tree).unwrap();
        assert!(text.contains("linked: list[string] = {"));
        assert!(text.contains("\"path/to/dep1.bin\""));
        assert!(text.contains("\"path/to/dep2.bin\""));
    }

    #[test]
    fn test_write_with_hash_lookup() {
        use indexmap::IndexMap;
        use ltk_meta::property::values::String;

        // Create a simple tree with a hash value
        let mut properties = IndexMap::new();
        let name_hash = ltk_hash::fnv1a::hash_lower("testField");
        properties.insert(name_hash, PropertyValueEnum::String(String::from("hello")));

        let path_hash = ltk_hash::fnv1a::hash_lower("Test/Path");
        let class_hash = ltk_hash::fnv1a::hash_lower("TestClass");

        let obj = BinObject {
            path_hash,
            class_hash,
            properties,
        };

        let tree = Bin::new(std::iter::once(obj), std::iter::empty::<&str>());

        // Without hash lookup - should have hex values
        let text_hex = print(&tree).unwrap();
        assert!(text_hex.contains(&format!("{:#x}", path_hash)));

        // With hash lookup - should have named values
        let mut hashes = HashMapProvider::new();
        hashes.insert_entry(path_hash, "Test/Path");
        hashes.insert_field(name_hash, "testField");
        hashes.insert_type(class_hash, "TestClass");

        let text_named = print_with_hashes(&tree, &hashes).unwrap();
        assert!(text_named.contains("\"Test/Path\""));
        assert!(text_named.contains("testField:"));
        assert!(text_named.contains("TestClass {"));
    }
}
