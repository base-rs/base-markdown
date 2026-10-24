// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Authored definitions with a sorted, first-definition-wins lookup index.
use crate::stream::Text;
use crate::util::normalize_identifier::normalize_identifier;
use alloc::{string::String, vec::Vec};

#[derive(Debug)]
pub(crate) struct Definition {
    pub label: Text<'static>,
    pub key: String,
    pub destination: Text<'static>,
    pub title: Option<Text<'static>>,
}

#[derive(Debug, Default)]
pub(crate) struct Definitions {
    pub entries: Vec<Definition>,
    ordered: Vec<usize>,
    footnotes: Vec<String>,
    footnote_order: Vec<usize>,
}

impl Definitions {
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn push(
        &mut self,
        source: &str,
        label: Text<'static>,
        destination: Text<'static>,
        title: Option<Text<'static>>,
    ) -> usize {
        let key = normalize_identifier(
            label
                .resolve(source)
                .expect("definition labels are source-valid"),
        );
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
            .binary_search_by(|&index| self.entries[index].key.as_str().cmp(key))
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
