// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Authored definitions with a sorted, first-definition-wins lookup index.
use crate::stream::Text;
use crate::util::normalize_identifier::{
    normalize_identifier, normalize_reference_identifier_into,
};
use alloc::{borrow::Cow, collections::BTreeMap, string::String, vec::Vec};

#[derive(Clone, Debug)]
pub(crate) struct Definition<'a> {
    /// The byte offset where the authored definition begins.
    pub start: usize,
    pub label: Text<'static>,
    pub key: Cow<'a, str>,
    pub destination: Text<'static>,
    pub title: Option<Text<'static>>,
}

/// Owned definitions authored before a point in the source, indexed as they are appended so that incremental parsing never re-sorts them.
#[derive(Debug, Default)]
pub(crate) struct Seed {
    entries: Vec<Definition<'static>>,
    index: BTreeMap<String, usize>,
    footnotes: Vec<String>,
    footnote_starts: Vec<usize>,
    footnote_index: BTreeMap<String, usize>,
}

impl Seed {
    /// Append definitions authored after every existing one, keeping the first definition of each key.
    pub fn append(&mut self, authored: Authored) {
        for entry in authored.entries {
            let index = self.entries.len();
            self.index
                .entry(entry.key.clone().into_owned())
                .or_insert(index);
            self.entries.push(entry);
        }
        for (start, key) in authored.footnotes {
            let index = self.footnotes.len();
            self.footnote_index.entry(key.clone()).or_insert(index);
            self.footnotes.push(key);
            self.footnote_starts.push(start);
        }
    }
}

/// Owned copies of definitions taken from a parse, ready to append to a [`Seed`].
pub(crate) struct Authored {
    entries: Vec<Definition<'static>>,
    footnotes: Vec<(usize, String)>,
}

/// The definitions of a document outside a fragment being parsed into it, which bind the
/// fragment's references and footnote calls that the fragment's own do not.
pub(crate) trait Outside: core::fmt::Debug {
    /// Whether a definition outside the fragment binds `key`, a footnote identifier where
    /// `footnote`, and a reference key otherwise.
    fn binds(&self, footnote: bool, key: &str) -> bool;
}

#[derive(Debug, Default)]
pub(crate) struct Definitions<'a> {
    // Definitions authored before the parsed source; indices below its lengths refer to it.
    seed: Option<&'a Seed>,
    entries: Vec<Definition<'a>>,
    ordered: Vec<usize>,
    footnotes: Vec<String>,
    footnote_starts: Vec<usize>,
    footnote_order: Vec<usize>,
    // Lookups ignore definitions starting at or after this offset. Incremental parsing scopes each top-level block to the definitions authored before its end.
    limit: Option<usize>,
    // The definitions outside the parsed source, with the keys asked of them so far, each of a
    // footnote or not, with the index of the entry or footnote it was given, if they bind it, so
    // that each key is asked once, those of references first. Those entries follow the source's
    // own.
    outside: Option<&'a dyn Outside>,
    asked: [BTreeMap<String, Option<usize>>; 2],
    found: Vec<Definition<'a>>,
    found_footnotes: Vec<String>,
}

impl<'a> Definitions<'a> {
    /// Create definitions that extend `seed` with those of the parsed source.
    pub fn seeded(seed: &'a Seed) -> Self {
        Self {
            seed: Some(seed),
            ..Self::default()
        }
    }

    /// Create definitions of a fragment of a document whose other definitions are `outside`.
    pub fn outside(outside: &'a dyn Outside) -> Self {
        Self {
            outside: Some(outside),
            ..Self::default()
        }
    }

    // Whether no reference can be bound, so that none need be looked up.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
            && self.seed.is_none_or(|seed| seed.entries.is_empty())
            && self.outside.is_none()
    }

    // The index that a key the source does not define has outside it, if a definition there
    // binds it, asking `outside` once for each key.
    fn find_outside(&mut self, footnote: bool, key: &str) -> Option<usize> {
        let outside = self.outside?;
        if let Some(&index) = self.asked[usize::from(footnote)].get(key) {
            return index;
        }
        let index = outside.binds(footnote, key).then(|| {
            if footnote {
                self.found_footnotes.push(String::from(key));
                self.seed_footnotes() + self.footnotes.len() + self.found_footnotes.len() - 1
            } else {
                // A document stores a reference by its name, so the definition is not read.
                self.found.push(Definition {
                    start: 0,
                    label: Text::Literal(""),
                    key: Cow::Owned(String::from(key)),
                    destination: Text::Literal(""),
                    title: None,
                });
                self.seed_entries() + self.entries.len() + self.found.len() - 1
            }
        });
        self.asked[usize::from(footnote)].insert(String::from(key), index);
        index
    }

    fn seed_entries(&self) -> usize {
        self.seed.map_or(0, |seed| seed.entries.len())
    }

    fn seed_footnotes(&self) -> usize {
        self.seed.map_or(0, |seed| seed.footnotes.len())
    }

    pub fn entry(&self, index: usize) -> &Definition<'a> {
        match self.seed {
            Some(seed) if index < seed.entries.len() => &seed.entries[index],
            _ => {
                let own = index - self.seed_entries();
                self.entries
                    .get(own)
                    .unwrap_or_else(|| &self.found[own - self.entries.len()])
            }
        }
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
        let index = self.seed_entries() + self.entries.len();
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

    // The index keeps the first authored definition of each key, so an invisible match has no visible predecessor. Seeded definitions precede the parsed source.
    // A key the source does not define is looked up outside it.
    pub fn find(&mut self, key: &str) -> Option<usize> {
        let index = match self.seed.and_then(|seed| seed.index.get(key)) {
            Some(&index) => index,
            None => {
                match find_index(&self.ordered, &self.entries, key, |entry| {
                    entry.key.as_ref()
                }) {
                    Some(index) => index + self.seed_entries(),
                    None => return self.find_outside(false, key),
                }
            }
        };
        Some(index).filter(|&index| self.visible(self.entry(index).start))
    }

    #[inline]
    fn visible(&self, start: usize) -> bool {
        self.limit.is_none_or(|limit| start < limit)
    }

    /// Restrict lookups to definitions starting before `limit`, or remove the restriction.
    pub fn set_limit(&mut self, limit: Option<usize>) {
        self.limit = limit;
    }

    /// Copy the definitions of the parsed source starting before `offset` into owned storage, so they can extend the seed of a later parse of the following source.
    pub fn before(&self, offset: usize) -> Authored {
        let entries = self.entries.partition_point(|entry| entry.start < offset);
        let footnotes = self
            .footnote_starts
            .partition_point(|&start| start < offset);
        Authored {
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
            footnotes: self.footnote_starts[..footnotes]
                .iter()
                .copied()
                .zip(self.footnotes[..footnotes].iter().cloned())
                .collect(),
        }
    }

    pub fn push_footnote(&mut self, source: &str, start: usize, label: &Text<'_>) -> usize {
        let index = self.seed_footnotes() + self.footnotes.len();
        self.footnote_starts.push(start);
        self.footnotes.push(normalize_identifier(
            label
                .resolve(source)
                .expect("footnote labels are source-valid"),
        ));
        index
    }

    pub fn footnote_key(&self, index: usize) -> &str {
        match self.seed {
            Some(seed) if index < seed.footnotes.len() => &seed.footnotes[index],
            _ => {
                let own = index - self.seed_footnotes();
                self.footnotes
                    .get(own)
                    .unwrap_or_else(|| &self.found_footnotes[own - self.footnotes.len()])
            }
        }
    }

    pub fn find_footnote(&mut self, key: &str) -> Option<usize> {
        if let Some(&index) = self.seed.and_then(|seed| seed.footnote_index.get(key)) {
            return Some(index)
                .filter(|&index| self.visible(self.seed.unwrap().footnote_starts[index]));
        }
        let Some(index) = find_index(&self.footnote_order, &self.footnotes, key, String::as_str)
        else {
            return self.find_outside(true, key);
        };
        self.visible(self.footnote_starts[index])
            .then_some(index + self.seed_footnotes())
    }
}

// Sort equal keys by authored position so deduplication keeps the first definition.
pub(crate) fn build_index<T>(order: &mut Vec<usize>, values: &[T], key: impl Fn(&T) -> &str) {
    order.clear();
    order.extend(0..values.len());
    crate::work::step!(values.len());
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
pub(crate) fn owned_reference_key(value: &str) -> String {
    let mut key = String::with_capacity(value.len());
    normalize_reference_identifier_into(value, &mut key);
    key
}

pub(crate) fn reference_key(value: &str) -> Cow<'_, str> {
    if is_canonical_reference_key(value) {
        Cow::Borrowed(value)
    } else {
        Cow::Owned(owned_reference_key(value))
    }
}

// The reference key of `value`: the value itself when it is its own key, and otherwise the key,
// normalized into `scratch`.
pub(crate) fn scratch_reference_key<'v>(value: &'v str, scratch: &'v mut String) -> &'v str {
    if is_canonical_reference_key(value) {
        value
    } else {
        normalize_reference_identifier_into(value, scratch);
        scratch
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
