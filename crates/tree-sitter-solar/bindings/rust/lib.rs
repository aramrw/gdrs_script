//! This crate provides solar language support for the [tree-sitter][] parsing library.
//!
//! [tree-sitter]: https://tree-sitter.github.io/tree-sitter/

use tree_sitter::Language;

extern "C" {
    fn tree_sitter_solar() -> Language;
}

/// Get the tree-sitter [Language][] for this grammar.
///
/// [Language]: https://docs.rs/tree-sitter/*/tree_sitter/struct.Language.html
pub fn language() -> Language {
    unsafe { tree_sitter_solar() }
}

/// The content of the [`node-types.json`][] file for this grammar.
///
/// [`node-types.json`]: https://tree-sitter.github.io/tree-sitter/using-parsers#node-types
pub const NODE_TYPES: &str = include_str!("../../src/node-types.json");

// Uncomment these if you decide to add highlighting/tags/etc. later
// pub const HIGHLIGHTS_QUERY: &str = include_str!("../../queries/highlights.scm");
// pub const TAGS_QUERY: &str = include_str!("../../queries/tags.scm");
// pub const INJECTIONS_QUERY: &str = include_str!("../../queries/injections.scm");
