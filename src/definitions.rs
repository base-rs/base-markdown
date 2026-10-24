// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Authored definitions with a sorted, first-definition-wins lookup index.
use crate::stream::Text;
use crate::util::normalize_identifier::{
    normalize_identifier, normalize_reference_identifier_into,
};
use alloc::{borrow::Cow, string::String, vec::Vec};

#[derive(Debug)]
pub(crate) struct Definition<'a> {
    pub label: Text<'static>,
    pub key: Cow<'a, str>,
    pub destination: Text<'static>,
    pub title: Option<Text<'static>>,
}

#[derive(Debug, Default)]
pub(crate) struct Definitions<'a> {
    pub entries: Vec<Definition<'a>>,
    ordered: Vec<usize>,
    footnotes: Vec<String>,
    footnote_order: Vec<usize>,
}

impl<'a> Definitions<'a> {
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn push(
        &mut self,
        source: &'a str,
        label: Text<'static>,
        destination: Text<'static>,
        title: Option<Text<'static>>,
    ) -> usize {
        let key = match &label {
            Text::Source(range) => reference_key(&source[range.clone()]),
            _ => Cow::Owned(owned_reference_key(
                label
                    .resolve(source)
                    .expect("definition labels are source-valid"),
            )),
        };
        let index = self.entries.len();
        self.entries.push(Definition {
            label,
            key,
            destination,
            title,
        });
        index
    }

    pub fn finish(&mut self) {
        self.ordered.clear();
        self.ordered.extend(0..self.entries.len());
        let entries = &self.entries;
        self.ordered
            .sort_unstable_by(|&a, &b| entries[a].key.cmp(&entries[b].key).then(a.cmp(&b)));
        self.ordered
            .dedup_by(|a, b| entries[*a].key == entries[*b].key);
        self.footnote_order.clear();
        self.footnote_order.extend(0..self.footnotes.len());
        let footnotes = &self.footnotes;
        self.footnote_order
            .sort_unstable_by(|&a, &b| footnotes[a].cmp(&footnotes[b]).then(a.cmp(&b)));
        self.footnote_order
            .dedup_by(|a, b| footnotes[*a] == footnotes[*b]);
    }

    pub fn find(&self, key: &str) -> Option<usize> {
        self.ordered
            .binary_search_by(|&index| self.entries[index].key.as_ref().cmp(key))
            .ok()
            .map(|index| self.ordered[index])
    }

    pub fn push_footnote(&mut self, source: &str, label: &Text<'_>) -> usize {
        let index = self.footnotes.len();
        self.footnotes.push(normalize_identifier(
            label
                .resolve(source)
                .expect("footnote labels are source-valid"),
        ));
        index
    }

    pub fn footnote_key(&self, index: usize) -> &str {
        &self.footnotes[index]
    }

    pub fn find_footnote(&self, key: &str) -> Option<usize> {
        self.footnote_order
            .binary_search_by(|&index| self.footnotes[index].as_str().cmp(key))
            .ok()
            .map(|index| self.footnote_order[index])
    }
}

// Reference keys fold ASCII letters to lowercase after the shared Unicode fold.
// This preserves equality while allowing ordinary lowercase source labels to be borrowed.
// Footnote keys retain their existing uppercase spelling because they appear in HTML IDs.
fn owned_reference_key(value: &str) -> String {
    let mut key = String::with_capacity(value.len());
    normalize_reference_identifier_into(value, &mut key);
    key
}

fn reference_key(value: &str) -> Cow<'_, str> {
    if !value.starts_with(' ')
        && !value.ends_with(' ')
        && value.bytes().all(|byte| {
            byte.is_ascii() && !byte.is_ascii_uppercase() && !matches!(byte, b'\t' | b'\r' | b'\n')
        })
        && !value.contains("  ")
    {
        Cow::Borrowed(value)
    } else {
        Cow::Owned(owned_reference_key(value))
    }
}

#[cfg(test)]
mod tests {
    use super::{normalize_reference_identifier_into, reference_key};
    use alloc::{borrow::Cow, string::String};

    #[test]
    fn canonical_ascii_reference_keys_borrow_the_input() {
        for value in ["", "key", "a b", "a\u{b}b\u{c}c"] {
            let key = reference_key(value);
            assert!(matches!(key, Cow::Borrowed(_)));
            assert_eq!(key.as_ptr(), value.as_ptr());
            assert_eq!(key, value);
        }
    }

    #[test]
    fn owned_reference_keys_preserve_whitespace_and_unicode_equality() {
        for (value, expected) in [
            (" A ", "a"),
            ("a ", "a"),
            (" \t\r\n ", ""),
            ("a  b", "a b"),
            ("\ta\r\nb\t", "a b"),
            ("ẞ", "ss"),
            ("SS", "ss"),
            ("ϴ A", "Θ a"),
            ("θ a", "Θ a"),
            ("İ", "i\u{307}"),
            ("a\u{a0}B", "a\u{a0}b"),
        ] {
            let key = reference_key(value);
            assert!(matches!(key, Cow::Owned(_)), "{:?}", value);
            assert_eq!(key, expected);
            let mut scratch = String::from("old value");
            normalize_reference_identifier_into(value, &mut scratch);
            assert_eq!(scratch, expected);
        }
    }
}
