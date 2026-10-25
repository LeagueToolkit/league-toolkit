//! The ini text format.

use crate::{IniHash, Inibin, Value};

/// The prefixes of a line that the ini compiler of old builds stores as part of a property name.
/// [`IniTextOptions::additional_comment_tokens`] reads a line with such a prefix as a comment.
pub const ADDITIONAL_COMMENT_TOKENS: [&str; 5] = ["'", "*", "#", "//", "`"];

/// How [`Inibin::from_ini`] reads ini text.
///
/// The default reads a line that starts with one of [`ADDITIONAL_COMMENT_TOKENS`] as a property,
/// as the ini compiler of old builds does.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IniTextOptions {
    /// Reads a line that starts with one of [`ADDITIONAL_COMMENT_TOKENS`] as a comment.
    pub additional_comment_tokens: bool,
}

impl Inibin {
    /// Reads ini text. Every value is a [`Value::String`].
    ///
    /// The text from the first `;` of a line to its end is a comment. A line `[name]` starts the
    /// section `name`. A line with `=` is a property: the text before the first `=` is the name,
    /// the text after it is the value, and both lose the whitespace around them. A property before
    /// the first section is in the section with the empty name. A repeated property takes the
    /// value of its last line. Reading skips every other line.
    #[must_use]
    pub fn from_ini(text: &str, options: IniTextOptions) -> Inibin {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let mut inibin = Inibin::new();
        let mut section = "";
        for line in text.lines() {
            let line = line.split_once(';').map_or(line, |(line, _)| line).trim();
            if options.additional_comment_tokens
                && ADDITIONAL_COMMENT_TOKENS
                    .iter()
                    .any(|token| line.starts_with(token))
            {
                continue;
            }
            if let Some(name) = line
                .strip_prefix('[')
                .and_then(|line| line.strip_suffix(']'))
            {
                section = name.trim();
            } else if let Some((name, value)) = line.split_once('=') {
                inibin.insert(
                    IniHash::new(section, name.trim()),
                    Value::String(value.trim().to_owned()),
                );
            }
        }
        inibin
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn string(inibin: &Inibin, section: &str, name: &str) -> Option<String> {
        inibin
            .get(IniHash::new(section, name))
            .and_then(Value::as_str)
            .map(str::to_owned)
    }

    #[test]
    fn from_ini_reads_sections_and_trimmed_properties() {
        let inibin = Inibin::from_ini(
            "\u{feff}[System]\r\n  GroupPart1 =  glow \r\n[glow]\np-life=1.5\n",
            IniTextOptions::default(),
        );

        assert_eq!(inibin.len(), 2);
        assert_eq!(
            string(&inibin, "System", "GroupPart1").as_deref(),
            Some("glow")
        );
        assert_eq!(string(&inibin, "glow", "p-life").as_deref(), Some("1.5"));
    }

    #[test]
    fn from_ini_cuts_semicolon_comments() {
        let inibin = Inibin::from_ini(
            "; header\n[Data] ; stats\nArmor=30 ; Burn total damage\n;Hidden=1\n",
            IniTextOptions::default(),
        );

        assert_eq!(inibin.len(), 1);
        assert_eq!(string(&inibin, "Data", "Armor").as_deref(), Some("30"));
    }

    #[test]
    fn from_ini_reads_prefixed_lines_as_properties_by_default() {
        let text = "[System]\n'GroupPart3=off\n//Note=1\n#Tint=2\n";

        let inibin = Inibin::from_ini(text, IniTextOptions::default());

        assert_eq!(
            string(&inibin, "System", "'GroupPart3").as_deref(),
            Some("off")
        );
        assert_eq!(string(&inibin, "System", "//Note").as_deref(), Some("1"));
        assert_eq!(string(&inibin, "System", "#Tint").as_deref(), Some("2"));
    }

    #[test]
    fn from_ini_with_additional_comment_tokens_skips_prefixed_lines() {
        let options = IniTextOptions {
            additional_comment_tokens: true,
        };

        let inibin = Inibin::from_ini("[System]\n'GroupPart3=off\n*A=1\n`B=2\nC=3\n", options);

        assert_eq!(inibin.len(), 1);
        assert_eq!(string(&inibin, "System", "C").as_deref(), Some("3"));
    }

    #[test]
    fn from_ini_skips_lines_without_equals_and_keeps_last_repeat() {
        let inibin = Inibin::from_ini(
            "Top=1\n[A]\njunk line\nX=1\nX=2\n[B\n",
            IniTextOptions::default(),
        );

        assert_eq!(inibin.len(), 2);
        assert_eq!(string(&inibin, "", "Top").as_deref(), Some("1"));
        assert_eq!(string(&inibin, "A", "X").as_deref(), Some("2"));
    }
}
