//! The recipes of the classes, and the `recipe!` and `name!` macros that they are written in.
//!
//! A recipe lists the sections of a file and the typed properties of each section:
//!
//! ```text
//! recipe!(r =>
//!     for d in ["eINTERMEDIATE", "eADVANCED", "eUBER"] {
//!         ["BotMasteryData" d] {
//!             for i in 1.. while int ("Mastery" i) {
//!                 int ("Mastery" i "Level");
//!             }
//!         }
//!     }
//! );
//! ```
//!
//! A name is one token tree: a string literal, a variable, or a parenthesized list of parts. A
//! part is a string literal, an integer, a variable or a parenthesized expression.
//! `("Mastery" i "Level")` is the name `Mastery3Level` for `i == 3`. A read hashes the parts
//! forwards from the state of the section and builds no string.
//!
//! In a section, a statement is one of:
//!
//! - `int "A", ("B" i);`: reads each name with the type of the reader method: `int`, `float`,
//!   `string`, `vec2`, `vec3`, `vec4`, or `key` for no type.
//! - `read (ty) "A", "B";`: reads each name with the `Option<Ty>` value `ty`.
//! - `seq int "Item" from 1;`: reads `Item1`, `Item2`, and so on, up to the first missing one.
//! - `let v = int "A";`: binds the result of a read.
//! - `if let v = string "A" { ... }`, `if (condition) { ... } else { ... }`.
//! - `for i in 1..5 { ... }`, `for k in ["A", "B"] { ... }`, `for k in LIST { ... }`.
//! - `for i in 1.. while int ("A" i) { ... }`: runs up to the first index without the key.
//!   `while string ("A" i) => v` binds the value.
//! - `stale { ... }`, `stale unless (condition) { ... }`: the names of the block are stale.
//! - `use f(args);`: calls the fragment `f(r, section, args)`.
//! - `[parts] { ... }`: reads the section named by the parts. `with s { ... }` reads the section
//!   `s`.
//!
//! - `if has "A" { ... }`: runs the block if the file has the key. Any reader method works in
//!   place of `has`.
//! - `let v = name ("A" i);` binds a name, `let v = (expression);` binds a value.
//!
//! At the top level, a statement is a section block, `for`, `stale`, `if (condition)`, `let`, or
//! `use f(args);`, which calls `f(r, args)`. `for i in 1.. while [parts] int "A" { ... }` reads
//! the sections `parts` for `i` from 1, up to the first section without the key.

pub(crate) mod character;
pub(crate) mod globals;
pub(crate) mod level;
pub(crate) mod particles;
pub(crate) mod scene;
pub(crate) mod spell;
pub(crate) mod troybin;
pub(crate) mod ui;

/// Returns the parts of a name, as a slice of [`Part`](crate::reader::Part).
macro_rules! name {
    ($text:literal) => {
        &[$crate::reader::Part::from($text)]
    };
    (($($part:tt)+)) => {
        &[$($crate::recipes::name!(@part $part)),+]
    };
    (@part ($e:expr)) => {
        $crate::reader::Part::from($e)
    };
    (@part $part:tt) => {
        $crate::reader::AsPart::as_part(&$part)
    };
    ($var:ident) => {
        &[$crate::reader::AsPart::as_part(&$var)]
    };
}

/// Returns the type of a reader method.
macro_rules! ty {
    (key) => {
        None
    };
    (int) => {
        Some($crate::Ty::Int)
    };
    (float) => {
        Some($crate::Ty::Float)
    };
    (string) => {
        Some($crate::Ty::String)
    };
    (vec2) => {
        Some($crate::Ty::Vec2)
    };
    (vec3) => {
        Some($crate::Ty::Vec3)
    };
    (vec4) => {
        Some($crate::Ty::Vec4)
    };
}

/// Reads the sections and properties of a recipe. See the [module](self) documentation.
macro_rules! recipe {
    // --- entry ---------------------------------------------------------------------------------
    ($r:ident, $s:ident => $($body:tt)*) => {
        $crate::recipes::recipe!(@sec $r, $s; $($body)*)
    };
    ($r:ident => $($body:tt)*) => {
        $crate::recipes::recipe!(@top $r; $($body)*)
    };

    // --- top level -----------------------------------------------------------------------------
    (@top $r:ident;) => {};
    (@top $r:ident; [$($section:tt)+] { $($body:tt)* } $($rest:tt)*) => {
        {
            let section = $r.section($crate::recipes::name!(($($section)+)));
            $crate::recipes::recipe!(@sec $r, section; $($body)*);
        }
        $crate::recipes::recipe!(@top $r; $($rest)*);
    };
    (@top $r:ident; for $i:ident in $a:literal .. while [$($section:tt)+] $ty:ident $name:tt { $($body:tt)* } $($rest:tt)*) => {
        for $i in $a.. {
            let section = $r.section($crate::recipes::name!(($($section)+)));
            if !$crate::reader::Found::found(&$r.$ty(section, $crate::recipes::name!($name))) {
                break;
            }
            $crate::recipes::recipe!(@sec $r, section; $($body)*);
        }
        $crate::recipes::recipe!(@top $r; $($rest)*);
    };
    (@top $r:ident; for $i:ident in $a:literal .. $b:literal { $($body:tt)* } $($rest:tt)*) => {
        for $i in $a..$b {
            $crate::recipes::recipe!(@top $r; $($body)*);
        }
        $crate::recipes::recipe!(@top $r; $($rest)*);
    };
    (@top $r:ident; for $i:ident in ($e:expr) { $($body:tt)* } $($rest:tt)*) => {
        for $i in $e {
            $crate::recipes::recipe!(@top $r; $($body)*);
        }
        $crate::recipes::recipe!(@top $r; $($rest)*);
    };
    (@top $r:ident; for $i:ident in $list:tt { $($body:tt)* } $($rest:tt)*) => {
        for $i in $list {
            $crate::recipes::recipe!(@top $r; $($body)*);
        }
        $crate::recipes::recipe!(@top $r; $($rest)*);
    };
    (@top $r:ident; stale { $($body:tt)* } $($rest:tt)*) => {
        $r.stale(|$r| {
            $crate::recipes::recipe!(@top $r; $($body)*);
        });
        $crate::recipes::recipe!(@top $r; $($rest)*);
    };
    (@top $r:ident; if ($c:expr) { $($body:tt)* } $($rest:tt)*) => {
        if $c {
            $crate::recipes::recipe!(@top $r; $($body)*);
        }
        $crate::recipes::recipe!(@top $r; $($rest)*);
    };
    (@top $r:ident; let $v:ident = name $name:tt; $($rest:tt)*) => {
        let $v = $crate::recipes::name!($name);
        $crate::recipes::recipe!(@top $r; $($rest)*);
    };
    (@top $r:ident; let $v:ident = ($e:expr); $($rest:tt)*) => {
        let $v = $e;
        $crate::recipes::recipe!(@top $r; $($rest)*);
    };
    (@top $r:ident; use $f:ident($($arg:expr),* $(,)?); $($rest:tt)*) => {
        $f($r $(, $arg)*);
        $crate::recipes::recipe!(@top $r; $($rest)*);
    };

    // --- section -------------------------------------------------------------------------------
    (@sec $r:ident, $s:ident;) => {};
    (@sec $r:ident, $s:ident; [$($section:tt)+] { $($body:tt)* } $($rest:tt)*) => {
        {
            let section = $r.section($crate::recipes::name!(($($section)+)));
            $crate::recipes::recipe!(@sec $r, section; $($body)*);
        }
        $crate::recipes::recipe!(@sec $r, $s; $($rest)*);
    };
    (@sec $r:ident, $s:ident; with $other:ident { $($body:tt)* } $($rest:tt)*) => {
        $crate::recipes::recipe!(@sec $r, $other; $($body)*);
        $crate::recipes::recipe!(@sec $r, $s; $($rest)*);
    };
    (@sec $r:ident, $s:ident; for $i:ident in $a:literal .. while [$($section:tt)+] $ty:ident $name:tt { $($body:tt)* } $($rest:tt)*) => {
        for $i in $a.. {
            let section = $r.section($crate::recipes::name!(($($section)+)));
            if !$crate::reader::Found::found(&$r.$ty(section, $crate::recipes::name!($name))) {
                break;
            }
            $crate::recipes::recipe!(@sec $r, section; $($body)*);
        }
        $crate::recipes::recipe!(@sec $r, $s; $($rest)*);
    };
    (@sec $r:ident, $s:ident; for $i:ident in $a:literal .. while $ty:ident $name:tt => $v:ident { $($body:tt)* } $($rest:tt)*) => {
        for $i in $a.. {
            let Some($v) = $r.$ty($s, $crate::recipes::name!($name)) else {
                break;
            };
            $crate::recipes::recipe!(@sec $r, $s; $($body)*);
        }
        $crate::recipes::recipe!(@sec $r, $s; $($rest)*);
    };
    (@sec $r:ident, $s:ident; for $i:ident in $a:literal .. while $ty:ident $name:tt { $($body:tt)* } $($rest:tt)*) => {
        for $i in $a.. {
            if !$crate::reader::Found::found(&$r.$ty($s, $crate::recipes::name!($name))) {
                break;
            }
            $crate::recipes::recipe!(@sec $r, $s; $($body)*);
        }
        $crate::recipes::recipe!(@sec $r, $s; $($rest)*);
    };
    (@sec $r:ident, $s:ident; for $i:ident in $a:literal .. $b:literal { $($body:tt)* } $($rest:tt)*) => {
        for $i in $a..$b {
            $crate::recipes::recipe!(@sec $r, $s; $($body)*);
        }
        $crate::recipes::recipe!(@sec $r, $s; $($rest)*);
    };
    (@sec $r:ident, $s:ident; for $i:ident in ($e:expr) { $($body:tt)* } $($rest:tt)*) => {
        for $i in $e {
            $crate::recipes::recipe!(@sec $r, $s; $($body)*);
        }
        $crate::recipes::recipe!(@sec $r, $s; $($rest)*);
    };
    (@sec $r:ident, $s:ident; for $i:ident in $list:tt { $($body:tt)* } $($rest:tt)*) => {
        for $i in $list {
            $crate::recipes::recipe!(@sec $r, $s; $($body)*);
        }
        $crate::recipes::recipe!(@sec $r, $s; $($rest)*);
    };
    (@sec $r:ident, $s:ident; stale unless ($c:expr) { $($body:tt)* } $($rest:tt)*) => {
        let client = $c;
        $r.stale_unless(client, |$r| {
            $crate::recipes::recipe!(@sec $r, $s; $($body)*);
        });
        $crate::recipes::recipe!(@sec $r, $s; $($rest)*);
    };
    (@sec $r:ident, $s:ident; stale { $($body:tt)* } $($rest:tt)*) => {
        $r.stale(|$r| {
            $crate::recipes::recipe!(@sec $r, $s; $($body)*);
        });
        $crate::recipes::recipe!(@sec $r, $s; $($rest)*);
    };
    (@sec $r:ident, $s:ident; if let $v:ident = $ty:ident $name:tt { $($body:tt)* } $($rest:tt)*) => {
        if let Some($v) = $r.$ty($s, $crate::recipes::name!($name)) {
            $crate::recipes::recipe!(@sec $r, $s; $($body)*);
        }
        $crate::recipes::recipe!(@sec $r, $s; $($rest)*);
    };
    (@sec $r:ident, $s:ident; if ($c:expr) { $($then:tt)* } else { $($else:tt)* } $($rest:tt)*) => {
        if $c {
            $crate::recipes::recipe!(@sec $r, $s; $($then)*);
        } else {
            $crate::recipes::recipe!(@sec $r, $s; $($else)*);
        }
        $crate::recipes::recipe!(@sec $r, $s; $($rest)*);
    };
    (@sec $r:ident, $s:ident; if ($c:expr) { $($then:tt)* } $($rest:tt)*) => {
        if $c {
            $crate::recipes::recipe!(@sec $r, $s; $($then)*);
        }
        $crate::recipes::recipe!(@sec $r, $s; $($rest)*);
    };
    (@sec $r:ident, $s:ident; if $ty:ident $name:tt { $($body:tt)* } $($rest:tt)*) => {
        if $crate::reader::Found::found(&$r.$ty($s, $crate::recipes::name!($name))) {
            $crate::recipes::recipe!(@sec $r, $s; $($body)*);
        }
        $crate::recipes::recipe!(@sec $r, $s; $($rest)*);
    };
    (@sec $r:ident, $s:ident; let $v:ident = name $name:tt; $($rest:tt)*) => {
        let $v = $crate::recipes::name!($name);
        $crate::recipes::recipe!(@sec $r, $s; $($rest)*);
    };
    (@sec $r:ident, $s:ident; let $v:ident = ($e:expr); $($rest:tt)*) => {
        let $v = $e;
        $crate::recipes::recipe!(@sec $r, $s; $($rest)*);
    };
    (@sec $r:ident, $s:ident; let $v:ident = $ty:ident $name:tt; $($rest:tt)*) => {
        let $v = $r.$ty($s, $crate::recipes::name!($name));
        $crate::recipes::recipe!(@sec $r, $s; $($rest)*);
    };
    (@sec $r:ident, $s:ident; seq $ty:ident $name:tt from $start:literal; $($rest:tt)*) => {
        $r.seq($s, $crate::recipes::name!($name), $start, $crate::recipes::ty!($ty));
        $crate::recipes::recipe!(@sec $r, $s; $($rest)*);
    };
    (@sec $r:ident, $s:ident; use $f:ident($($arg:expr),* $(,)?); $($rest:tt)*) => {
        $f($r, $s $(, $arg)*);
        $crate::recipes::recipe!(@sec $r, $s; $($rest)*);
    };
    (@sec $r:ident, $s:ident; read ($t:expr) $($name:tt),+; $($rest:tt)*) => {
        $( $r.typed($s, $crate::recipes::name!($name), $t); )+
        $crate::recipes::recipe!(@sec $r, $s; $($rest)*);
    };
    (@sec $r:ident, $s:ident; $ty:ident $($name:tt),+; $($rest:tt)*) => {
        $( let _ = $r.$ty($s, $crate::recipes::name!($name)); )+
        $crate::recipes::recipe!(@sec $r, $s; $($rest)*);
    };
}

pub(crate) use {name, recipe, ty};
