// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use crate::NodeId;
use alloc::{
    collections::{BTreeMap, BTreeSet},
    string::String,
};

/// Heading levels to include when building a table of contents.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HeadingOptions {
    /// Shallowest included level.
    pub min_level: u8,
    /// Deepest included level.
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
/// A heading's stable node ID, plain text and unique HTML anchor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HeadingEntry {
    /// Heading node.
    pub node: NodeId,
    /// Heading level.
    pub level: u8,
    /// Semantic plain text.
    pub text: String,
    /// Anchor shared with the HTML writer's heading IDs.
    pub anchor: String,
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
