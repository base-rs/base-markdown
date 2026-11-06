// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! The bindings of a document's references: which definition each label names.
use crate::definitions::{hash_key, hashed_reference_key};
use crate::document::{next_descendant, Node, NodeId};
use crate::stream::{Content, EventKind, Tag, Target, Text};
use crate::ReferenceKind;
use alloc::{string::String, vec, vec::Vec};

/// The definition each label of the attached document names, built in one pass in document
/// order.
///
/// A reference is a name and a definition a binding of it: the first link reference definition
/// in document order whose label matches a reference's, as the parser matches labels, is the one
/// it leads to, and the first footnote definition with a footnote call's identifier is the one
/// it calls. A document stores no binding, so the next table built sees every edit to its
/// definitions. A `Bindings` borrows the document, so it cannot outlive an edit.
///
/// [`Document::bindings`][crate::Document::bindings] builds the table of the attached document,
/// in time linear in its size; each lookup in it then takes constant expected time. It covers
/// the attached document only: a detached subtree is a tree of its own, whose references its own
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

// Room for the key looked up, and for the key of a definition it is checked against, when
// either is not the label itself.
#[derive(Clone, Debug, Default)]
pub(crate) struct Scratch {
    key: String,
    other: String,
}

// The nodes of one key, in no order: the first inserted, and any others.
#[derive(Clone, Debug)]
struct Entry {
    hash: u64,
    first: NodeId,
    more: Vec<NodeId>,
}

impl Entry {
    fn contains(&self, id: NodeId) -> bool {
        self.first == id || self.more.contains(&id)
    }
}

// The nodes of each key, in an open-addressed hash table with linear probing. An entry keeps
// only the hash of its key, and a lookup checks the key of an entry's first node, so the table
// borrows nothing and allocates nothing for a key.
#[derive(Clone, Debug, Default)]
struct KeyMap {
    // A power of two of slots, or none.
    slots: Vec<Option<Entry>>,
    len: usize,
}

impl KeyMap {
    fn mask(&self) -> usize {
        self.slots.len().wrapping_sub(1)
    }

    #[allow(clippy::cast_possible_truncation)]
    fn home(&self, hash: u64) -> usize {
        hash as usize & self.mask()
    }

    // The slot of the entry of hash `hash` whose first node `matches`, if any.
    fn find(&self, hash: u64, mut matches: impl FnMut(&Entry) -> bool) -> Option<usize> {
        if self.slots.is_empty() {
            return None;
        }
        let mut slot = self.home(hash);
        loop {
            match &self.slots[slot] {
                None => return None,
                Some(entry) if entry.hash == hash && matches(entry) => return Some(slot),
                Some(_) => slot = (slot + 1) & self.mask(),
            }
        }
    }

    // The slot and entry of hash `hash` whose first node `matches`, if any.
    fn get(&self, hash: u64, matches: impl FnMut(&Entry) -> bool) -> Option<(usize, &Entry)> {
        let slot = self.find(hash, matches)?;
        Some((
            slot,
            self.slots[slot]
                .as_ref()
                .expect("a found slot holds an entry"),
        ))
    }

    // Add `id` to the entry of its key, which `matches` recognizes by its first node, keeping
    // only the first node of a key unless `all` is set.
    fn insert(&mut self, hash: u64, id: NodeId, all: bool, matches: impl FnMut(&Entry) -> bool) {
        if let Some(slot) = self.find(hash, matches) {
            if all {
                self.slots[slot]
                    .as_mut()
                    .expect("a found slot holds an entry")
                    .more
                    .push(id);
            }
            return;
        }
        if (self.len + 1) * 2 > self.slots.len() {
            let size = (self.slots.len() * 2).max(8);
            let old = core::mem::take(&mut self.slots);
            self.slots.resize_with(size, || None);
            for entry in old.into_iter().flatten() {
                self.place(entry);
            }
        }
        self.place(Entry {
            hash,
            first: id,
            more: Vec::new(),
        });
        self.len += 1;
    }

    // Put `entry`, of a key no entry has, in the first empty slot from its home.
    fn place(&mut self, entry: Entry) {
        let mut slot = self.home(entry.hash);
        while self.slots[slot].is_some() {
            slot = (slot + 1) & self.mask();
        }
        self.slots[slot] = Some(entry);
    }

    // Remove `id` from the entry of hash `hash` that holds it, and the entry once it is empty,
    // moving back the entries after it that it displaced.
    fn remove(&mut self, hash: u64, id: NodeId) {
        let mut slot = self
            .find(hash, |entry| entry.contains(id))
            .expect("an indexed node is in the entry of its key");
        let entry = self.slots[slot]
            .as_mut()
            .expect("a found slot holds an entry");
        if entry.first != id {
            let at = entry
                .more
                .iter()
                .position(|&other| other == id)
                .expect("the entry holds the node");
            entry.more.swap_remove(at);
            return;
        }
        if let Some(next) = entry.more.pop() {
            entry.first = next;
            return;
        }
        self.slots[slot] = None;
        self.len -= 1;
        let mut next = (slot + 1) & self.mask();
        while let Some(entry) = &self.slots[next] {
            let home = self.home(entry.hash);
            // The entry stays when its home is cyclically after the emptied slot, up to it.
            let stays = if slot <= next {
                slot < home && home <= next
            } else {
                slot < home || home <= next
            };
            if !stays {
                self.slots[slot] = self.slots[next].take();
                slot = next;
            }
            next = (next + 1) & self.mask();
        }
    }
}

// The hashed key of definition or footnote definition `id`, and whether it is a footnote
// definition.
fn indexed_key<'v>(
    source: &'v str,
    nodes: &'v [Node],
    id: NodeId,
    scratch: &'v mut String,
) -> (bool, u64, &'v str) {
    match &nodes[id.index()].kind {
        EventKind::Definition { label, .. } => {
            let (hash, key) = hashed_reference_key(text(label, source), scratch);
            (false, hash, key)
        }
        EventKind::Start(Tag::FootnoteDefinition { identifier, .. }) => {
            let identifier = text(identifier, source);
            (true, hash_key(identifier), identifier)
        }
        _ => unreachable!("only definitions are indexed"),
    }
}

// The label of definition `id` as it is written, or the identifier of a footnote definition.
fn written_label<'v>(source: &'v str, nodes: &'v [Node], id: NodeId) -> &'v str {
    match &nodes[id.index()].kind {
        EventKind::Definition { label, .. } => text(label, source),
        EventKind::Start(Tag::FootnoteDefinition { identifier, .. }) => text(identifier, source),
        _ => unreachable!("only definitions are indexed"),
    }
}

// Whether the first node of `entry` has the key `key` of label `label`: at once when it is
// written the same, as it usually is, and otherwise by its own key.
fn has_key(
    source: &str,
    nodes: &[Node],
    entry: &Entry,
    (label, key): (&str, &str),
    scratch: &mut String,
) -> bool {
    written_label(source, nodes, entry.first) == label
        || indexed_key(source, nodes, entry.first, scratch).2 == key
}

// Whether a node of `kind` can hold a definition: only containers of blocks can.
fn holds_blocks(kind: &EventKind<'_>) -> bool {
    matches!(kind, EventKind::Start(tag)
        if matches!(tag.kind().content(), Content::Root | Content::Blocks | Content::Items))
}

// The next node after `id` in preorder under `root` that is not in a container that cannot hold
// a definition, such as a paragraph.
fn next_block(nodes: &[Node], root: NodeId, id: NodeId) -> Option<NodeId> {
    let node = &nodes[id.index()];
    if holds_blocks(&node.kind) {
        if let Some(child) = node.first_child() {
            return Some(child);
        }
    }
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

// Each definition and footnote definition of the tree under `root`, in document order.
fn each_definition(nodes: &[Node], root: NodeId, mut visit: impl FnMut(NodeId)) {
    let mut current = Some(root);
    while let Some(id) = current {
        if matches!(
            nodes[id.index()].kind,
            EventKind::Definition { .. } | EventKind::Start(Tag::FootnoteDefinition { .. })
        ) {
            visit(id);
        }
        current = next_block(nodes, root, id);
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
    fn insert(
        &mut self,
        source: &str,
        nodes: &[Node],
        id: NodeId,
        all: bool,
        scratch: &mut Scratch,
    ) {
        let (footnote, hash, key) = indexed_key(source, nodes, id, &mut scratch.key);
        let map = if footnote {
            &mut self.footnotes
        } else {
            &mut self.definitions
        };
        let other = &mut scratch.other;
        let label = written_label(source, nodes, id);
        map.insert(hash, id, all, |entry| {
            has_key(source, nodes, entry, (label, key), other)
        });
    }

    fn map(&self, footnote: bool) -> &KeyMap {
        if footnote {
            &self.footnotes
        } else {
            &self.definitions
        }
    }

    // The entry of definitions, or footnote definitions, of the label or identifier `label`,
    // whose key is `key` of hash `hash`.
    fn entry(
        &self,
        source: &str,
        nodes: &[Node],
        footnote: bool,
        label: &str,
        (hash, key): (u64, &str),
        other: &mut String,
    ) -> Option<(usize, &Entry)> {
        self.map(footnote).get(hash, |entry| {
            has_key(source, nodes, entry, (label, key), other)
        })
    }
}

// The hashed key a reference labelled `label` looks up, or a footnote call with `identifier`.
fn lookup_key<'v>(label: &'v str, footnote: bool, scratch: &'v mut String) -> (u64, &'v str) {
    if footnote {
        (hash_key(label), label)
    } else {
        hashed_reference_key(label, scratch)
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

    // The definition, or footnote definition, that a label or identifier names.
    fn get(
        &self,
        source: &str,
        nodes: &[Node],
        label: &str,
        footnote: bool,
        scratch: &mut Scratch,
    ) -> Option<NodeId> {
        let key = lookup_key(label, footnote, &mut scratch.key);
        let (_, entry) =
            self.keys
                .entry(source, nodes, footnote, label, key, &mut scratch.other)?;
        Some(entry.first)
    }
}

// Every definition and footnote definition of the attached document, by key, which a document
// keeps as it is edited. A key's one definition is its binding; of several, the first in
// document order is, found among them alone when asked.
#[derive(Clone, Debug, Default)]
pub(crate) struct Index {
    keys: Keys,
}

impl Index {
    // The index of definitions and footnote definitions `ids`, each attached.
    pub(crate) fn new(source: &str, nodes: &[Node], ids: impl IntoIterator<Item = NodeId>) -> Self {
        let mut index = Self::default();
        let mut scratch = Scratch::default();
        for id in ids {
            index.keys.insert(source, nodes, id, true, &mut scratch);
        }
        index
    }

    // The index of the attached document, whose root is `root`.
    #[cfg(feature = "serde")]
    pub(crate) fn of_tree(source: &str, nodes: &[Node], root: NodeId) -> Self {
        let mut ids = Vec::new();
        each_definition(nodes, root, |id| ids.push(id));
        Self::new(source, nodes, ids)
    }

    // Add the definitions of the subtree under `root`, which has just been attached.
    pub(crate) fn insert_tree(&mut self, source: &str, nodes: &[Node], root: NodeId) {
        let mut scratch = Scratch::default();
        each_definition(nodes, root, |id| {
            self.keys.insert(source, nodes, id, true, &mut scratch);
        });
    }

    // Add definition `id`, which has just been attached or changed its key.
    pub(crate) fn insert(&mut self, source: &str, nodes: &[Node], id: NodeId) {
        self.keys
            .insert(source, nodes, id, true, &mut Scratch::default());
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
        let (footnote, hash, _) = indexed_key(source, nodes, id, scratch);
        if footnote {
            self.keys.footnotes.remove(hash, id);
        } else {
            self.keys.definitions.remove(hash, id);
        }
    }

    // The binding of a label or identifier: its one definition, or the first in document order
    // of its several, found among them alone, and kept in `firsts`, if given, by its slot.
    fn get(
        &self,
        source: &str,
        nodes: &[Node],
        label: &str,
        footnote: bool,
        scratch: &mut Scratch,
        firsts: Option<&mut Firsts>,
    ) -> Option<NodeId> {
        let key = lookup_key(label, footnote, &mut scratch.key);
        let (slot, entry) =
            self.keys
                .entry(source, nodes, footnote, label, key, &mut scratch.other)?;
        if entry.more.is_empty() {
            return Some(entry.first);
        }
        let first = || {
            entry.more.iter().fold(entry.first, |first, &id| {
                if precedes(nodes, id, first) {
                    id
                } else {
                    first
                }
            })
        };
        let Some(firsts) = firsts else {
            return Some(first());
        };
        let firsts = &mut firsts[usize::from(footnote)];
        if firsts.is_empty() {
            firsts.resize(self.keys.map(footnote).slots.len(), None);
        }
        Some(*firsts[slot].get_or_insert_with(first))
    }
}

// The first definition of each key of several, by its slot in the index, of reference
// definitions and of footnote definitions, as a pass finds them.
type Firsts = [Vec<Option<NodeId>>; 2];

// Whether `a` comes before `b` in document order, where both are distinct and in one tree: in
// time linear in their depth and in the siblings between their ancestors.
fn precedes(nodes: &[Node], a: NodeId, b: NodeId) -> bool {
    let depth = |mut id: NodeId| {
        let mut depth = 0;
        while let Some(parent) = nodes[id.index()].parent() {
            depth += 1;
            id = parent;
        }
        depth
    };
    let up = |id: NodeId| nodes[id.index()].parent().expect("both are in one tree");
    let (mut a, mut b) = (a, b);
    let (mut depth_a, mut depth_b) = (depth(a), depth(b));
    // An ancestor comes before what it holds.
    while depth_a > depth_b {
        a = up(a);
        depth_a -= 1;
        if a == b {
            return false;
        }
    }
    while depth_b > depth_a {
        b = up(b);
        depth_b -= 1;
        if a == b {
            return true;
        }
    }
    while up(a) != up(b) {
        a = up(a);
        b = up(b);
    }
    // Walk on from both at once, so that this takes time linear in the siblings between them
    // whichever comes first: one meets the other, or reaches the end of its siblings first.
    let (mut from_a, mut from_b) = (Some(a), Some(b));
    loop {
        from_a = from_a.and_then(|id| nodes[id.index()].next_sibling());
        from_b = from_b.and_then(|id| nodes[id.index()].next_sibling());
        match (from_a, from_b) {
            (Some(id), _) if id == b => return true,
            (_, Some(id)) if id == a => return false,
            (None, _) => return false,
            (_, None) => return true,
            _ => {}
        }
    }
}

// The document a lookup reads: its source, nodes, the index of its attached definitions and its
// root.
#[derive(Clone, Copy)]
pub(crate) struct Context<'d> {
    pub(crate) source: &'d str,
    pub(crate) nodes: &'d [Node],
    pub(crate) index: &'d Index,
    pub(crate) root: NodeId,
}

// Where the references of one tree look their bindings up, which owns nothing of the document.
#[derive(Clone, Debug)]
pub(crate) enum Lookup {
    // The attached document, through its index. A pass over the whole document keeps the
    // binding of each key of several definitions it finds, to find it once; another finds it
    // among them each time.
    Attached(Option<Firsts>),
    // A detached tree, or a subtree, through a table of it.
    Tree(Table),
}

impl Lookup {
    // The lookup of the tree whose root is `tree`, for a pass over all of it when `whole` is set.
    pub(crate) fn new(cx: Context<'_>, tree: NodeId, whole: bool) -> Self {
        if tree == cx.root {
            Self::Attached(whole.then(Firsts::default))
        } else {
            Self::Tree(Table::new(cx.source, cx.nodes, tree))
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
            Self::Tree(table) => table.get(cx.source, cx.nodes, label, footnote, scratch),
            Self::Attached(firsts) => cx.index.get(
                cx.source,
                cx.nodes,
                label,
                footnote,
                scratch,
                firsts.as_mut(),
            ),
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
        self.table.get(
            self.source,
            self.nodes,
            label,
            false,
            &mut Scratch::default(),
        )
    }

    /// The footnote definition that a footnote call with identifier `identifier` calls, if any.
    /// The identifier is the normalized one the call stores.
    pub fn footnote_definition(&self, identifier: &str) -> Option<NodeId> {
        self.table.get(
            self.source,
            self.nodes,
            identifier,
            true,
            &mut Scratch::default(),
        )
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
