// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! The bindings of a document's references: which definition each label names.
use crate::definitions::scratch_reference_key;
use crate::document::{next_descendant, Node, NodeId};
use crate::stream::{Content, EventKind, Tag, Target, Text};
use crate::ReferenceKind;
use alloc::{
    boxed::Box,
    collections::{BTreeMap, VecDeque},
    string::String,
    vec,
    vec::Vec,
};

/// The definition each label of the attached document names, built in one pass in document
/// order.
///
/// A reference is a name and a definition a binding of it: the first link reference definition
/// in document order whose label matches a reference's, as the parser matches labels, is the one
/// it leads to, and the first footnote definition with a footnote call's identifier to end, which
/// is one nested in another of its identifier before the other, is the one it calls. A document stores no binding, so the next table built sees every edit to its
/// definitions. A `Bindings` borrows the document, so it cannot outlive an edit.
///
/// [`Document::bindings`][crate::Document::bindings] builds the table of the attached document,
/// in one pass over it, filing each definition by a hash of its key, with an overflow ordered
/// by key for keys whose hashes collide, so that filing one and each lookup in it take constant
/// time usually and at most time logarithmic in the number of keys, whatever they are. It
/// covers the attached document only: a detached subtree is a tree of its own, whose references its own
/// definitions bind, so [`Bindings::destination`] gives `None` for a node in one, and
/// [`Document::destination`][crate::Document::destination] looks it up in its own tree.
#[derive(Clone, Debug)]
pub struct Bindings<'d> {
    source: &'d str,
    nodes: &'d [Node],
    table: Table,
    // Whether each node is in the attached document.
    attached: Vec<bool>,
}

fn text<'d>(value: &'d Text<'_>, source: &'d str) -> &'d str {
    value.resolve(source).expect("document text is validated")
}

// Room for the key looked up, when it is not the label itself.
#[derive(Clone, Debug, Default)]
pub(crate) struct Scratch {
    key: String,
}

// The nodes of one key: the first inserted, which is its binding, and the copies of it inserted
// after, in the order inserted.
#[derive(Clone, Debug)]
struct Entry {
    first: NodeId,
    more: VecDeque<NodeId>,
}

impl Entry {
    fn nodes(&self) -> impl Iterator<Item = NodeId> + '_ {
        core::iter::once(self.first).chain(self.more.iter().copied())
    }
}

// The entries of one kind of key, filed by a hash of the key in groups of a few slots, so that a
// lookup usually compares one key. A key whose group is full goes to an overflow ordered by key,
// so that no set of keys, such as one whose hashes collide, makes a lookup or an insertion take
// more than time logarithmic in the keys: the hash has no seed, which this crate has no source
// of, so the bound rests on the overflow alone. The keys are kept in one string, so filing one
// allocates nothing, but now and then to grow.
#[derive(Clone, Debug, Default)]
struct KeyMap {
    // `GROUP` slots a group, a power of two of groups, or none, each slot the index of an entry
    // in `entries` or `EMPTY`, with the low bits of its key's hash, so that a lookup reads only
    // the entries whose bits match.
    slots: Vec<(u32, u32)>,
    // Whether each group has had a key go to the overflow since the slots were last made, so
    // that a lookup in any other reads the overflow only when the group is full.
    overflowed: Vec<bool>,
    entries: Vec<Filed>,
    // The entries of keys whose group was full when they were filed, by key.
    overflow: BTreeMap<String, u32>,
    // The keys of `entries`, one after another, with those of removed entries until compacted.
    names: String,
    // How many bytes of `names` are keys of `entries`.
    live: usize,
}

// An entry with its key's hash and where its key is in the names.
#[derive(Clone, Debug)]
struct Filed {
    hash: u64,
    name: core::ops::Range<usize>,
    entry: Entry,
}

// How many slots a group has.
const GROUP: usize = 8;
const EMPTY: u32 = u32::MAX;

// A hash of `key`, a word at a time, whose high bits pick its group.
fn hash_key(key: &str) -> u64 {
    const K: u64 = 0x517c_c1b7_2722_0a95;
    let mix = |hash: u64, word: u64| (hash.rotate_left(5) ^ word).wrapping_mul(K);
    let bytes = key.as_bytes();
    let mut chunks = bytes.chunks_exact(8);
    let mut hash = (bytes.len() as u64).wrapping_mul(K);
    for chunk in &mut chunks {
        let mut word = [0; 8];
        word.copy_from_slice(chunk);
        hash = mix(hash, u64::from_le_bytes(word));
    }
    let rest = chunks.remainder();
    if !rest.is_empty() {
        let mut word = [0; 8];
        word[..rest.len()].copy_from_slice(rest);
        hash = mix(hash, u64::from_le_bytes(word));
    }
    hash
}

// The low bits of `hash`, which a slot keeps.
fn low_bits(hash: u64) -> u32 {
    // Truncating keeps the low bits.
    #[allow(clippy::cast_possible_truncation)]
    let low = hash as u32;
    low
}

// Where a key is filed: its slot in a group, or the overflow.
enum Place {
    Slot(usize),
    Overflow,
}

impl KeyMap {
    // The index of the group of `hash`, picked by its high bits, which every bit of the key
    // reaches.
    fn group_index(&self, hash: u64) -> usize {
        let bits = self.overflowed.len().trailing_zeros();
        if bits == 0 {
            return 0;
        }
        // At most as many bits as a group index has are kept.
        #[allow(clippy::cast_possible_truncation)]
        let group = (hash >> (64 - bits)) as usize;
        group
    }

    // The slots of the group of `hash`. Those that are filled come first.
    fn group(&self, hash: u64) -> core::ops::Range<usize> {
        let group = self.group_index(hash);
        group * GROUP..(group + 1) * GROUP
    }

    fn name(&self, filed: &Filed) -> &str {
        &self.names[filed.name.clone()]
    }

    // Where `key`, of hash `hash`, is filed, and its entry's index.
    fn find(&self, hash: u64, key: &str) -> Option<(Place, usize)> {
        if self.slots.is_empty() {
            return None;
        }
        for slot in self.group(hash) {
            let (at, low) = self.slots[slot];
            if at == EMPTY {
                break;
            }
            if low == low_bits(hash) {
                let filed = &self.entries[at as usize];
                if filed.hash == hash && self.name(filed) == key {
                    return Some((Place::Slot(slot), at as usize));
                }
            }
        }
        if !self.overflowed[self.group_index(hash)] {
            return None;
        }
        self.overflow
            .get(key)
            .map(|&at| (Place::Overflow, at as usize))
    }

    fn get(&self, key: &str) -> Option<&Entry> {
        let (_, at) = self.find(hash_key(key), key)?;
        Some(&self.entries[at].entry)
    }

    // The entry of `key`, which is filed.
    fn entry_mut(&mut self, key: &str) -> &mut Entry {
        let (_, at) = self
            .find(hash_key(key), key)
            .expect("an indexed node is in the entry of its key");
        &mut self.entries[at].entry
    }

    // The entry of `key`, or `None` once a new entry of `id` alone is filed under it.
    fn get_or_file(&mut self, key: &str, id: NodeId) -> Option<&mut Entry> {
        self.get_or_file_hashed(hash_key(key), key, id)
    }

    // [`KeyMap::get_or_file`] of a key whose hash is `hash`.
    fn get_or_file_hashed(&mut self, hash: u64, key: &str, id: NodeId) -> Option<&mut Entry> {
        if let Some((_, at)) = self.find(hash, key) {
            return Some(&mut self.entries[at].entry);
        }
        if (self.entries.len() + 1) * 2 > self.slots.len() {
            self.reserve(self.entries.len() + 1);
        }
        let start = self.names.len();
        self.names.push_str(key);
        self.live += key.len();
        let at = self.entries.len();
        self.entries.push(Filed {
            hash,
            name: start..self.names.len(),
            entry: Entry {
                first: id,
                more: VecDeque::new(),
            },
        });
        self.place(at);
        None
    }

    // Put entry `at` in an empty slot of its group, or in the overflow when there is none.
    fn place(&mut self, at: usize) {
        let index: u32 = core::convert::TryFrom::try_from(at).expect("fewer keys than u32::MAX");
        let hash = self.entries[at].hash;
        if let Some(slot) = self.group(hash).find(|&slot| self.slots[slot].0 == EMPTY) {
            self.slots[slot] = (index, low_bits(hash));
        } else {
            let key = String::from(self.name(&self.entries[at]));
            self.overflow.insert(key, index);
            let group = self.group_index(hash);
            self.overflowed[group] = true;
        }
    }

    // Room for `count` keys, each group at most half full on average, with every key placed
    // again. Only ever called for more groups than there are.
    fn reserve(&mut self, count: usize) {
        let mut groups = (self.slots.len() / GROUP).max(1);
        while groups * GROUP < count * 2 {
            groups *= 2;
        }
        self.slots = vec![(EMPTY, 0); groups * GROUP];
        self.overflowed = vec![false; groups];
        self.overflow.clear();
        self.compact();
        for at in 0..self.entries.len() {
            self.place(at);
        }
    }

    // Keep only the names of the entries.
    fn compact(&mut self) {
        let mut names = String::with_capacity(self.live);
        for filed in &mut self.entries {
            let start = names.len();
            names.push_str(&self.names[filed.name.clone()]);
            filed.name = start..names.len();
        }
        self.names = names;
    }

    fn remove(&mut self, key: &str) {
        self.remove_hashed(hash_key(key), key);
    }

    // [`KeyMap::remove`] of a key whose hash is `hash`.
    fn remove_hashed(&mut self, hash: u64, key: &str) {
        let (place, at) = self.find(hash, key).expect("only a filed key is removed");
        match place {
            Place::Slot(slot) => {
                // The last filled slot of the group takes the place of the removed one, so the
                // filled slots stay first.
                let group = self.group(hash);
                let last = group
                    .clone()
                    .rev()
                    .find(|&other| self.slots[other].0 != EMPTY)
                    .expect("the removed slot is filled");
                self.slots[slot] = self.slots[last];
                self.slots[last].0 = EMPTY;
            }
            Place::Overflow => {
                self.overflow.remove(key);
            }
        }
        self.live -= key.len();
        let last = self.entries.len() - 1;
        if at != last {
            // The last entry takes the place of the removed one.
            let moved = &self.entries[last];
            let index: u32 =
                core::convert::TryFrom::try_from(at).expect("fewer keys than u32::MAX");
            match self.find(moved.hash, self.name(moved)) {
                Some((Place::Slot(slot), _)) => self.slots[slot].0 = index,
                Some((Place::Overflow, _)) => {
                    let name = &self.names[moved.name.clone()];
                    *self
                        .overflow
                        .get_mut(name)
                        .expect("an overflowing key is in the overflow") = index;
                }
                None => unreachable!("a filed key is found"),
            }
        }
        self.entries.swap_remove(at);
        if self.names.len() > 2 * self.live + 64 {
            self.compact();
        }
    }

    fn values_mut(&mut self) -> impl Iterator<Item = &mut Entry> {
        self.entries.iter_mut().map(|filed| &mut filed.entry)
    }
}

// The key of definition or footnote definition `id`, and whether it is a footnote definition.
fn indexed_key<'v>(
    source: &'v str,
    nodes: &'v [Node],
    id: NodeId,
    scratch: &'v mut String,
) -> (bool, &'v str) {
    defined_key(&nodes[id.index()].kind, source, scratch)
}

// The key that `kind`, a definition or footnote definition, defines, and whether it is a
// footnote definition.
fn defined_key<'v>(
    kind: &'v EventKind<'_>,
    source: &'v str,
    scratch: &'v mut String,
) -> (bool, &'v str) {
    match kind {
        EventKind::Definition { label, .. } => {
            (false, scratch_reference_key(text(label, source), scratch))
        }
        EventKind::Start(Tag::FootnoteDefinition { identifier, .. }) => {
            (true, text(identifier, source))
        }
        _ => unreachable!("only definitions are indexed"),
    }
}

// Whether a node of `kind` can hold a definition: only containers of blocks can.
pub(crate) fn holds_blocks(kind: &EventKind<'_>) -> bool {
    matches!(kind, EventKind::Start(tag)
        if matches!(tag.kind().content(), Content::Root | Content::Blocks | Content::Items))
}

// The next node after `id` in preorder under `root` that is not in a container that cannot hold
// a definition, such as a paragraph.
pub(crate) fn next_block(nodes: &[Node], root: NodeId, id: NodeId) -> Option<NodeId> {
    let node = &nodes[id.index()];
    if holds_blocks(&node.kind) {
        if let Some(child) = node.first_child() {
            return Some(child);
        }
    }
    next_after(nodes, root, id)
}

// The next node in preorder under `root` after `id` and what it holds.
pub(crate) fn next_after(nodes: &[Node], root: NodeId, id: NodeId) -> Option<NodeId> {
    let mut at = id;
    loop {
        if at == root {
            return None;
        }
        let node = &nodes[at.index()];
        if let Some(next) = node.next_sibling() {
            return Some(next);
        }
        at = node
            .parent()
            .expect("a descendant has a parent before its subtree root");
    }
}

// Each definition and footnote definition of the tree under `root`, in the order they end, which
// is document order but for a footnote definition, which comes after what it holds, other than
// the root. The first of a key in this order binds it, as the renderer shows the first footnote
// definition of an identifier to end, which is one nested in another before the other.
fn each_definition(nodes: &[Node], root: NodeId, mut visit: impl FnMut(NodeId)) {
    let footnote = |id: NodeId| {
        matches!(
            nodes[id.index()].kind,
            EventKind::Start(Tag::FootnoteDefinition { .. })
        )
    };
    // The root of a tree comes first, so that it is never a copy of what it holds.
    if footnote(root) {
        visit(root);
    }
    let mut current = Some(root);
    while let Some(id) = current {
        if matches!(nodes[id.index()].kind, EventKind::Definition { .. }) {
            visit(id);
        }
        // Each footnote definition this leaves, from the deepest, ends here.
        let next = next_block(nodes, root, id);
        let node = &nodes[id.index()];
        if !(holds_blocks(&node.kind) && node.first_child().is_some()) {
            let mut at = id;
            loop {
                if at != root && footnote(at) {
                    visit(at);
                }
                if at == root || nodes[at.index()].next_sibling().is_some() {
                    break;
                }
                at = nodes[at.index()]
                    .parent()
                    .expect("a descendant has a parent before its subtree root");
            }
        }
        current = next;
    }
}

// The definitions and footnote definitions of each key: of a tree, the first of each in
// document order, or of the attached document, all of them in no order.
#[derive(Clone, Debug, Default)]
struct Keys {
    definitions: KeyMap,
    footnotes: KeyMap,
}

impl Keys {
    // Add `id`, keeping all of a key's nodes when `all` is set; whether it is the first of its
    // key.
    fn insert(
        &mut self,
        source: &str,
        nodes: &[Node],
        id: NodeId,
        all: bool,
        scratch: &mut Scratch,
    ) -> bool {
        let (footnote, key) = indexed_key(source, nodes, id, &mut scratch.key);
        match self.map_mut(footnote).get_or_file(key, id) {
            Some(entry) => {
                if all {
                    entry.more.push_back(id);
                }
                false
            }
            None => true,
        }
    }

    fn map(&self, footnote: bool) -> &KeyMap {
        if footnote {
            &self.footnotes
        } else {
            &self.definitions
        }
    }

    fn map_mut(&mut self, footnote: bool) -> &mut KeyMap {
        if footnote {
            &mut self.footnotes
        } else {
            &mut self.definitions
        }
    }

    // The entry of definitions, or footnote definitions, of key `key`.
    fn entry(&self, footnote: bool, key: &str) -> Option<&Entry> {
        self.map(footnote).get(key)
    }

    // Remove `id` from the entry of its key, making the next node of the entry its first when
    // `id` was, and removing the entry once it is empty. Whether `id` was or left a copy.
    fn remove(&mut self, footnote: bool, key: &str, id: NodeId) -> bool {
        let map = self.map_mut(footnote);
        let entry = map.entry_mut(key);
        if entry.first == id {
            if let Some(next) = entry.more.pop_front() {
                entry.first = next;
                return true;
            }
            map.remove(key);
            return false;
        }
        let at = entry
            .more
            .iter()
            .position(|&other| other == id)
            .expect("an indexed node is in the entry of its key");
        entry.more.remove(at);
        true
    }
}

// The key a reference labelled `label` looks up, or a footnote call with `identifier`.
fn lookup_key<'v>(label: &'v str, footnote: bool, scratch: &'v mut String) -> &'v str {
    if footnote {
        label
    } else {
        scratch_reference_key(label, scratch)
    }
}

// The first definition of each reference key and footnote identifier of a tree, which owns
// nothing of the document, so a pass that edits the document can keep it.
#[derive(Clone, Debug, Default)]
pub(crate) struct Table {
    keys: Keys,
}

impl Table {
    // The table of the tree under `root`, which should be the root of a tree, attached or
    // detached, or a subtree whose own bindings are wanted.
    pub(crate) fn new(source: &str, nodes: &[Node], root: NodeId) -> Self {
        let mut keys = Keys::default();
        let mut scratch = Scratch::default();
        each_definition(nodes, root, |id| {
            keys.insert(source, nodes, id, false, &mut scratch);
        });
        Self { keys }
    }

    // The definitions and footnote definitions of the tree under `root` that are copies, each
    // after the first of its key there, in document order.
    pub(crate) fn copies(source: &str, nodes: &[Node], root: NodeId) -> Vec<NodeId> {
        // A tree of one definition has no copy, and is found so without filing it.
        let mut count = 0;
        each_definition(nodes, root, |_| count += 1);
        if count < 2 {
            return Vec::new();
        }
        let mut keys = Keys::default();
        let mut scratch = Scratch::default();
        let mut copies = Vec::new();
        each_definition(nodes, root, |id| {
            if !keys.insert(source, nodes, id, false, &mut scratch) {
                copies.push(id);
            }
        });
        copies
    }

    // The table of the tree under `root`, but for the definitions and footnote definitions for
    // which `skip` holds.
    pub(crate) fn without(
        source: &str,
        nodes: &[Node],
        root: NodeId,
        skip: impl Fn(NodeId) -> bool,
    ) -> Self {
        let mut keys = Keys::default();
        let mut scratch = Scratch::default();
        each_definition(nodes, root, |id| {
            if !skip(id) {
                keys.insert(source, nodes, id, false, &mut scratch);
            }
        });
        Self { keys }
    }

    // Whether a definition, or footnote definition, of `key` is in the table.
    pub(crate) fn binds(&self, footnote: bool, key: &str) -> bool {
        self.keys.entry(footnote, key).is_some()
    }

    // The definition, or footnote definition, that a label or identifier names.
    fn get(&self, label: &str, footnote: bool, scratch: &mut Scratch) -> Option<NodeId> {
        let key = lookup_key(label, footnote, &mut scratch.key);
        Some(self.keys.entry(footnote, key)?.first)
    }
}

// Every definition and footnote definition of the attached document, by key, which a document
// keeps as it is edited. A key's first definition in document order is its binding, and any
// others are copies of it, which only a parsed, built or loaded document has, in document order:
// an edit adds no copy, so removing the binding makes the next copy the binding without
// comparing where they are.
#[derive(Clone, Debug, Default)]
pub(crate) struct Index {
    keys: Keys,
    // How many copies there are.
    copies: usize,
}

// The keys a tree defines: whether each is of a footnote definition, and the key.
pub(crate) type DefinedKeys = Vec<(bool, String)>;

impl Index {
    // The index of definitions and footnote definitions `ids`, each attached, in document order.
    pub(crate) fn new(source: &str, nodes: &[Node], ids: &[NodeId]) -> Self {
        let mut index = Self::default();
        let footnotes = ids
            .iter()
            .filter(|id| {
                matches!(
                    nodes[id.index()].kind,
                    EventKind::Start(Tag::FootnoteDefinition { .. })
                )
            })
            .count();
        index.keys.footnotes.reserve(footnotes);
        index.keys.definitions.reserve(ids.len() - footnotes);
        let mut scratch = Scratch::default();
        for &id in ids {
            if !index.keys.insert(source, nodes, id, true, &mut scratch) {
                index.copies += 1;
            }
        }
        index
    }

    // The index of the attached document, whose root is `root`.
    #[cfg(feature = "serde")]
    pub(crate) fn of_tree(source: &str, nodes: &[Node], root: NodeId) -> Self {
        Self::new(source, nodes, &definitions(nodes, root))
    }

    // Whether any key has a copy.
    pub(crate) fn has_copies(&self) -> bool {
        self.copies > 0
    }

    // Remove the copies of every key, which are about to be removed from the document, and
    // give them, in no particular order, in time linear in the keys and copies.
    pub(crate) fn take_copies(&mut self) -> Vec<NodeId> {
        let mut copies = Vec::with_capacity(self.copies);
        for map in [&mut self.keys.definitions, &mut self.keys.footnotes] {
            for entry in map.values_mut() {
                copies.extend(entry.more.drain(..));
            }
        }
        self.copies = 0;
        copies
    }

    // Add the definitions of the subtree under `root`, which has just been attached, and whose
    // keys `admits` accepted.
    pub(crate) fn insert_tree(&mut self, source: &str, nodes: &[Node], root: NodeId) {
        let mut scratch = Scratch::default();
        each_definition(nodes, root, |id| {
            let first = self.keys.insert(source, nodes, id, true, &mut scratch);
            debug_assert!(first, "an edit adds no copy");
        });
    }

    // Add definition `id`, which has just been attached or changed its key to one that `admits`
    // accepted.
    pub(crate) fn insert(&mut self, source: &str, nodes: &[Node], id: NodeId) {
        let first = self
            .keys
            .insert(source, nodes, id, true, &mut Scratch::default());
        debug_assert!(first, "an edit adds no copy");
    }

    // Remove the definitions of the subtree under `root`, which is about to be detached.
    pub(crate) fn remove_tree(&mut self, source: &str, nodes: &[Node], root: NodeId) {
        let mut scratch = String::new();
        each_definition(nodes, root, |id| {
            self.remove_key(source, nodes, id, &mut scratch);
        });
    }

    // Remove definition `id`, which is about to be detached or change its key.
    pub(crate) fn remove(&mut self, source: &str, nodes: &[Node], id: NodeId) {
        self.remove_key(source, nodes, id, &mut String::new());
    }

    fn remove_key(&mut self, source: &str, nodes: &[Node], id: NodeId, scratch: &mut String) {
        let (footnote, key) = indexed_key(source, nodes, id, scratch);
        let copy = self.keys.remove(footnote, key, id);
        if copy {
            self.copies -= 1;
        }
    }

    // Whether each definition and footnote definition of the subtree under `root`, which is
    // attached, is the only one of its key, so that moving it within the document moves no
    // binding.
    pub(crate) fn movable(&self, source: &str, nodes: &[Node], root: NodeId) -> bool {
        let mut scratch = Scratch::default();
        let mut movable = true;
        each_definition(nodes, root, |id| {
            let (footnote, key) = indexed_key(source, nodes, id, &mut scratch.key);
            movable &= self
                .keys
                .entry(footnote, key)
                .map_or(true, |entry| entry.more.is_empty());
        });
        movable
    }

    // Whether the attached document can take definitions of `keys`, each of which a tree
    // defines once, when the definitions for which `leaving` holds leave it: whether no other
    // definition of the document has any of them, as an edit adds no copy.
    pub(crate) fn admits(&self, keys: &[(bool, String)], leaving: impl Fn(NodeId) -> bool) -> bool {
        keys.iter()
            .all(|(footnote, key)| !self.binds(*footnote, key, &leaving))
    }

    // Whether the attached document has a definition, or footnote definition, of `key` other
    // than those for which `leaving` holds, in time linear in those of `key` it passes first.
    pub(crate) fn binds(
        &self,
        footnote: bool,
        key: &str,
        leaving: impl Fn(NodeId) -> bool,
    ) -> bool {
        self.keys
            .entry(footnote, key)
            .is_some_and(|entry| !entry.nodes().all(leaving))
    }

    // The binding of a label or identifier: the first of its definitions.
    fn get(&self, label: &str, footnote: bool, scratch: &mut Scratch) -> Option<NodeId> {
        let key = lookup_key(label, footnote, &mut scratch.key);
        Some(self.keys.entry(footnote, key)?.first)
    }
}

// The definitions and footnote definitions of the tree under `root`, in document order.
pub(crate) fn definitions(nodes: &[Node], root: NodeId) -> Vec<NodeId> {
    let mut ids = Vec::new();
    each_definition(nodes, root, |id| ids.push(id));
    ids
}

// The keys of the definitions and footnote definitions of the tree under `root`, or `None` when
// two of them share one.
pub(crate) fn tree_keys(source: &str, nodes: &[Node], root: NodeId) -> Option<DefinedKeys> {
    let mut keys = Keys::default();
    let mut scratch = Scratch::default();
    let mut defined = Vec::new();
    let mut single = true;
    each_definition(nodes, root, |id| {
        if keys.insert(source, nodes, id, false, &mut scratch) {
            let (footnote, key) = indexed_key(source, nodes, id, &mut scratch.key);
            defined.push((footnote, String::from(key)));
        } else {
            single = false;
        }
    });
    single.then_some(defined)
}

// The key that `kind` defines, if it is a definition or footnote definition.
pub(crate) fn kind_key(kind: &EventKind<'_>, source: &str) -> (bool, String) {
    let mut scratch = String::new();
    let (footnote, key) = defined_key(kind, source, &mut scratch);
    (footnote, String::from(key))
}

// The document a lookup reads: its source, nodes, the index of its attached definitions and its
// root.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Context<'d> {
    pub(crate) source: &'d str,
    pub(crate) nodes: &'d [Node],
    pub(crate) index: &'d Index,
    pub(crate) root: NodeId,
}

// Where the references of one tree look their bindings up, which owns nothing of the document.
#[derive(Clone, Debug)]
pub(crate) enum Lookup {
    // The attached document, through its index.
    Attached,
    // A detached tree, or a subtree, through a table of it.
    Tree(Box<Table>),
}

impl Lookup {
    // The lookup of the tree whose root is `tree`.
    pub(crate) fn new(cx: Context<'_>, tree: NodeId) -> Self {
        if tree == cx.root {
            Self::Attached
        } else {
            Self::Tree(Box::new(Table::new(cx.source, cx.nodes, tree)))
        }
    }

    fn get(
        &mut self,
        cx: Context<'_>,
        label: &str,
        footnote: bool,
        scratch: &mut Scratch,
    ) -> Option<NodeId> {
        match self {
            Self::Tree(table) => table.get(label, footnote, scratch),
            Self::Attached => cx.index.get(label, footnote, scratch),
        }
    }

    // The definition a reference labelled `label` leads to.
    pub(crate) fn definition(
        &mut self,
        cx: Context<'_>,
        label: &str,
        scratch: &mut Scratch,
    ) -> Option<NodeId> {
        self.get(cx, label, false, scratch)
    }

    // The footnote definition a call with identifier `identifier` calls.
    pub(crate) fn footnote_definition(
        &mut self,
        cx: Context<'_>,
        identifier: &str,
        scratch: &mut Scratch,
    ) -> Option<NodeId> {
        self.get(cx, identifier, true, scratch)
    }

    // The definition the reference `kind`, a link or image, leads to.
    pub(crate) fn definition_of(
        &mut self,
        cx: Context<'_>,
        kind: &EventKind<'_>,
        scratch: &mut Scratch,
    ) -> Option<NodeId> {
        let EventKind::Start(Tag::Link(target) | Tag::Image(target)) = kind else {
            unreachable!("only a reference is bound")
        };
        let reference = target.reference().expect("only a reference is bound");
        self.definition(cx, text(&reference.label, cx.source), scratch)
    }

    // Whether `kind` names nothing: an unbound reference or footnote call.
    pub(crate) fn unbound(
        &mut self,
        cx: Context<'_>,
        kind: &EventKind<'_>,
        scratch: &mut Scratch,
    ) -> bool {
        match kind {
            EventKind::Start(
                Tag::Link(Target::Reference(reference)) | Tag::Image(Target::Reference(reference)),
            ) => self
                .definition(cx, text(&reference.label, cx.source), scratch)
                .is_none(),
            EventKind::FootnoteReference { identifier, .. } => self
                .footnote_definition(cx, text(identifier, cx.source), scratch)
                .is_none(),
            _ => unreachable!("only a reference or footnote call names a definition"),
        }
    }
}

// The destination and title of definition `id`, which a lookup found.
pub(crate) fn definition_target(
    nodes: &[Node],
    id: NodeId,
) -> (&Text<'static>, Option<&Text<'static>>) {
    let EventKind::Definition {
        destination, title, ..
    } = &nodes[id.index()].kind
    else {
        unreachable!("a binding is a definition")
    };
    (destination, title.as_ref())
}

// The destination and title that the link or image `kind` leads to: its own for a resource,
// and for a reference those of its definition, as `definition` finds it.
pub(crate) fn destination<'d>(
    source: &'d str,
    nodes: &'d [Node],
    kind: &'d EventKind<'static>,
    definition: impl FnOnce(&str) -> Option<NodeId>,
) -> Option<(&'d str, Option<&'d str>)> {
    let EventKind::Start(Tag::Link(target) | Tag::Image(target)) = kind else {
        return None;
    };
    let (destination, title) = match target {
        Target::Reference(reference) => {
            definition_target(nodes, definition(text(&reference.label, source))?)
        }
        target => target
            .destination()
            .expect("a document stores a reference by name or a resource"),
    };
    Some((
        text(destination, source),
        title.map(|title| text(title, source)),
    ))
}

impl<'d> Bindings<'d> {
    // The bindings of the attached document, whose root is `root`.
    pub(crate) fn new(source: &'d str, nodes: &'d [Node], root: NodeId) -> Self {
        let mut attached = vec![false; nodes.len()];
        let mut current = Some(root);
        while let Some(id) = current {
            attached[id.index()] = true;
            current = next_descendant(nodes, root, id);
        }
        Self {
            source,
            nodes,
            table: Table::new(source, nodes, root),
            attached,
        }
    }

    /// The link reference definition that a reference labelled `label` leads to, if any.
    pub fn definition(&self, label: &str) -> Option<NodeId> {
        self.table.get(label, false, &mut Scratch::default())
    }

    /// The footnote definition that a footnote call with identifier `identifier` calls, if any.
    /// The identifier is the normalized one the call stores.
    pub fn footnote_definition(&self, identifier: &str) -> Option<NodeId> {
        self.table.get(identifier, true, &mut Scratch::default())
    }

    /// The destination and title that the link or image `id` leads to: its own for a resource,
    /// and its definition's for a bound reference. `None` for an unbound reference, a node that
    /// is not a link or image, or one that is not in the attached document.
    pub fn destination(&self, id: NodeId) -> Option<(&'d str, Option<&'d str>)> {
        if !self.attached.get(id.index()).copied()? {
            return None;
        }
        destination(
            self.source,
            self.nodes,
            &self.nodes[id.index()].kind,
            |label| self.definition(label),
        )
    }

    // The definition the reference `target` names.
    pub(crate) fn definition_of(&self, target: &Target<'_>) -> Option<NodeId> {
        self.definition(text(&target.reference()?.label, self.source))
    }

    // Whether the reference or footnote call `kind` names nothing in the attached document.
    pub(crate) fn unbound(&self, kind: &EventKind<'_>) -> bool {
        match kind {
            EventKind::Start(
                Tag::Link(Target::Reference(reference)) | Tag::Image(Target::Reference(reference)),
            ) => self
                .definition(text(&reference.label, self.source))
                .is_none(),
            EventKind::FootnoteReference { identifier, .. } => self
                .footnote_definition(text(identifier, self.source))
                .is_none(),
            _ => false,
        }
    }
}

// What an unbound reference reads back as around its text, as text: its opening bracket, and
// what follows its text, with its label for a full reference.
pub(crate) fn unbound_text(
    kind: ReferenceKind,
    label: &str,
    image: bool,
) -> (&'static str, String) {
    let open = if image { "![" } else { "[" };
    let close = match kind {
        ReferenceKind::Shortcut => String::from("]"),
        ReferenceKind::Collapsed => String::from("][]"),
        ReferenceKind::Full => {
            let mut close = String::from("][");
            push_label(&mut close, label);
            close.push(']');
            close
        }
    };
    (open, close)
}

// What an unbound footnote call reads back as, as text.
pub(crate) fn unbound_footnote_text(label: &str) -> String {
    let mut text = String::from("[^");
    push_label(&mut text, label);
    text.push(']');
    text
}

// Push a label as the text it reads back as: with its character escapes and references decoded,
// and each run of whitespace that holds a line ending as one space, which renders the same and
// can be written in any text.
fn push_label(output: &mut String, label: &str) {
    let decoded = crate::value_builder::decode_string_with_options(
        label,
        0..label.len(),
        &crate::ParseOptions::default(),
    );
    let decoded = text(&decoded, label);
    let mut pending = String::new();
    let mut broken = false;
    for character in decoded.chars() {
        if matches!(character, ' ' | '\t' | '\r' | '\n') {
            broken |= matches!(character, '\r' | '\n');
            pending.push(character);
            continue;
        }
        output.push_str(if broken { " " } else { &pending });
        pending.clear();
        broken = false;
        output.push(character);
    }
    output.push_str(if broken { " " } else { &pending });
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::{hash_key, KeyMap, NodeId, GROUP};
    use alloc::{format, string::String, vec::Vec};

    // Filing `count` keys, each with the hash `hash` gives it, and finding each.
    fn file_and_find(count: usize, hash: impl Fn(&str) -> u64) -> KeyMap {
        let mut map = KeyMap::default();
        let keys: Vec<String> = (0..count).map(|index| format!("k{index}")).collect();
        for (index, key) in keys.iter().enumerate() {
            assert!(map
                .get_or_file_hashed(hash(key), key, NodeId::new(index))
                .is_none());
        }
        for (index, key) in keys.iter().enumerate() {
            let entry = map.get_or_file_hashed(hash(key), key, NodeId::new(0));
            assert_eq!(entry.map(|entry| entry.first), Some(NodeId::new(index)));
        }
        map
    }

    // Keys of one hash fill their group and go to the overflow, ordered by key, where each is
    // found, and removing keys in any order keeps the others found.
    #[test]
    fn keys_of_one_hash_are_found_in_the_overflow() {
        let mut map = file_and_find(200, |_| 7);
        assert_eq!(map.overflow.len(), 200 - GROUP);
        for index in (0..200).filter(|index| index % 3 != 1) {
            let key = format!("k{index}");
            map.remove_hashed(7, &key);
            assert!(map.find(7, &key).is_none());
        }
        for index in (0..200).filter(|index| index % 3 == 1) {
            let (_, at) = map.find(7, &format!("k{index}")).unwrap();
            assert_eq!(map.entries[at].entry.first, NodeId::new(index));
        }
    }

    // Making more groups places every key again, so one that overflowed and now fits in a slot
    // leaves the overflow: removed, it is not found there once its group overflows again.
    #[test]
    fn keys_placed_again_leave_the_overflow() {
        // Every key of four groups is in the first, and of eight in the first or second, by
        // whether its number is odd.
        let hash = |key: &str| {
            let index: u64 = key[1..].parse().unwrap();
            ((index & 1) << 61) | index
        };
        let mut map = file_and_find(16, hash);
        assert_eq!(map.overflowed.len(), 4);
        assert_eq!(map.overflow.len(), 16 - GROUP);
        map.reserve(32);
        assert_eq!(map.overflowed.len(), 8);
        assert!(map.overflow.is_empty());
        map.remove_hashed(hash("k8"), "k8");
        for index in [16, 18] {
            let key = format!("k{index}");
            assert!(map
                .get_or_file_hashed(hash(&key), &key, NodeId::new(index))
                .is_none());
        }
        assert!(map.overflowed[0]);
        assert!(map.find(hash("k8"), "k8").is_none());
        assert!(map.find(hash("k18"), "k18").is_some());
    }

    // The names of removed keys are dropped once they are most of the names, so filing and
    // removing keys again and again keeps the names as long as a few times the keys left.
    #[test]
    fn removed_names_are_compacted() {
        let mut map = KeyMap::default();
        assert!(map.get_or_file("kept", NodeId::new(0)).is_none());
        for index in 0..10_000 {
            let key = format!("k{index}");
            assert!(map.get_or_file(&key, NodeId::new(index)).is_none());
            map.remove(&key);
            assert!(map.names.len() <= 2 * map.live + 64 + key.len());
        }
        assert_eq!(
            map.get("kept").map(|entry| entry.first),
            Some(NodeId::new(0))
        );
    }

    // Keys of one hash take time logarithmic in them each to file and find, as the overflow is
    // ordered: 8 times as many grow the time at most a few times as much more than keys that
    // spread over the groups do.
    #[test]
    fn keys_of_one_hash_take_logarithmic_time() {
        let time = |count: usize, hash: &dyn Fn(&str) -> u64| {
            (0..5)
                .map(|_| {
                    let start = std::time::Instant::now();
                    drop(file_and_find(count, hash));
                    start.elapsed().as_secs_f64()
                })
                .fold(f64::MAX, f64::min)
        };
        let ratio = || {
            let one = |_: &str| 7;
            let spread = |key: &str| hash_key(key);
            (time(1 << 12, &one) / time(1 << 9, &one))
                / (time(1 << 12, &spread) / time(1 << 9, &spread))
        };
        assert!((0..3).any(|_| ratio() < 2.5));
    }
}
