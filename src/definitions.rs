// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Authored definitions with a sorted, first-definition-wins lookup index.
use crate::stream::Text;
use crate::util::normalize_identifier::{
    normalize_identifier, normalize_reference_identifier_into,
};
use alloc::{borrow::Cow, string::String, vec::Vec};

#[derive(Clone, Debug)]
pub(crate) struct Definition<'a> {
    /// The byte offset where the authored definition begins.
    pub start: usize,
    pub label: Text<'static>,
    pub key: Cow<'a, str>,
    pub destination: Text<'static>,
    pub title: Option<Text<'static>>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Definitions<'a> {
    pub entries: Vec<Definition<'a>>,
    ordered: Vec<usize>,
    footnotes: Vec<String>,
    footnote_starts: Vec<usize>,
    footnote_order: Vec<usize>,
    // Lookups ignore definitions starting at or after this offset. Incremental parsing scopes each top-level block to the definitions authored before its end.
    limit: Option<usize>,
}

impl<'a> Definitions<'a> {
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn push(
        &mut self,
        source: &'a str,
        start: usize,
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
            start,
            label,
            key,
            destination,
            title,
        });
        index
    }

    pub fn finish(&mut self) {
        build_index(&mut self.ordered, &self.entries, |entry| entry.key.as_ref());
        build_index(&mut self.footnote_order, &self.footnotes, String::as_str);
    }

    // The index keeps the first authored definition of each key, so an invisible match has no visible predecessor.
    pub fn find(&self, key: &str) -> Option<usize> {
        find_index(&self.ordered, &self.entries, key, |entry| {
            entry.key.as_ref()
        })
        .filter(|&index| self.visible(self.entries[index].start))
    }

    #[inline]
    fn visible(&self, start: usize) -> bool {
        self.limit.map_or(true, |limit| start < limit)
    }

    /// Restrict lookups to definitions starting before `limit`, or remove the restriction.
    pub fn set_limit(&mut self, limit: Option<usize>) {
        self.limit = limit;
    }

    /// Copy the definitions starting before `offset` into owned storage, so they can seed a later parse of the following source.
    pub fn before(&self, offset: usize) -> Definitions<'static> {
        let entries = self.entries.partition_point(|entry| entry.start < offset);
        let footnotes = self
            .footnote_starts
            .partition_point(|&start| start < offset);
        Definitions {
            entries: self.entries[..entries]
                .iter()
                .map(|entry| Definition {
                    start: entry.start,
                    label: entry.label.clone(),
                    key: Cow::Owned(entry.key.clone().into_owned()),
                    destination: entry.destination.clone(),
                    title: entry.title.clone(),
                })
                .collect(),
            ordered: Vec::new(),
            footnotes: self.footnotes[..footnotes].to_vec(),
            footnote_starts: self.footnote_starts[..footnotes].to_vec(),
            footnote_order: Vec::new(),
            limit: None,
        }
    }

    pub fn push_footnote(&mut self, source: &str, start: usize, label: &Text<'_>) -> usize {
        let index = self.footnotes.len();
        self.footnote_starts.push(start);
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
        find_index(&self.footnote_order, &self.footnotes, key, String::as_str)
            .filter(|&index| self.visible(self.footnote_starts[index]))
    }
}

// Sort equal keys by authored position so deduplication keeps the first definition.
pub(crate) fn build_index<T>(order: &mut Vec<usize>, values: &[T], key: impl Fn(&T) -> &str) {
    order.clear();
    order.extend(0..values.len());
    order.sort_unstable_by(|&a, &b| key(&values[a]).cmp(key(&values[b])).then(a.cmp(&b)));
    order.dedup_by(|a, b| key(&values[*a]) == key(&values[*b]));
}

pub(crate) fn find_index<T>(
    order: &[usize],
    values: &[T],
    value: &str,
    key: impl Fn(&T) -> &str,
) -> Option<usize> {
    order
        .binary_search_by(|&index| key(&values[index]).cmp(value))
        .ok()
        .map(|index| order[index])
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
    if is_canonical_reference_key(value) {
        Cow::Borrowed(value)
    } else {
        Cow::Owned(owned_reference_key(value))
    }
}

fn is_canonical_reference_key(value: &str) -> bool {
    let mut previous = b' ';
    for byte in value.bytes() {
        if !byte.is_ascii()
            || byte.is_ascii_uppercase()
            || matches!(byte, b'\t' | b'\r' | b'\n')
            || byte == b' ' && previous == b' '
        {
            return false;
        }
        previous = byte;
    }
    value.is_empty() || previous != b' '
}

#[cfg(test)]
mod tests {
    use super::{normalize_reference_identifier_into, reference_key};
    use alloc::{borrow::Cow, string::String};

    #[test]
    fn reference_keys_borrow_canonical_ascii_and_normalize_other_labels() {
        for (value, expected, borrowed) in [
            ("", "", true),
            ("key", "key", true),
            ("a b", "a b", true),
            ("a\u{b}b\u{c}c", "a\u{b}b\u{c}c", true),
            (" A ", "a", false),
            ("a ", "a", false),
            (" \t\r\n ", "", false),
            ("a  b", "a b", false),
            ("\ta\r\nb\t", "a b", false),
            ("ẞ", "ss", false),
            ("SS", "ss", false),
            ("ϴ A", "Θ a", false),
            ("θ a", "Θ a", false),
            ("İ", "i\u{307}", false),
            ("a\u{a0}B", "a\u{a0}b", false),
        ] {
            let key = reference_key(value);
            assert_eq!(matches!(key, Cow::Borrowed(_)), borrowed);
            if borrowed {
                assert_eq!(key.as_ptr(), value.as_ptr());
            }
            assert_eq!(key, expected);
            let mut scratch = String::from("old value");
            normalize_reference_identifier_into(value, &mut scratch);
            assert_eq!(scratch, expected);
        }
    }
}
