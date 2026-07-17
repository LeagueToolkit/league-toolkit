use crate::cst::Kind;

use super::state::TypeChecker;

impl TypeChecker<'_> {
    /// Prints the current traversal stack on tree enter/exit. `arrow` is `">"` on enter, `"<"`
    /// on exit. No-op unless the `debug` feature is enabled and `RB_STACK` is set.
    #[cfg(feature = "debug")]
    pub(super) fn trace_stack(&self, depth: u32, arrow: &str, kind: Kind) {
        if std::env::var("RB_STACK").is_err() {
            return;
        }
        let indent = "  ".repeat(depth.saturating_sub(1) as _);
        eprintln!("{indent}{arrow} d:{depth} | {kind:?}");
        eprint!("{indent}  stack: ");
        if self.stack.is_empty() {
            eprint!("empty");
        }
        eprintln!();
        for s in &self.stack {
            eprintln!("{indent}    - {}: {:?}", s.0, s.1);
        }
    }
    #[cfg(not(feature = "debug"))]
    pub(super) fn trace_stack(&self, _depth: u32, _arrow: &str, _kind: Kind) {}

    /// Prints the depth of a just-popped stack entry when exiting a tree, indented to
    /// `indent_depth`. No-op unless the `debug` feature is enabled and `RB_STACK` is set.
    #[cfg(feature = "debug")]
    pub(super) fn trace_popped(&self, indent_depth: u32, popped_depth: u32) {
        if std::env::var("RB_STACK").is_ok() {
            let indent = "  ".repeat(indent_depth.saturating_sub(1) as _);
            eprintln!("{indent}< popped {popped_depth}");
        }
    }
    #[cfg(not(feature = "debug"))]
    pub(super) fn trace_popped(&self, _indent_depth: u32, _popped_depth: u32) {}
}

/// One-off debug trace message. No-op unless the `debug` feature is enabled and `RB_STACK` is
/// set - arguments are only evaluated when tracing is active.
macro_rules! trace {
    ($($arg:tt)*) => {
        #[cfg(feature = "debug")]
        if ::std::env::var("RB_STACK").is_ok() {
            eprintln!($($arg)*);
        }
    };
}
pub(super) use trace;
