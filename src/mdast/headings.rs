// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Helpers for extracting headings from an mdast tree.
use super::Node;
use alloc::{
    collections::{BTreeMap, BTreeSet},
    string::{String, ToString},
    vec::Vec,
};

/// Options for extracting headings from a document.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HeadingOptions {
    /// The shallowest heading level to include (between `1` and `6`).
    pub min_level: u8,
    /// The deepest heading level to include (between `1` and `6`).
    pub max_level: u8,
}

impl Default for HeadingOptions {
    fn default() -> Self {
        Self {
            min_level: 1,
            max_level: 6,
        }
    }
}

/// A heading extracted from an mdast tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HeadingEntry<'a> {
    /// The original heading node.
    pub node: &'a Node,
    /// The heading level, between `1` and `6`.
    pub level: u8,
    /// The heading's text content.
    pub text: String,
    /// A unique, lowercase anchor derived from the heading text.
    pub anchor: String,
}

/// Headings extracted from a Markdown document.
///
/// This helper is useful for building a table of contents. It walks the tree
/// in document order, converts each heading to plain text, and assigns unique
/// anchors to repeated headings. Anchor assignment includes headings filtered
/// out by [`HeadingOptions`], so the remaining anchors still match rendered
/// headings when [`CompileOptions::heading_ids`][crate::CompileOptions::heading_ids]
/// is enabled.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Headings<'a> {
    entries: Vec<HeadingEntry<'a>>,
}

impl<'a> Headings<'a> {
    /// Extract all headings from a document tree.
    #[must_use]
    pub fn extract(root: &'a Node) -> Self {
        Self::extract_with_options(root, &HeadingOptions::default())
    }

    /// Extract headings from a document tree with a level range.
    #[must_use]
    pub fn extract_with_options(root: &'a Node, options: &HeadingOptions) -> Self {
        let mut result = Self::default();
        let mut anchors = AnchorGenerator::default();

        root.walk(|node| {
            if let Node::Heading(heading) = node {
                let text = chomp_line_ending(&node.text_content()).to_string();
                let anchor = anchors.anchor_for(&text);

                if heading.depth >= options.min_level && heading.depth <= options.max_level {
                    result.entries.push(HeadingEntry {
                        node,
                        level: heading.depth,
                        text,
                        anchor,
                    });
                }
            }
        });

        result
    }

    /// Return the extracted entries as a slice.
    #[must_use]
    pub fn as_slice(&self) -> &[HeadingEntry<'a>] {
        &self.entries
    }

    /// Iterate over the extracted entries in document order.
    pub fn iter(&self) -> core::slice::Iter<'_, HeadingEntry<'a>> {
        self.entries.iter()
    }

    /// Return the number of extracted headings.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the document contains no headings in the selected level range.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl<'a, 'b> IntoIterator for &'b Headings<'a> {
    type Item = &'b HeadingEntry<'a>;
    type IntoIter = core::slice::Iter<'b, HeadingEntry<'a>>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter()
    }
}

impl<'a> IntoIterator for Headings<'a> {
    type Item = HeadingEntry<'a>;
    type IntoIter = alloc::vec::IntoIter<HeadingEntry<'a>>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}

#[derive(Default)]
pub(crate) struct AnchorGenerator {
    next_suffix: BTreeMap<String, usize>,
    used: BTreeSet<String>,
}

impl AnchorGenerator {
    pub(crate) fn anchor_for(&mut self, text: &str) -> String {
        let base = slug(text);
        let mut suffix = self.next_suffix.get(&base).copied().unwrap_or(2);
        let mut anchor = base.clone();

        while self.used.contains(&anchor) {
            anchor = alloc::format!("{base}-{suffix}");
            suffix += 1;
        }

        self.next_suffix.insert(base, suffix);
        self.used.insert(anchor.clone());
        anchor
    }
}

fn slug(text: &str) -> String {
    let mut result = String::new();
    let mut previous_was_whitespace = false;
    let lowercase = text.to_lowercase();

    for character in lowercase.chars() {
        if character.is_whitespace() {
            if !previous_was_whitespace {
                result.push('-');
            }
            previous_was_whitespace = true;
        } else {
            result.push(character);
            previous_was_whitespace = false;
        }
    }

    result
}

fn chomp_line_ending(text: &str) -> &str {
    if let Some(text) = text.strip_suffix("\r\n") {
        text
    } else if let Some(text) = text.strip_suffix('\n') {
        text
    } else if let Some(text) = text.strip_suffix('\r') {
        text
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::{chomp_line_ending, slug, HeadingOptions, Headings};
    use crate::mdast::{Heading, Node, Root, Text};
    use alloc::{string::String, vec, vec::Vec};

    fn heading(depth: u8, text: &str) -> Node {
        Node::Heading(Heading {
            position: None,
            depth,
            children: vec![Node::Text(Text {
                value: String::from(text),
                position: None,
            })],
        })
    }

    fn root(children: Vec<Node>) -> Node {
        Node::Root(Root {
            position: None,
            children,
        })
    }

    #[test]
    fn extracts_headings_with_filtered_unique_anchors() {
        let document = root(vec![
            heading(1, "Title"),
            heading(2, "Title"),
            heading(3, "Other"),
            heading(6, "Title"),
        ]);
        let headings = Headings::extract_with_options(
            &document,
            &HeadingOptions {
                min_level: 2,
                max_level: 3,
            },
        );

        assert_eq!(headings.len(), 2);
        assert!(!headings.is_empty());
        assert_eq!(headings.as_slice()[0].text, "Title");
        assert_eq!(headings.as_slice()[0].anchor, "title-2");
        assert_eq!(headings.as_slice()[1].text, "Other");
        assert_eq!(headings.as_slice()[1].anchor, "other");
        assert_eq!(headings.iter().count(), 2);
        assert_eq!((&headings).into_iter().count(), 2);
        assert_eq!(
            headings
                .into_iter()
                .map(|entry| entry.anchor)
                .collect::<Vec<_>>(),
            ["title-2", "other"]
        );

        let empty = Headings::extract_with_options(
            &document,
            &HeadingOptions {
                min_level: 4,
                max_level: 3,
            },
        );
        assert!(empty.is_empty());
        assert_eq!(empty.len(), 0);
        assert_eq!(empty.as_slice(), &[]);
    }

    #[test]
    fn extracts_all_headings_and_resolves_explicit_slug_collisions() {
        let document = root(vec![
            heading(1, "Heading"),
            heading(2, "Heading"),
            heading(3, "Heading-2"),
            heading(4, "Heading"),
        ]);
        let headings = Headings::extract(&document);

        assert_eq!(headings.len(), 4);
        assert_eq!(
            headings
                .iter()
                .map(|entry| entry.anchor.as_str())
                .collect::<Vec<_>>(),
            ["heading", "heading-2", "heading-2-2", "heading-3"]
        );
        assert_eq!(
            headings.iter().map(|entry| entry.level).collect::<Vec<_>>(),
            [1, 2, 3, 4]
        );
        assert_eq!(
            headings
                .iter()
                .map(|entry| entry.node)
                .collect::<Vec<_>>()
                .len(),
            4
        );
    }

    #[test]
    fn normalizes_slug_whitespace_and_chomps_line_endings() {
        assert_eq!(slug("  Crème\r\nTea \t"), "-crème-tea-");
        assert_eq!(slug(""), "");
        assert_eq!(chomp_line_ending("line\r\n"), "line");
        assert_eq!(chomp_line_ending("line\n"), "line");
        assert_eq!(chomp_line_ending("line\r"), "line");
        assert_eq!(chomp_line_ending("line"), "line");
    }
}
