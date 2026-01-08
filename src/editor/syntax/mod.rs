//! Syntax module for tree-sitter integration.
//!
//! Provides:
//! - Language detection from file extensions
//! - Unified syntax state with incremental parsing
//! - Syntax highlighting with caching

mod highlight;
mod language;

pub use highlight::{highlight_style, HighlightResult, HighlightSpan, HIGHLIGHT_NAMES};
pub use language::Language;

use ratatui::style::Style;
use tree_sitter::{InputEdit, Parser, Tree};
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

/// Unified syntax state managing both parsing and highlighting.
///
/// This struct combines the parser (for incremental tree updates) with
/// the highlighter (for syntax coloring), enabling efficient updates
/// when the buffer changes.
pub struct SyntaxState {
    language: Language,
    parser: Parser,
    tree: Option<Tree>,
    config: HighlightConfiguration,
    highlighter: Highlighter,
    /// Cached highlight spans (invalidated on edit)
    cached_spans: Vec<HighlightSpan>,
    /// Whether cache is valid
    pub cache_valid: bool,
}

impl SyntaxState {
    /// Create a new SyntaxState for the given language.
    /// Returns None for PlainText (no syntax highlighting needed).
    pub fn new(language: Language) -> Option<Self> {
        match language {
            Language::PlainText => None,
            lang => {
                let mut parser = Parser::new();

                // Set parser language
                let set_result = match lang {
                    Language::Rust => parser.set_language(&tree_sitter_rust::LANGUAGE.into()),
                    Language::Python => parser.set_language(&tree_sitter_python::LANGUAGE.into()),
                    Language::JavaScript => {
                        parser.set_language(&tree_sitter_javascript::LANGUAGE.into())
                    }
                    Language::TypeScript => {
                        parser.set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
                    }
                    Language::Tsx => {
                        parser.set_language(&tree_sitter_typescript::LANGUAGE_TSX.into())
                    }
                    Language::Go => parser.set_language(&tree_sitter_go::LANGUAGE.into()),
                    Language::C => parser.set_language(&tree_sitter_c::LANGUAGE.into()),
                    Language::Cpp => parser.set_language(&tree_sitter_cpp::LANGUAGE.into()),
                    Language::Json => parser.set_language(&tree_sitter_json::LANGUAGE.into()),
                    Language::Markdown => parser.set_language(&tree_sitter_md::LANGUAGE.into()),
                    Language::Html => parser.set_language(&tree_sitter_html::LANGUAGE.into()),
                    Language::Css => parser.set_language(&tree_sitter_css::LANGUAGE.into()),
                    Language::Java => parser.set_language(&tree_sitter_java::LANGUAGE.into()),
                    Language::PlainText => return None,
                };

                match set_result {
                    Ok(_) => {}
                    Err(e) => {
                        log::warn!("Failed to set parser language for {:?}: {}", lang, e);
                        return None;
                    }
                }

                // Build highlight configuration
                let config = match Self::build_highlight_config(lang) {
                    Some(c) => c,
                    None => {
                        log::warn!("Failed to build highlight config for {:?}", lang);
                        return None;
                    }
                };

                Some(Self {
                    language: lang,
                    parser,
                    tree: None,
                    config,
                    highlighter: Highlighter::new(),
                    cached_spans: Vec::new(),
                    cache_valid: false,
                })
            }
        }
    }

    /// Build the highlight configuration for a language.
    fn build_highlight_config(language: Language) -> Option<HighlightConfiguration> {
        let mut config = match language {
            Language::Rust => HighlightConfiguration::new(
                tree_sitter_rust::LANGUAGE.into(),
                "rust",
                tree_sitter_rust::HIGHLIGHTS_QUERY,
                tree_sitter_rust::INJECTIONS_QUERY,
                "",
            ),
            Language::Python => HighlightConfiguration::new(
                tree_sitter_python::LANGUAGE.into(),
                "python",
                tree_sitter_python::HIGHLIGHTS_QUERY,
                "",
                "",
            ),
            Language::JavaScript => {
                // Combine base highlights with JSX highlights for JSX support
                let combined_highlights = format!(
                    "{}\n{}",
                    tree_sitter_javascript::HIGHLIGHT_QUERY,
                    tree_sitter_javascript::JSX_HIGHLIGHT_QUERY
                );
                HighlightConfiguration::new(
                    tree_sitter_javascript::LANGUAGE.into(),
                    "javascript",
                    &combined_highlights,
                    tree_sitter_javascript::INJECTIONS_QUERY,
                    tree_sitter_javascript::LOCALS_QUERY,
                )
            }
            Language::TypeScript => {
                // TypeScript queries extend JavaScript, so combine them
                let combined_highlights = format!(
                    "{}\n{}",
                    tree_sitter_javascript::HIGHLIGHT_QUERY,
                    tree_sitter_typescript::HIGHLIGHTS_QUERY
                );
                HighlightConfiguration::new(
                    tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
                    "typescript",
                    &combined_highlights,
                    tree_sitter_javascript::INJECTIONS_QUERY,
                    tree_sitter_typescript::LOCALS_QUERY,
                )
            }
            Language::Tsx => {
                // TSX queries extend JavaScript + TypeScript + JSX
                let combined_highlights = format!(
                    "{}\n{}\n{}",
                    tree_sitter_javascript::HIGHLIGHT_QUERY,
                    tree_sitter_javascript::JSX_HIGHLIGHT_QUERY,
                    tree_sitter_typescript::HIGHLIGHTS_QUERY
                );
                HighlightConfiguration::new(
                    tree_sitter_typescript::LANGUAGE_TSX.into(),
                    "tsx",
                    &combined_highlights,
                    tree_sitter_javascript::INJECTIONS_QUERY,
                    tree_sitter_typescript::LOCALS_QUERY,
                )
            }
            Language::Go => HighlightConfiguration::new(
                tree_sitter_go::LANGUAGE.into(),
                "go",
                tree_sitter_go::HIGHLIGHTS_QUERY,
                "",
                "",
            ),
            Language::C => HighlightConfiguration::new(
                tree_sitter_c::LANGUAGE.into(),
                "c",
                tree_sitter_c::HIGHLIGHT_QUERY,
                "",
                "",
            ),
            Language::Cpp => HighlightConfiguration::new(
                tree_sitter_cpp::LANGUAGE.into(),
                "cpp",
                tree_sitter_cpp::HIGHLIGHT_QUERY,
                "",
                "",
            ),
            Language::Json => HighlightConfiguration::new(
                tree_sitter_json::LANGUAGE.into(),
                "json",
                tree_sitter_json::HIGHLIGHTS_QUERY,
                "",
                "",
            ),
            Language::Markdown => {
                // tree-sitter-md has separate block and inline grammars
                HighlightConfiguration::new(
                    tree_sitter_md::LANGUAGE.into(),
                    "markdown",
                    tree_sitter_md::HIGHLIGHT_QUERY_BLOCK,
                    tree_sitter_md::INJECTION_QUERY_BLOCK,
                    "",
                )
            }
            Language::Html => HighlightConfiguration::new(
                tree_sitter_html::LANGUAGE.into(),
                "html",
                tree_sitter_html::HIGHLIGHTS_QUERY,
                tree_sitter_html::INJECTIONS_QUERY,
                "",
            ),
            Language::Css => HighlightConfiguration::new(
                tree_sitter_css::LANGUAGE.into(),
                "css",
                tree_sitter_css::HIGHLIGHTS_QUERY,
                "",
                "",
            ),
            Language::Java => HighlightConfiguration::new(
                tree_sitter_java::LANGUAGE.into(),
                "java",
                tree_sitter_java::HIGHLIGHTS_QUERY,
                "",
                "",
            ),
            Language::PlainText => return None,
        };

        match config {
            Ok(ref mut c) => {
                c.configure(HIGHLIGHT_NAMES);
                config.ok()
            }
            Err(e) => {
                log::warn!("Failed to create highlight configuration: {}", e);
                None
            }
        }
    }

    /// Apply an incremental edit to the tree.
    /// Called before parsing to inform tree-sitter what changed.
    pub fn apply_edit(&mut self, edit: InputEdit) {
        match &mut self.tree {
            Some(tree) => tree.edit(&edit),
            None => {}
        }
        self.cache_valid = false;
    }

    /// Parse the source text.
    /// Uses incremental parsing if a previous tree exists.
    pub fn parse(&mut self, source: &str) {
        self.tree = self.parser.parse(source, self.tree.as_ref());
    }

    /// Parse with a timeout to prevent hanging on malformed input.
    /// Returns true if parse succeeded, false if timed out.
    pub fn parse_with_timeout(&mut self, source: &str, timeout_ms: u64) -> bool {
        match timeout_ms {
            0 => self.parser.set_timeout_micros(0), // No timeout
            ms => self.parser.set_timeout_micros(ms * 1000),
        }

        self.tree = self.parser.parse(source, self.tree.as_ref());

        match &self.tree {
            Some(_) => true,
            None => {
                log::warn!("Parse timed out after {}ms", timeout_ms);
                false
            }
        }
    }

    /// Get highlight spans, computing if needed.
    /// Uses cached spans if available and valid.
    ///
    /// NOTE: Takes &mut self because tree-sitter-highlight's Highlighter
    /// uses internal mutable state and we cache the result. Conceptually
    /// this is a read operation with lazy evaluation.
    pub fn get_highlights(&mut self, source: &str) -> &[HighlightSpan] {
        match self.cache_valid {
            true => &self.cached_spans,
            false => {
                match self.compute_highlights(source) {
                    HighlightResult::Success(spans) => {
                        self.cached_spans = spans;
                    }
                    HighlightResult::PartialSuccess { spans, error: _ } => {
                        self.cached_spans = spans;
                    }
                    HighlightResult::Failure(_) => {
                        self.cached_spans.clear();
                    }
                }
                self.cache_valid = true;
                &self.cached_spans
            }
        }
    }

    /// Compute highlights with detailed result information.
    ///
    /// NOTE: Takes &mut self because tree-sitter-highlight's Highlighter
    /// uses internal mutable state. Conceptually this is a read operation
    /// with caching for performance.
    pub fn compute_highlights(&mut self, source: &str) -> HighlightResult {
        let highlights = match self
            .highlighter
            .highlight(&self.config, source.as_bytes(), None, |_| None)
        {
            Ok(h) => h,
            Err(e) => {
                log::warn!("Syntax highlighting initialization failed: {}", e);
                return HighlightResult::Failure(format!("Highlight init error: {}", e));
            }
        };

        let mut spans = Vec::new();
        let mut style_stack: Vec<Style> = vec![Style::default()];
        let mut had_error = false;
        let mut error_msg = String::new();

        for event in highlights {
            match event {
                Ok(HighlightEvent::Source { start, end }) => {
                    let style = style_stack.last().copied().unwrap_or_default();
                    match style != Style::default() {
                        true => spans.push(HighlightSpan { start, end, style }),
                        false => {}
                    }
                }
                Ok(HighlightEvent::HighlightStart(highlight)) => {
                    let name = HIGHLIGHT_NAMES.get(highlight.0).copied().unwrap_or("");
                    let style = highlight_style(name);
                    style_stack.push(style);
                }
                Ok(HighlightEvent::HighlightEnd) => {
                    style_stack.pop();
                }
                Err(e) => {
                    log::warn!("Syntax highlighting error during iteration: {}", e);
                    had_error = true;
                    error_msg = format!("Highlight error: {}", e);
                    break;
                }
            }
        }

        match had_error {
            true => HighlightResult::PartialSuccess {
                spans,
                error: error_msg,
            },
            false => HighlightResult::Success(spans),
        }
    }

    /// Get the detected language.
    #[allow(dead_code)]
    pub fn language(&self) -> Language {
        self.language
    }

    /// Invalidate the cache, forcing recomputation on next access.
    pub fn invalidate_cache(&mut self) {
        self.cache_valid = false;
    }

    // ==================== Node Navigation ====================

    /// Get the smallest named node containing the given byte range.
    /// Returns (start_byte, end_byte) of the node, or None if no tree exists.
    pub fn get_node_range_at(&self, start_byte: usize, end_byte: usize) -> Option<(usize, usize)> {
        match &self.tree {
            Some(tree) => {
                let root = tree.root_node();
                // Find the smallest named node that contains the range
                let node = Self::find_smallest_containing_node(root, start_byte, end_byte);
                match node {
                    Some(n) => Some((n.start_byte(), n.end_byte())),
                    None => Some((root.start_byte(), root.end_byte())),
                }
            }
            None => None,
        }
    }

    /// Get the parent node's range for expanding selection.
    /// Takes current selection range, returns parent node range.
    pub fn get_parent_node_range(&self, start_byte: usize, end_byte: usize) -> Option<(usize, usize)> {
        match &self.tree {
            Some(tree) => {
                let root = tree.root_node();
                // Find the smallest node that exactly matches or contains the range
                let current_node = Self::find_smallest_containing_node(root, start_byte, end_byte);

                match current_node {
                    Some(node) => {
                        // If current selection exactly matches a node, get its parent
                        match node.start_byte() == start_byte && node.end_byte() == end_byte {
                            true => {
                                // Walk up to find a larger named parent
                                let mut parent = node.parent();
                                while let Some(p) = parent {
                                    match p.is_named() && (p.start_byte() != start_byte || p.end_byte() != end_byte) {
                                        true => return Some((p.start_byte(), p.end_byte())),
                                        false => parent = p.parent(),
                                    }
                                }
                                // No larger parent found, return root
                                Some((root.start_byte(), root.end_byte()))
                            }
                            false => {
                                // Selection doesn't match node exactly, expand to this node first
                                Some((node.start_byte(), node.end_byte()))
                            }
                        }
                    }
                    None => Some((root.start_byte(), root.end_byte())),
                }
            }
            None => None,
        }
    }

    /// Get the first named child's range for shrinking selection.
    /// Takes current selection range, returns first child node range.
    pub fn get_child_node_range(&self, start_byte: usize, end_byte: usize) -> Option<(usize, usize)> {
        match &self.tree {
            Some(tree) => {
                let root = tree.root_node();
                // Find the node that matches the current selection
                let current_node = Self::find_node_at_range(root, start_byte, end_byte);

                match current_node {
                    Some(node) => {
                        // Find first named child
                        let mut cursor = node.walk();
                        match node.named_child(0) {
                            Some(child) => Some((child.start_byte(), child.end_byte())),
                            None => {
                                // No named children, try any children
                                for child in node.children(&mut cursor) {
                                    match child.is_named() {
                                        true => return Some((child.start_byte(), child.end_byte())),
                                        false => continue,
                                    }
                                }
                                // No children at all, stay at current
                                None
                            }
                        }
                    }
                    None => None,
                }
            }
            None => None,
        }
    }

    /// Find the smallest named node that contains the given byte range.
    fn find_smallest_containing_node(
        node: tree_sitter::Node,
        start_byte: usize,
        end_byte: usize,
    ) -> Option<tree_sitter::Node> {
        // Check if this node contains the range
        match node.start_byte() <= start_byte && node.end_byte() >= end_byte {
            true => {
                // Check children for a smaller containing node
                let mut cursor = node.walk();
                for child in node.named_children(&mut cursor) {
                    match Self::find_smallest_containing_node(child, start_byte, end_byte) {
                        Some(smaller) => return Some(smaller),
                        None => continue,
                    }
                }
                // No smaller child found, this node is the smallest
                match node.is_named() {
                    true => Some(node),
                    false => None,
                }
            }
            false => None,
        }
    }

    /// Find a node that exactly matches the given byte range.
    fn find_node_at_range(
        node: tree_sitter::Node,
        start_byte: usize,
        end_byte: usize,
    ) -> Option<tree_sitter::Node> {
        // Check if this node exactly matches
        match node.start_byte() == start_byte && node.end_byte() == end_byte {
            true => Some(node),
            false => {
                // Check if range is within this node
                match node.start_byte() <= start_byte && node.end_byte() >= end_byte {
                    true => {
                        // Search children
                        let mut cursor = node.walk();
                        for child in node.children(&mut cursor) {
                            match Self::find_node_at_range(child, start_byte, end_byte) {
                                Some(found) => return Some(found),
                                None => continue,
                            }
                        }
                        // No exact match in children, return this node as closest
                        Some(node)
                    }
                    false => None,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_syntax_state_new_rust() {
        let state = SyntaxState::new(Language::Rust);
        assert!(state.is_some());
    }

    #[test]
    fn test_syntax_state_new_plain_text() {
        let state = SyntaxState::new(Language::PlainText);
        assert!(state.is_none());
    }

    #[test]
    fn test_syntax_state_parse_and_highlight() {
        let mut state = SyntaxState::new(Language::Rust).expect("Should create Rust state");
        let source = "fn main() { println!(\"Hello\"); }";

        state.parse(source);
        let spans = state.get_highlights(source);

        assert!(!spans.is_empty(), "Should have highlight spans for Rust");
    }

    #[test]
    fn test_syntax_state_cache_invalidation() {
        let mut state = SyntaxState::new(Language::Rust).expect("Should create Rust state");
        let source = "fn main() {}";

        state.parse(source);
        let _ = state.get_highlights(source);
        assert!(state.cache_valid);

        state.invalidate_cache();
        assert!(!state.cache_valid);
    }

    #[test]
    fn test_markdown_highlighter() {
        let mut state =
            SyntaxState::new(Language::Markdown).expect("Markdown state should be created");

        let source = "# Hello World\n\nSome *emphasis* and **bold** text.";
        state.parse(source);
        let spans = state.get_highlights(source);

        assert!(
            !spans.is_empty(),
            "Should have highlight spans for markdown"
        );
    }

    #[test]
    fn test_typescript_highlighter() {
        let mut state =
            SyntaxState::new(Language::TypeScript).expect("TypeScript state should be created");

        let source = r#"
import { useState } from "react";

interface Props {
    name: string;
}

function greet(props: Props): string {
    const message = `Hello, ${props.name}!`;
    return message;
}
"#;
        state.parse(source);
        let spans = state.get_highlights(source);

        assert!(
            !spans.is_empty(),
            "Should have highlight spans for TypeScript"
        );
    }

    #[test]
    fn test_tsx_jsx_highlighter() {
        let mut state = SyntaxState::new(Language::Tsx).expect("TSX state should be created");

        let source = r#"
import React from "react";

function App() {
    return (
        <div className="container">
            <h1>Hello World</h1>
            <button onClick={() => alert("clicked")}>Click me</button>
        </div>
    );
}
"#;
        state.parse(source);
        let spans = state.get_highlights(source);

        assert!(!spans.is_empty(), "Should have highlight spans for TSX");
    }

    #[test]
    fn test_node_navigation() {
        let mut state = SyntaxState::new(Language::Rust).expect("Should create Rust state");
        let source = "fn main() { let x = 42; }";
        //           0123456789...
        // "fn" is at bytes 0-2
        // "main" is at bytes 3-7
        // "42" is at bytes 20-22

        state.parse(source);

        // Test getting node at position (cursor at "42")
        let range = state.get_node_range_at(20, 20);
        assert!(range.is_some(), "Should find node at position");
        let (start, end) = range.expect("range exists");
        assert!(start <= 20 && end >= 20, "Node should contain position");

        // Test parent node expansion
        let parent_range = state.get_parent_node_range(20, 22);
        assert!(parent_range.is_some(), "Should find parent node");
        let (parent_start, parent_end) = parent_range.expect("parent exists");
        assert!(
            parent_start <= 20 && parent_end >= 22,
            "Parent should contain child range"
        );
    }
}
