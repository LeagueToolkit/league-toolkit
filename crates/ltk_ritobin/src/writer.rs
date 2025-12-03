//! Text writer for ritobin format.

use std::fmt::Write;

use ltk_meta::{
    value::{
        ContainerValue, EmbeddedValue, MapValue, OptionalValue, PropertyValueEnum, StructValue,
        UnorderedContainerValue,
    },
    BinProperty, BinTree, BinTreeObject,
};

use crate::{error::WriteError, types::kind_to_type_name};

/// Configuration for the text writer.
#[derive(Debug, Clone)]
pub struct WriterConfig {
    /// Number of spaces per indent level.
    pub indent_size: usize,
    /// Whether to use named hashes when available (requires hash lookup).
    pub use_named_hashes: bool,
}

impl Default for WriterConfig {
    fn default() -> Self {
        Self {
            indent_size: 4,
            use_named_hashes: false,
        }
    }
}

/// Text writer for ritobin format.
pub struct TextWriter {
    buffer: String,
    indent_level: usize,
    config: WriterConfig,
}

impl TextWriter {
    pub fn new() -> Self {
        Self::with_config(WriterConfig::default())
    }

    pub fn with_config(config: WriterConfig) -> Self {
        Self {
            buffer: String::new(),
            indent_level: 0,
            config,
        }
    }

    pub fn into_string(self) -> String {
        self.buffer
    }

    fn indent(&mut self) {
        self.indent_level += self.config.indent_size;
    }

    fn dedent(&mut self) {
        self.indent_level = self.indent_level.saturating_sub(self.config.indent_size);
    }

    fn pad(&mut self) {
        for _ in 0..self.indent_level {
            self.buffer.push(' ');
        }
    }

    fn write_raw(&mut self, s: &str) {
        self.buffer.push_str(s);
    }

    fn write_type(&mut self, value: &PropertyValueEnum) {
        let type_name = kind_to_type_name(value.kind());
        self.write_raw(type_name);

        match value {
            PropertyValueEnum::Container(ContainerValue { item_kind, .. })
            | PropertyValueEnum::UnorderedContainer(UnorderedContainerValue(ContainerValue {
                item_kind,
                ..
            })) => {
                self.write_raw("[");
                self.write_raw(kind_to_type_name(*item_kind));
                self.write_raw("]");
            }
            PropertyValueEnum::Optional(OptionalValue { kind, .. }) => {
                self.write_raw("[");
                self.write_raw(kind_to_type_name(*kind));
                self.write_raw("]");
            }
            PropertyValueEnum::Map(MapValue {
                key_kind,
                value_kind,
                ..
            }) => {
                self.write_raw("[");
                self.write_raw(kind_to_type_name(*key_kind));
                self.write_raw(",");
                self.write_raw(kind_to_type_name(*value_kind));
                self.write_raw("]");
            }
            _ => {}
        }
    }

    fn write_value(&mut self, value: &PropertyValueEnum) -> Result<(), WriteError> {
        match value {
            PropertyValueEnum::None(_) => self.write_raw("null"),
            PropertyValueEnum::Bool(v) => self.write_raw(if v.0 { "true" } else { "false" }),
            PropertyValueEnum::I8(v) => write!(self.buffer, "{}", v.0)?,
            PropertyValueEnum::U8(v) => write!(self.buffer, "{}", v.0)?,
            PropertyValueEnum::I16(v) => write!(self.buffer, "{}", v.0)?,
            PropertyValueEnum::U16(v) => write!(self.buffer, "{}", v.0)?,
            PropertyValueEnum::I32(v) => write!(self.buffer, "{}", v.0)?,
            PropertyValueEnum::U32(v) => write!(self.buffer, "{}", v.0)?,
            PropertyValueEnum::I64(v) => write!(self.buffer, "{}", v.0)?,
            PropertyValueEnum::U64(v) => write!(self.buffer, "{}", v.0)?,
            PropertyValueEnum::F32(v) => write!(self.buffer, "{}", v.0)?,
            PropertyValueEnum::Vector2(v) => {
                write!(self.buffer, "{{ {}, {} }}", v.0.x, v.0.y)?;
            }
            PropertyValueEnum::Vector3(v) => {
                write!(self.buffer, "{{ {}, {}, {} }}", v.0.x, v.0.y, v.0.z)?;
            }
            PropertyValueEnum::Vector4(v) => {
                write!(
                    self.buffer,
                    "{{ {}, {}, {}, {} }}",
                    v.0.x, v.0.y, v.0.z, v.0.w
                )?;
            }
            PropertyValueEnum::Matrix44(v) => {
                self.write_raw("{\n");
                self.indent();
                let arr = v.0.to_cols_array();
                for (i, val) in arr.iter().enumerate() {
                    if i % 4 == 0 {
                        self.pad();
                    }
                    write!(self.buffer, "{}", val)?;
                    if i % 4 == 3 {
                        self.write_raw("\n");
                        if i == 15 {
                            self.dedent();
                        }
                    } else {
                        self.write_raw(", ");
                    }
                }
                self.pad();
                self.write_raw("}");
            }
            PropertyValueEnum::Color(v) => {
                write!(
                    self.buffer,
                    "{{ {}, {}, {}, {} }}",
                    v.0.r, v.0.g, v.0.b, v.0.a
                )?;
            }
            PropertyValueEnum::String(v) => {
                write!(self.buffer, "{:?}", v.0)?;
            }
            PropertyValueEnum::Hash(v) => {
                write!(self.buffer, "{:#x}", v.0)?;
            }
            PropertyValueEnum::WadChunkLink(v) => {
                write!(self.buffer, "{:#x}", v.0)?;
            }
            PropertyValueEnum::ObjectLink(v) => {
                // For links, we try to output as quoted string if we have the name
                // For now, output as hex
                write!(self.buffer, "{:#x}", v.0)?;
            }
            PropertyValueEnum::BitBool(v) => self.write_raw(if v.0 { "true" } else { "false" }),

            PropertyValueEnum::Container(ContainerValue { items, .. })
            | PropertyValueEnum::UnorderedContainer(UnorderedContainerValue(ContainerValue {
                items,
                ..
            })) => {
                if items.is_empty() {
                    self.write_raw("{}");
                } else {
                    self.write_raw("{\n");
                    self.indent();
                    for item in items {
                        self.pad();
                        self.write_value(item)?;
                        self.write_raw("\n");
                    }
                    self.dedent();
                    self.pad();
                    self.write_raw("}");
                }
            }
            PropertyValueEnum::Optional(OptionalValue { value, .. }) => {
                if let Some(inner) = value {
                    self.write_raw("{\n");
                    self.indent();
                    self.pad();
                    self.write_value(inner)?;
                    self.write_raw("\n");
                    self.dedent();
                    self.pad();
                    self.write_raw("}");
                } else {
                    self.write_raw("{}");
                }
            }
            PropertyValueEnum::Map(MapValue { entries, .. }) => {
                if entries.is_empty() {
                    self.write_raw("{}");
                } else {
                    self.write_raw("{\n");
                    self.indent();
                    for (key, value) in entries {
                        self.pad();
                        self.write_value(&key.0)?;
                        self.write_raw(" = ");
                        self.write_value(value)?;
                        self.write_raw("\n");
                    }
                    self.dedent();
                    self.pad();
                    self.write_raw("}");
                }
            }
            PropertyValueEnum::Struct(v) => {
                self.write_struct_value(v)?;
            }
            PropertyValueEnum::Embedded(EmbeddedValue(v)) => {
                self.write_struct_value(v)?;
            }
        }
        Ok(())
    }

    fn write_struct_value(&mut self, v: &StructValue) -> Result<(), WriteError> {
        if v.class_hash == 0 && v.properties.is_empty() {
            self.write_raw("null");
        } else {
            write!(self.buffer, "{:#x} ", v.class_hash)?;
            if v.properties.is_empty() {
                self.write_raw("{}");
            } else {
                self.write_raw("{\n");
                self.indent();
                for prop in v.properties.values() {
                    self.pad();
                    write!(self.buffer, "{:#x}: ", prop.name_hash)?;
                    self.write_type(&prop.value);
                    self.write_raw(" = ");
                    self.write_value(&prop.value)?;
                    self.write_raw("\n");
                }
                self.dedent();
                self.pad();
                self.write_raw("}");
            }
        }
        Ok(())
    }

    #[allow(dead_code)]
    fn write_property(&mut self, name: &str, prop: &BinProperty) -> Result<(), WriteError> {
        self.pad();
        self.write_raw(name);
        self.write_raw(": ");
        self.write_type(&prop.value);
        self.write_raw(" = ");
        self.write_value(&prop.value)?;
        self.write_raw("\n");
        Ok(())
    }

    fn write_property_with_hash(&mut self, prop: &BinProperty) -> Result<(), WriteError> {
        self.pad();
        write!(self.buffer, "{:#x}: ", prop.name_hash)?;
        self.write_type(&prop.value);
        self.write_raw(" = ");
        self.write_value(&prop.value)?;
        self.write_raw("\n");
        Ok(())
    }
}

impl Default for TextWriter {
    fn default() -> Self {
        Self::new()
    }
}

/// Write a BinTree to ritobin text format.
pub fn write(tree: &BinTree) -> Result<String, WriteError> {
    write_with_config(tree, WriterConfig::default())
}

/// Write a BinTree to ritobin text format with custom configuration.
pub fn write_with_config(tree: &BinTree, config: WriterConfig) -> Result<String, WriteError> {
    let mut writer = TextWriter::with_config(config);

    // Header
    writer.write_raw("#PROP_text\n");

    // Type
    writer.write_raw("type: string = \"PROP\"\n");

    // Version
    writeln!(writer.buffer, "version: u32 = {}", tree.version)?;

    // Dependencies (linked)
    if !tree.dependencies.is_empty() {
        writer.write_raw("linked: list[string] = {\n");
        writer.indent();
        for dep in &tree.dependencies {
            writer.pad();
            writeln!(writer.buffer, "{:?}", dep)?;
        }
        writer.dedent();
        writer.write_raw("}\n");
    }

    // Entries (objects)
    if !tree.objects.is_empty() {
        writer.write_raw("entries: map[hash,embed] = {\n");
        writer.indent();
        for obj in tree.objects.values() {
            writer.pad();
            write!(writer.buffer, "{:#x} = ", obj.path_hash)?;

            // Write the object as an embed
            write!(writer.buffer, "{:#x} ", obj.class_hash)?;
            if obj.properties.is_empty() {
                writer.write_raw("{}\n");
            } else {
                writer.write_raw("{\n");
                writer.indent();
                for prop in obj.properties.values() {
                    writer.write_property_with_hash(prop)?;
                }
                writer.dedent();
                writer.pad();
                writer.write_raw("}\n");
            }
        }
        writer.dedent();
        writer.write_raw("}\n");
    }

    Ok(writer.into_string())
}

/// A builder for creating ritobin files programmatically.
#[derive(Debug, Default)]
pub struct RitobinBuilder {
    version: u32,
    dependencies: Vec<String>,
    objects: Vec<BinTreeObject>,
}

impl RitobinBuilder {
    pub fn new() -> Self {
        Self {
            version: 3,
            dependencies: Vec::new(),
            objects: Vec::new(),
        }
    }

    pub fn version(mut self, version: u32) -> Self {
        self.version = version;
        self
    }

    pub fn dependency(mut self, dep: impl Into<String>) -> Self {
        self.dependencies.push(dep.into());
        self
    }

    pub fn dependencies(mut self, deps: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.dependencies.extend(deps.into_iter().map(Into::into));
        self
    }

    pub fn object(mut self, obj: BinTreeObject) -> Self {
        self.objects.push(obj);
        self
    }

    pub fn objects(mut self, objs: impl IntoIterator<Item = BinTreeObject>) -> Self {
        self.objects.extend(objs);
        self
    }

    pub fn build(self) -> BinTree {
        // Note: BinTree::new sets version to 3, but we want to allow overriding
        // For now this is a limitation
        BinTree::new(self.objects, self.dependencies)
    }

    pub fn to_text(self) -> Result<String, WriteError> {
        write(&self.build())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_simple() {
        let tree = BinTree::new(std::iter::empty(), std::iter::empty());
        let text = write(&tree).unwrap();
        assert!(text.contains("#PROP_text"));
        assert!(text.contains("type: string = \"PROP\""));
        assert!(text.contains("version: u32 = 3"));
    }

    #[test]
    fn test_write_with_dependencies() {
        let tree = BinTree::new(
            std::iter::empty(),
            vec![
                "path/to/dep1.bin".to_string(),
                "path/to/dep2.bin".to_string(),
            ],
        );
        let text = write(&tree).unwrap();
        assert!(text.contains("linked: list[string] = {"));
        assert!(text.contains("\"path/to/dep1.bin\""));
        assert!(text.contains("\"path/to/dep2.bin\""));
    }

    #[test]
    fn test_builder() {
        let tree = RitobinBuilder::new()
            .version(3)
            .dependency("path/to/dep.bin")
            .build();

        assert_eq!(tree.dependencies.len(), 1);
    }
}
