use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq, PartialOrd)]
pub struct ModProject {
    pub name: String,
    pub display_name: String,
    pub version: String,
    pub description: String,
    pub authors: Vec<ModProjectAuthor>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, PartialOrd)]
#[serde(untagged)]
pub enum ModProjectAuthor {
    Name(String),
    Role { name: String, role: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_project() {
        let project: ModProject =
            toml::from_str(include_str!("../test-data/modproject.toml")).unwrap();

        assert_eq!(
            project,
            ModProject {
                name: "test".to_string(),
                display_name: "Test 123".to_string(),
                version: "0.1.0".to_string(),
                description: "test".to_string(),
                authors: vec![
                    ModProjectAuthor::Name("test".to_string()),
                    ModProjectAuthor::Role {
                        name: "test 2".to_string(),
                        role: "developer".to_string(),
                    },
                ],
            }
        );
    }
}
