use crate::HashProvider;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Wrap {
    AllowInline,
    Always,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum VectorWrap {
    /// Never wrap, always one line
    #[default]
    Never,
    /// Match wrap behaviour for lists
    List,
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// How/when to wrap, see [`PrintConfig`]
pub struct WrapConfig {
    /// The (soft) maximum line width used for wrapping.
    /// See [`Self::structs`], [`Self::lists`] & [`Self::vectors`] for specific wrap behaviour
    pub wrap_width: Option<usize>,

    /// Whether to allow the printing of structs/classes in one line
    /// (subject to [`Self::line_width`] wrapping)
    pub structs: Wrap,

    /// Whether to allow the printing of lists in one line
    /// (subject to [`Self::line_width`] wrapping)
    pub lists: Wrap,

    /// How `vec2`/`vec3`/`vec4`/`rgba` types get wrapped
    ///
    /// **NOTE:** this *only* affects [`crate::Cst`]s built from [`crate::Ast`]s,
    /// as it relies on type information.
    pub vectors: VectorWrap,
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Where to put spaces, see [`PrintConfig`]
pub struct Spaces {
    /// Whether to put spaces between type arguments in type expressions.
    /// `map[string,u32]` vs `map[string, u32]`
    pub between_type_arguments: bool,
    /// Whether to put a space inside empty blocks.
    /// `Class {}` vs `Class { }`
    pub inside_empty_blocks: bool,
}

/// Configuration for the ritobin printer.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PrintConfig<Hashes: HashProvider> {
    /// Number of spaces per indent level.
    pub indent_size: usize,

    /// Where to put spaces
    pub spaces: Spaces,
    /// How/when to wrap
    pub wrap: WrapConfig,

    #[cfg_attr(feature = "serde", serde(skip, default))]
    pub hashes: Hashes,
}

impl Default for WrapConfig {
    fn default() -> Self {
        Self {
            wrap_width: Some(120),
            structs: Wrap::Always,
            lists: Wrap::AllowInline,
            vectors: VectorWrap::Never,
        }
    }
}
impl Default for Spaces {
    fn default() -> Self {
        Self {
            between_type_arguments: true,
            inside_empty_blocks: true,
        }
    }
}
impl Default for PrintConfig<()> {
    fn default() -> Self {
        Self {
            indent_size: 4,
            spaces: Spaces::default(),
            wrap: WrapConfig::default(),
            hashes: (),
        }
    }
}

impl WrapConfig {
    pub fn canonical() -> Self {
        Self {
            wrap_width: None,
            structs: Wrap::Always,
            lists: Wrap::Always,
            vectors: VectorWrap::Never,
        }
    }
}
impl Spaces {
    fn canonical() -> Self {
        Self {
            between_type_arguments: false,
            inside_empty_blocks: false,
        }
    }
}
impl PrintConfig<()> {
    pub fn canonical() -> Self {
        Self {
            indent_size: 4,
            spaces: Spaces::canonical(),
            wrap: WrapConfig::canonical(),
            hashes: (),
        }
    }
}

impl WrapConfig {
    pub fn line_width(mut self, line_width: usize) -> Self {
        self.wrap_width.replace(line_width);
        self
    }

    pub fn structs(mut self, wrapping: Wrap) -> Self {
        self.structs = wrapping;
        self
    }

    pub fn lists(mut self, wrapping: Wrap) -> Self {
        self.lists = wrapping;
        self
    }

    pub fn vectors(mut self, wrapping: VectorWrap) -> Self {
        self.vectors = wrapping;
        self
    }
}

impl<H: HashProvider> PrintConfig<H> {
    pub fn wrap(mut self, wrap: WrapConfig) -> Self {
        self.wrap = wrap;
        self
    }
    pub fn spaces(mut self, spaces: Spaces) -> Self {
        self.spaces = spaces;
        self
    }

    pub fn indent_size(mut self, indent_size: usize) -> Self {
        self.indent_size = indent_size;
        self
    }

    pub fn with_hashes<H2: HashProvider>(self, hashes: H2) -> PrintConfig<H2> {
        PrintConfig {
            indent_size: self.indent_size,
            spaces: self.spaces,
            wrap: self.wrap,
            hashes,
        }
    }

    pub fn take_hashes(self) -> (PrintConfig<()>, H) {
        (
            PrintConfig {
                indent_size: self.indent_size,
                spaces: self.spaces,
                wrap: self.wrap,
                hashes: (),
            },
            self.hashes,
        )
    }
}
