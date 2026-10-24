// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Authored definitions with a sorted, first-definition-wins lookup index.
use crate::stream::Text;
use crate::util::normalize_identifier::normalize_identifier;
use alloc::{string::String, vec::Vec};
use core::ops::Range;

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
        self.ordered.extend(0..self.entries.len());
        let entries = &self.entries;
        self.ordered
            .sort_unstable_by(|&a, &b| entries[a].key.cmp(&entries[b].key).then(a.cmp(&b)));
        self.ordered
            .dedup_by(|a, b| entries[*a].key == entries[*b].key);
    }

    pub fn find(&self, key: &str) -> Option<usize> {
        self.ordered
            .binary_search_by(|&index| self.entries[index].key.as_str().cmp(key))
            .ok()
            .map(|index| self.ordered[index])
    }
}

/// Scan a single-line full-reference label. The grammar producer handles multiline labels.
pub(crate) fn scan_label(bytes: &[u8], start: usize, end: usize) -> Option<(usize, Range<usize>)> {
    let begin = start + 1;
    let mut at = begin;
    let mut content = false;
    let mut tab_expansion = 0;
    while at < end && at - begin + tab_expansion <= crate::util::constant::LINK_REFERENCE_SIZE_MAX {
        match bytes[at] {
            b']' => return content.then_some((at + 1, begin..at)),
            b'[' | b'\r' | b'\n' => return None,
            b'\\' if at + 1 < end && matches!(bytes[at + 1], b'[' | b'\\' | b']') => {
                content = true;
                at += 1;
            }
            // The grammar counts virtual tab spaces toward its label limit. Use an upper bound so only potentially oversized labels need its exact column accounting, including stripped container prefixes.
            b'\t' => tab_expansion += 3,
            b' ' => {}
            _ => content = true,
        }
        at += 1;
    }
    None
}
