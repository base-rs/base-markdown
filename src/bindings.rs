// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! The bindings of a document's references: which definition each label names.
use crate::definitions::reference_key_in;
use crate::document::{next_descendant, Node, NodeId};
use crate::stream::{Content, EventKind, Tag, Target, Text};
use crate::ReferenceKind;
use alloc::{borrow::Cow, string::String, vec, vec::Vec};

/// The definition each label of the attached document names, built in one pass in document
/// order.
///
/// A reference is a name and a definition a binding of it: the first link reference definition
/// in document order whose label matches a reference's, as the parser matches labels, is the one
/// it leads to, and the first footnote definition with a footnote call's identifier is the one
/// it calls. A document stores no binding, so the next table built sees every edit to its
/// definitions. The table borrows the document, so it cannot outlive an edit.
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
    table: Table<'d>,
    // Whether each node is in the attached document.
    attached: Vec<bool>,
}

fn text<'d>(value: &'d Text<'_>, source: &'d str) -> &'d str {
    value.resolve(source).expect("document text is validated")
}

// FNV-1a, which is quick for the short keys that labels are.
fn hash(key: &str) -> u64 {
    key.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

// The first node of each key, in an open-addressed hash index of the keys in document order.
#[derive(Clone, Debug, Default)]
struct Index<'d> {
    entries: Vec<(Cow<'d, str>, NodeId)>,
    // A power of two of slots, each empty, as 0, or one more than the index of its entry.
    slots: Vec<usize>,
}

impl<'d> Index<'d> {
    // Index `entries`, in document order, keeping the first of each key.
    fn new(mut entries: Vec<(Cow<'d, str>, NodeId)>) -> Self {
        if entries.is_empty() {
            return Self::default();
        }
        let mask = (entries.len() * 2).next_power_of_two() - 1;
        let mut slots = vec![0; mask + 1];
        let mut kept = 0;
        for at in 0..entries.len() {
            #[allow(clippy::cast_possible_truncation)]
            let mut slot = hash(&entries[at].0) as usize & mask;
            loop {
                match slots[slot] {
                    0 => {
                        entries.swap(kept, at);
                        kept += 1;
                        slots[slot] = kept;
                        break;
                    }
                    entry if entries[entry - 1].0 == entries[at].0 => break,
                    _ => slot = (slot + 1) & mask,
                }
            }
        }
        entries.truncate(kept);
        Self { entries, slots }
    }

    fn get(&self, key: &str) -> Option<NodeId> {
        let mask = self.slots.len().checked_sub(1)?;
        #[allow(clippy::cast_possible_truncation)]
        let mut slot = hash(key) as usize & mask;
        loop {
            match self.slots[slot] {
                0 => return None,
                entry if self.entries[entry - 1].0 == key => {
                    return Some(self.entries[entry - 1].1)
                }
                _ => slot = (slot + 1) & mask,
            }
        }
    }

    fn into_owned(self) -> Index<'static> {
        Index {
            entries: self
                .entries
                .into_iter()
                .map(|(key, id)| (Cow::Owned(key.into_owned()), id))
                .collect(),
            slots: self.slots,
        }
    }
}

// The first definition of each reference key and footnote identifier of a tree, which borrows
// the document. A pass that edits the document collects what it edits first, or owns the table.
#[derive(Clone, Debug, Default)]
pub(crate) struct Table<'d> {
    definitions: Index<'d>,
    footnotes: Index<'d>,
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

impl<'d> Table<'d> {
    // The table of the tree under `root`, which should be the root of a tree, attached or
    // detached, with its footnote definitions when `footnotes` is set.
    pub(crate) fn new(source: &'d str, nodes: &'d [Node], root: NodeId, footnotes: bool) -> Self {
        let mut definitions = Vec::new();
        let mut calls = Vec::new();
        let mut scratch = String::new();
        let mut current = Some(root);
        while let Some(id) = current {
            match &nodes[id.index()].kind {
                EventKind::Definition { label, .. } => {
                    let label = text(label, source);
                    let key = match reference_key_in(label, &mut scratch) {
                        Ok(key) => Cow::Borrowed(key),
                        Err(key) => Cow::Owned(String::from(key)),
                    };
                    definitions.push((key, id));
                }
                EventKind::Start(Tag::FootnoteDefinition { identifier, .. }) if footnotes => {
                    calls.push((Cow::Borrowed(text(identifier, source)), id));
                }
                _ => {}
            }
            current = next_block(nodes, root, id);
        }
        Self {
            definitions: Index::new(definitions),
            footnotes: Index::new(calls),
        }
    }

    // This table with keys of its own, borrowing nothing.
    pub(crate) fn into_owned(self) -> Table<'static> {
        Table {
            definitions: self.definitions.into_owned(),
            footnotes: self.footnotes.into_owned(),
        }
    }

    // The definition a reference labelled `label` leads to, with `scratch` for a key that is not
    // the label itself.
    pub(crate) fn definition(&self, label: &str, scratch: &mut String) -> Option<NodeId> {
        match reference_key_in(label, scratch) {
            Ok(key) | Err(key) => self.definitions.get(key),
        }
    }

    // The footnote definition a call with identifier `identifier` calls.
    pub(crate) fn footnote_definition(&self, identifier: &str) -> Option<NodeId> {
        self.footnotes.get(identifier)
    }

    // The definition the reference `kind`, a link or image, leads to.
    pub(crate) fn definition_of(
        &self,
        kind: &EventKind<'_>,
        source: &str,
        scratch: &mut String,
    ) -> Option<NodeId> {
        let EventKind::Start(Tag::Link(target) | Tag::Image(target)) = kind else {
            unreachable!("only a reference is bound")
        };
        let reference = target.reference().expect("only a reference is bound");
        self.definition(text(&reference.label, source), scratch)
    }

    // Whether `kind` names nothing in this table: an unbound reference or footnote call.
    pub(crate) fn unbound(&self, kind: &EventKind<'_>, source: &str, scratch: &mut String) -> bool {
        match kind {
            EventKind::Start(
                Tag::Link(Target::Reference(reference)) | Tag::Image(Target::Reference(reference)),
            ) => self
                .definition(text(&reference.label, source), scratch)
                .is_none(),
            EventKind::FootnoteReference { identifier, .. } => {
                self.footnote_definition(text(identifier, source)).is_none()
            }
            _ => false,
        }
    }
}

// The destination and title of definition `id`, which a lookup in a table found.
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

// The destination and title that the link or image `kind` leads to under `table`: its own for a
// resource, and its definition's for a bound reference.
pub(crate) fn destination<'d>(
    source: &'d str,
    nodes: &'d [Node],
    table: &Table<'_>,
    kind: &'d EventKind<'static>,
) -> Option<(&'d str, Option<&'d str>)> {
    let EventKind::Start(Tag::Link(target) | Tag::Image(target)) = kind else {
        return None;
    };
    let (destination, title) = match target.destination() {
        Some(pair) => pair,
        None => definition_target(
            nodes,
            table.definition_of(kind, source, &mut String::new())?,
        ),
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
            table: Table::new(source, nodes, root, true),
            attached,
        }
    }

    /// The link reference definition that a reference labelled `label` leads to, if any.
    pub fn definition(&self, label: &str) -> Option<NodeId> {
        self.table.definition(label, &mut String::new())
    }

    /// The footnote definition that a footnote call with identifier `identifier` calls, if any.
    /// The identifier is the normalized one the call stores.
    pub fn footnote_definition(&self, identifier: &str) -> Option<NodeId> {
        self.table.footnote_definition(identifier)
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
            &self.table,
            &self.nodes[id.index()].kind,
        )
    }

    // The definition the reference `target` names.
    pub(crate) fn definition_of(&self, target: &Target<'_>) -> Option<NodeId> {
        self.definition(text(&target.reference()?.label, self.source))
    }

    // Whether the reference or footnote call `kind` names nothing in the attached document.
    pub(crate) fn unbound(&self, kind: &EventKind<'_>) -> bool {
        self.table.unbound(kind, self.source, &mut String::new())
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
