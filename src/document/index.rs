// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! The attached definitions of a document by normalized label, which edits keep up to date.
//!
//! The definitions indexed when the document is built come in document order, which is arena
//! order, so each label's are sorted once, with their keys, most of which are slices of the
//! source, and found by binary search. Edits only remove those, so the remaining ones of each label
//! stay in document order, and the first is found directly. Definitions indexed by edits are kept
//! by label apart, with the first of each label, among those built too, cached: inserting one
//! compares it with the first, and removing the first leaves the cache to be recomputed by the next
//! lookup, which finds the first of them all in one walk over their ancestors.

use super::{Node, NodeId};
use alloc::{
    borrow::Cow,
    collections::{btree_map::Entry, BTreeMap, BTreeSet},
    string::String,
    vec::Vec,
};
use core::{
    num::NonZeroUsize,
    ops::Range,
    sync::atomic::{AtomicUsize, Ordering},
};

// A normalized label: a slice of the source where the label is already normalized there, as most
// are, or the normalized form.
#[derive(Clone, Debug)]
enum Key {
    Source(Range<usize>),
    Owned(String),
}

impl Key {
    fn get<'k>(&'k self, source: &'k str) -> &'k str {
        match self {
            Self::Source(range) => &source[range.clone()],
            Self::Owned(key) => key,
        }
    }
}

// A definition indexed when the document was built.
#[derive(Clone, Debug)]
struct Built {
    key: Key,
    id: NodeId,
    alive: bool,
    // In the first entry of each key, how far after it the first alive entry of the key is.
    skip: usize,
}

// The definitions of one label indexed by edits.
#[derive(Debug)]
struct Added {
    ids: BTreeSet<NodeId>,
    // The first definition of the label in document order, counting those built, or 0 when it is
    // to be recomputed. An atomic lets a lookup cache it without making documents unshareable.
    first: AtomicUsize,
}

impl Clone for Added {
    fn clone(&self) -> Self {
        Self {
            ids: self.ids.clone(),
            first: AtomicUsize::new(self.first.load(Ordering::Relaxed)),
        }
    }
}

fn raw(id: NodeId) -> usize {
    id.0.get()
}

fn cooked(raw: usize) -> Option<NodeId> {
    NonZeroUsize::new(raw).map(NodeId)
}

/// The attached definitions of one kind, link reference or footnote, by normalized label.
#[derive(Clone, Debug, Default)]
pub(super) struct Labels {
    // Sorted by key, then by ID, which is document order.
    built: Vec<Built>,
    added: BTreeMap<String, Added>,
}

impl Labels {
    // Index definitions given sorted by key and then in document order.
    fn sorted(entries: impl Iterator<Item = (Key, NodeId)>) -> Self {
        Self {
            built: entries
                .map(|(key, id)| Built {
                    key,
                    id,
                    alive: true,
                    skip: 0,
                })
                .collect(),
            added: BTreeMap::new(),
        }
    }

    // Index definitions with `keys` in document order, `order` their indices sorted by key and
    // then in document order, and `ids` their nodes.
    fn parsed(keys: Vec<Key>, order: &[usize], ids: &[NodeId]) -> Self {
        assert_eq!(
            keys.len(),
            ids.len(),
            "the parser yields each definition it finds"
        );
        let mut keys: Vec<Option<Key>> = keys.into_iter().map(Some).collect();
        Self::sorted(order.iter().map(|&index| {
            (
                keys[index].take().expect("each definition is ordered once"),
                ids[index],
            )
        }))
    }

    /// Index definitions given in document order with their keys. In arena order too, they are
    /// built; otherwise they are indexed as edits index them, the first of each label known.
    pub(super) fn walked(mut entries: Vec<(String, NodeId)>) -> Self {
        if entries.windows(2).all(|pair| pair[0].1 < pair[1].1) {
            // A stable sort keeps each label's definitions in document order.
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            return Self::sorted(entries.into_iter().map(|(key, id)| (Key::Owned(key), id)));
        }
        let mut added = BTreeMap::<String, Added>::new();
        for (key, id) in entries {
            added
                .entry(key)
                .or_insert_with(|| Added {
                    ids: BTreeSet::new(),
                    first: AtomicUsize::new(raw(id)),
                })
                .ids
                .insert(id);
        }
        Self {
            built: Vec::new(),
            added,
        }
    }

    // The entries built of `key`.
    fn group(&self, source: &str, key: &str) -> Range<usize> {
        let start = self
            .built
            .partition_point(|entry| entry.key.get(source) < key);
        let end = start + self.built[start..].partition_point(|entry| entry.key.get(source) == key);
        start..end
    }

    // The first alive definition built of `key`.
    fn first_built(&self, source: &str, key: &str) -> Option<NodeId> {
        let group = self.group(source, key);
        let at = group.start + self.built.get(group.start)?.skip;
        (at < group.end).then(|| self.built[at].id)
    }

    /// The first attached definition of `key` in document order.
    pub(super) fn first(&self, nodes: &[Node], source: &str, key: &str) -> Option<NodeId> {
        let built = self.first_built(source, key);
        let Some(added) = self.added.get(key) else {
            return built;
        };
        if let Some(first) = cooked(added.first.load(Ordering::Relaxed)) {
            return Some(first);
        }
        let first = first_of(
            nodes,
            built.into_iter().chain(added.ids.iter().copied()),
            None,
        )
        .expect("an added label has a definition");
        added.first.store(raw(first), Ordering::Relaxed);
        Some(first)
    }

    /// The first attached definition of `key` in document order that is not below `parent`.
    pub(super) fn first_outside(
        &self,
        nodes: &[Node],
        source: &str,
        key: &str,
        parent: NodeId,
    ) -> Option<NodeId> {
        let group = self.group(source, key);
        let built = self.built[group]
            .iter()
            .filter(|entry| entry.alive)
            .map(|entry| entry.id);
        let added = self
            .added
            .get(key)
            .into_iter()
            .flat_map(|added| added.ids.iter().copied());
        first_of(nodes, built.chain(added), Some(parent))
    }

    /// Index an attached definition of `key`, once it is in place.
    pub(super) fn insert(&mut self, nodes: &[Node], source: &str, key: String, id: NodeId) {
        let built = self.first_built(source, &key);
        match self.added.entry(key) {
            Entry::Vacant(entry) => {
                let first = match built {
                    Some(built) if !precedes(nodes, id, built) => built,
                    _ => id,
                };
                entry.insert(Added {
                    ids: BTreeSet::from([id]),
                    first: AtomicUsize::new(raw(first)),
                });
            }
            Entry::Occupied(mut entry) => {
                let added = entry.get_mut();
                added.ids.insert(id);
                let first = added.first.get_mut();
                if cooked(*first).is_some_and(|first| precedes(nodes, id, first)) {
                    *first = raw(id);
                }
            }
        }
    }

    /// Stop indexing a definition of `key`.
    pub(super) fn remove(&mut self, source: &str, key: String, id: NodeId) {
        let group = self.group(source, &key);
        if let Ok(at) = self.built[group.clone()].binary_search_by_key(&id, |entry| entry.id) {
            let at = group.start + at;
            if self.built[at].alive {
                self.built[at].alive = false;
                let head = group.start;
                let mut first = head + self.built[head].skip;
                while first < group.end && !self.built[first].alive {
                    first += 1;
                }
                self.built[head].skip = first - head;
                if let Some(added) = self.added.get_mut(&key) {
                    if *added.first.get_mut() == raw(id) {
                        *added.first.get_mut() = 0;
                    }
                }
                return;
            }
        }
        let Entry::Occupied(mut entry) = self.added.entry(key) else {
            unreachable!("an indexed definition is under its key");
        };
        let added = entry.get_mut();
        assert!(
            added.ids.remove(&id),
            "an indexed definition is under its key"
        );
        if added.ids.is_empty() {
            entry.remove();
        } else if *added.first.get_mut() == raw(id) {
            *added.first.get_mut() = 0;
        }
    }
}

/// The keys of the definitions a parse found and their order, kept apart from the source so that
/// the source can move into the document.
pub(super) struct Parsed {
    references: (Vec<Key>, Vec<usize>),
    footnotes: (Vec<Key>, Vec<usize>),
}

impl Parsed {
    /// Take the keys and order of the definitions a parse of `source` found.
    pub(super) fn new(source: &str, definitions: crate::definitions::Definitions<'_>) -> Self {
        let base = source.as_ptr() as usize;
        let (keys, order, footnote_keys, footnote_order) = definitions.into_keys();
        let keys = keys
            .into_iter()
            .map(|key| match key {
                // A borrowed key is a slice of the source.
                Cow::Borrowed(key) => {
                    let start = key.as_ptr() as usize - base;
                    Key::Source(start..start + key.len())
                }
                Cow::Owned(key) => Key::Owned(key),
            })
            .collect();
        Self {
            references: (keys, order),
            footnotes: (
                footnote_keys.into_iter().map(Key::Owned).collect(),
                footnote_order,
            ),
        }
    }

    /// Index the definitions, given their nodes, which the parser yields in the order it found
    /// them, document order.
    pub(super) fn index(self, references: &[NodeId], footnotes: &[NodeId]) -> DefinitionIndex {
        DefinitionIndex {
            references: Labels::parsed(self.references.0, &self.references.1, references),
            footnotes: Labels::parsed(self.footnotes.0, &self.footnotes.1, footnotes),
        }
    }
}

/// The attached link reference and footnote definitions of a document.
#[derive(Clone, Debug, Default)]
pub(super) struct DefinitionIndex {
    pub(super) references: Labels,
    pub(super) footnotes: Labels,
}

impl DefinitionIndex {
    /// Whether no definition is indexed, as for a document built without any, which edits have
    /// not added to.
    pub(super) fn is_empty(&self) -> bool {
        [&self.references, &self.footnotes]
            .iter()
            .all(|labels| labels.built.is_empty() && labels.added.is_empty())
    }

    pub(super) fn labels(&self, footnote: bool) -> &Labels {
        if footnote {
            &self.footnotes
        } else {
            &self.references
        }
    }

    pub(super) fn labels_mut(&mut self, footnote: bool) -> &mut Labels {
        if footnote {
            &mut self.footnotes
        } else {
            &mut self.references
        }
    }
}

// Whether `a` comes before `b` in document order, both being in one tree: an ancestor comes
// before its descendants, and otherwise the two paths part at siblings. These are scanned forward
// from both at once, so that the scan stops after the shorter of the run from the one to the
// other and the run from the later one to the end. Nodes in different trees precede neither.
pub(super) fn precedes(nodes: &[Node], a: NodeId, b: NodeId) -> bool {
    let path = |id: NodeId| {
        let mut path: Vec<_> =
            core::iter::successors(Some(id), |id| nodes[id.index()].parent).collect();
        path.reverse();
        path
    };
    let (a, b) = (path(a), path(b));
    let common = a.iter().zip(&b).take_while(|(a, b)| a == b).count();
    match (a.get(common), b.get(common)) {
        (Some(&a), Some(&b)) => {
            let (mut after_a, mut after_b) = (Some(a), Some(b));
            loop {
                after_a = after_a.and_then(|id| nodes[id.index()].next_sibling);
                after_b = after_b.and_then(|id| nodes[id.index()].next_sibling);
                if after_a == Some(b) {
                    return true;
                }
                // `b`'s siblings come to an end before `a`'s reach it, so `b` is not after `a`,
                // or `a`'s run out before `b`'s, which is not reached from `a` then either.
                if after_b == Some(a) || after_a.is_none() {
                    return false;
                }
                if after_b.is_none() {
                    return true;
                }
            }
        }
        // One path holds the other, so one node is an ancestor of the other, or is it.
        (a, _) => a.is_none() && b.len() > common,
    }
}

// The first of `candidates`, attached nodes, in document order, not counting those below
// `exclude`. Their ancestors are marked, each once, then the tree is searched from the root
// through marked nodes only, scanning the children of each node searched until a marked one.
fn first_of(
    nodes: &[Node],
    candidates: impl Iterator<Item = NodeId>,
    exclude: Option<NodeId>,
) -> Option<NodeId> {
    // Whether each marked node is a candidate. The root is not one.
    let root = NodeId::new(0);
    let mut marked = BTreeMap::from([(root, false)]);
    for candidate in candidates {
        if let Some(mark) = marked.get_mut(&candidate) {
            *mark = true;
            continue;
        }
        marked.insert(candidate, true);
        let mut ancestor = nodes[candidate.index()].parent;
        while let Some(id) = ancestor {
            // An ancestor already marked keeps its mark, which may be a candidate's.
            if let Entry::Vacant(entry) = marked.entry(id) {
                entry.insert(false);
            } else {
                break;
            }
            ancestor = nodes[id.index()].parent;
        }
    }
    let mut pending = Vec::from([nodes[root.index()].first_child]);
    while let Some(next) = pending.last_mut() {
        let Some(id) = *next else {
            pending.pop();
            continue;
        };
        *next = nodes[id.index()].next_sibling;
        match marked.get(&id) {
            Some(true) => return Some(id),
            Some(false) if Some(id) != exclude => pending.push(nodes[id.index()].first_child),
            _ => {}
        }
    }
    None
}
