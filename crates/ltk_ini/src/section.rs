use glam::{Vec2, Vec3, Vec4};
use ltk_hash::{sdbm, IniHash};

use crate::{Inibin, Value};

/// Returns the hash state after the section name `section` and the `*` separator.
fn section_hash(section: &str) -> u32 {
    sdbm::hash_lower_from(sdbm::hash_lower(section), "*")
}

/// A view of the values of one section of an [`Inibin`]. The view accesses a value by property
/// name.
///
/// [`Inibin::section`] returns a `Section`. An inibin file does not store section names or
/// property names. A `Section` hashes the property name for each lookup and cannot list the
/// properties of the section.
///
/// ```
/// use ltk_ini::Inibin;
///
/// let mut inibin = Inibin::new();
/// inibin.section_mut("Flash").insert("p-life", 1.5);
///
/// let flash = inibin.section("Flash");
/// assert_eq!(flash.get_f32("p-life"), Some(1.5));
/// assert_eq!(flash.get_f32("p-linger"), None);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Section<'a> {
    inibin: &'a Inibin,
    hash: u32,
}

impl<'a> Section<'a> {
    pub(crate) fn new(inibin: &'a Inibin, name: &str) -> Self {
        Self {
            inibin,
            hash: section_hash(name),
        }
    }

    /// Returns the key of the property `name` in the section.
    #[must_use]
    pub fn key(&self, name: &str) -> IniHash {
        IniHash(sdbm::hash_lower_from(self.hash, name))
    }

    /// Returns the value of the property `name`. Returns `None` if the section has no property
    /// `name`.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&'a Value> {
        self.inibin.get(self.key(name))
    }

    /// Returns `true` if the section has the property `name`.
    #[must_use]
    pub fn contains_key(&self, name: &str) -> bool {
        self.inibin.contains_key(self.key(name))
    }

    /// Returns the string of the property `name`. Returns `None` if the value is not a
    /// [`Value::String`].
    #[must_use]
    pub fn get_str(&self, name: &str) -> Option<&'a str> {
        self.get(name).and_then(Value::as_str)
    }

    /// Returns the property `name` as the 32-bit integer that the client reads. Converts the
    /// value with [`Value::to_i32`].
    #[must_use]
    pub fn get_i32(&self, name: &str) -> Option<i32> {
        self.get(name).and_then(Value::to_i32)
    }

    /// Returns the property `name` as the 64-bit integer that the client reads. Converts the
    /// value with [`Value::to_i64`].
    #[must_use]
    pub fn get_i64(&self, name: &str) -> Option<i64> {
        self.get(name).and_then(Value::to_i64)
    }

    /// Returns the property `name` as the float that the client reads. Converts the value with
    /// [`Value::to_f32`].
    #[must_use]
    pub fn get_f32(&self, name: &str) -> Option<f32> {
        self.get(name).and_then(Value::to_f32)
    }

    /// Returns the property `name` as the boolean that the client reads. Converts the value with
    /// [`Value::to_bool`].
    #[must_use]
    pub fn get_bool(&self, name: &str) -> Option<bool> {
        self.get(name).and_then(Value::to_bool)
    }

    /// Returns the property `name` as the 2 floats that the client reads. Converts the value with
    /// [`Value::to_vec2`].
    #[must_use]
    pub fn get_vec2(&self, name: &str) -> Option<Vec2> {
        self.get(name).and_then(Value::to_vec2)
    }

    /// Returns the property `name` as the 3 floats that the client reads. Converts the value with
    /// [`Value::to_vec3`].
    #[must_use]
    pub fn get_vec3(&self, name: &str) -> Option<Vec3> {
        self.get(name).and_then(Value::to_vec3)
    }

    /// Returns the property `name` as the 4 floats that the client reads. Converts the value with
    /// [`Value::to_vec4`].
    #[must_use]
    pub fn get_vec4(&self, name: &str) -> Option<Vec4> {
        self.get(name).and_then(Value::to_vec4)
    }
}

/// A mutable view of the values of one section of an [`Inibin`]. The view accesses a value by
/// property name.
///
/// [`Inibin::section_mut`] returns a `SectionMut`.
///
/// ```
/// use ltk_ini::{Inibin, Value};
///
/// let mut inibin = Inibin::new();
/// let mut system = inibin.section_mut("System");
/// system.insert("GroupPart0", "Flash");
/// system.insert("build-up-time", Value::compact_float(0.5));
/// assert_eq!(system.remove("GroupPart0"), Some(Value::from("Flash")));
///
/// assert_eq!(inibin.len(), 1);
/// ```
#[derive(Debug)]
pub struct SectionMut<'a> {
    inibin: &'a mut Inibin,
    hash: u32,
}

impl<'a> SectionMut<'a> {
    pub(crate) fn new(inibin: &'a mut Inibin, name: &str) -> Self {
        Self {
            inibin,
            hash: section_hash(name),
        }
    }

    /// Returns the key of the property `name` in the section.
    #[must_use]
    pub fn key(&self, name: &str) -> IniHash {
        IniHash(sdbm::hash_lower_from(self.hash, name))
    }

    /// Returns the value of the property `name`. Returns `None` if the section has no property
    /// `name`.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.inibin.get(self.key(name))
    }

    /// Returns a mutable reference to the value of the property `name`. Returns `None` if the
    /// section has no property `name`.
    #[must_use]
    pub fn get_mut(&mut self, name: &str) -> Option<&mut Value> {
        self.inibin.get_mut(self.key(name))
    }

    /// Sets the property `name` to `value`. Returns the previous value of the property.
    pub fn insert(&mut self, name: &str, value: impl Into<Value>) -> Option<Value> {
        self.inibin.insert(self.key(name), value)
    }

    /// Removes the property `name`. Returns the removed value. Returns `None` if the section has
    /// no property `name`.
    pub fn remove(&mut self, name: &str) -> Option<Value> {
        self.inibin.remove(self.key(name))
    }
}

#[cfg(test)]
mod tests {
    use glam::{vec2, vec3, vec4};

    use super::*;

    #[test]
    fn key_returns_ini_hash_of_section_and_name() {
        let inibin = Inibin::new();
        let key = inibin.section("System").key("GroupPart1");
        assert_eq!(key, IniHash::new("System", "GroupPart1"));
        assert_eq!(key, IniHash(0x0616933a));
        assert_eq!(inibin.section("").key("x"), IniHash::new("", "x"));
    }

    #[test]
    fn get_str_ignores_ascii_case_of_names() {
        let mut inibin = Inibin::new();
        inibin.section_mut("System").insert("GroupPart1", "glow");
        assert_eq!(inibin.section("SYSTEM").get_str("grouppart1"), Some("glow"));
    }

    #[test]
    fn typed_getters_convert_like_value_to_methods() {
        let mut inibin = Inibin::new();
        let mut glow = inibin.section_mut("glow");
        glow.insert("p-life", Value::Fixed(15));
        glow.insert("pass", Value::I16(-50));
        glow.insert("flag", Value::U1(true));
        glow.insert("p-texture", "glow.dds");
        glow.insert("uv", Value::FixedVec2([5, 10]));
        glow.insert("p-vel", "0 2 0");
        glow.insert("p-rgba", Value::FixedVec4([10, 10, 10, 5]));

        let glow = inibin.section("glow");
        assert_eq!(glow.get_f32("p-life"), Some(1.5));
        assert_eq!(glow.get_i32("pass"), Some(-50));
        assert_eq!(glow.get_i64("pass"), Some(-50));
        assert_eq!(glow.get_bool("flag"), Some(true));
        assert_eq!(glow.get_str("p-texture"), Some("glow.dds"));
        assert_eq!(glow.get_str("p-life"), None);
        assert_eq!(glow.get_vec2("uv"), Some(vec2(0.5, 1.0)));
        assert_eq!(glow.get_vec3("p-vel"), Some(vec3(0.0, 2.0, 0.0)));
        assert_eq!(glow.get_vec4("p-rgba"), Some(vec4(1.0, 1.0, 1.0, 0.5)));
        assert_eq!(glow.get_f32("missing"), None);
        assert!(glow.contains_key("pass"));
        assert!(!inibin.section("core").contains_key("pass"));
    }

    #[test]
    fn section_mut_edits_only_its_section() {
        let mut inibin = Inibin::new();
        inibin.section_mut("core").insert("pass", 1);

        let mut glow = inibin.section_mut("glow");
        assert_eq!(glow.insert("pass", 2), None);
        assert_eq!(glow.insert("pass", 3), Some(Value::I32(2)));
        if let Some(pass) = glow.get_mut("pass") {
            *pass = Value::U8(4);
        }
        assert_eq!(glow.get("pass"), Some(&Value::U8(4)));
        assert_eq!(glow.remove("pass"), Some(Value::U8(4)));
        assert_eq!(glow.remove("pass"), None);

        assert_eq!(inibin.section("core").get_i32("pass"), Some(1));
        assert_eq!(inibin.len(), 1);
    }
}
