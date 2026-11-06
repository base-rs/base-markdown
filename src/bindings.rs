// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! The bindings of a document's references: which definition each label names.
use crate::definitions::{owned_reference_key, reference_key};
use crate::document::{next_descendant, Node, NodeId};
use crate::stream::{EventKind, Tag, Target, Text};
use crate::ReferenceKind;
use alloc::{borrow::Cow, string::String, vec::Vec};

/// The definition each label of one tree names, built in one pass in document order.
///
/// A reference is a name and a definition a binding of it: the first link reference definition
/// in document order whose label matches a reference's, as the parser matches labels, is the one
/// it leads to, and the first footnote definition with a footnote call's identifier is the one
/// it calls. A document stores no binding, so the next table built sees every edit to its
/// definitions. The table borrows the document, so it cannot outlive an edit.
///
/// [`Document::bindings`][crate::Document::bindings] builds the table of the attached document,
/// in time linear in its size, and log-linear in the number of its definitions; each lookup in
/// it then takes time logarithmic in the number of labels. Detached subtrees are not part of the document, so their definitions bind nothing in
/// it.
#[derive(Clone, Debug)]
pub struct Bindings<'d> {
    source: &'d str,
    nodes: &'d [Node],
    table: Table<'d>,
}

fn text<'d>(value: &'d Text<'_>, source: &'d str) -> &'d str {
    value.resolve(source).expect("document text is validated")
}

// The first definition of each reference key and footnote identifier of a tree, sorted by key.
// Keys borrow only the source, so the table outlives no borrow of the nodes and a pass can edit
// them while it looks up references.
#[derive(Clone, Debug, Default)]
pub(crate) struct Table<'s> {
    definitions: Vec<(Cow<'s, str>, NodeId)>,
    footnotes: Vec<(Cow<'s, str>, NodeId)>,
}

// Sort `entries`, in document order, by key, keeping the first of each key.
fn first_of_each(entries: &mut Vec<(Cow<'_, str>, NodeId)>) {
    // A stable sort keeps the definitions of one key in document order.
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    entries.dedup_by(|later, first| later.0 == first.0);
}

fn find(entries: &[(Cow<'_, str>, NodeId)], key: &str) -> Option<NodeId> {
    entries
        .binary_search_by(|(entry, _)| entry.as_ref().cmp(key))
        .ok()
        .map(|at| entries[at].1)
}

impl<'s> Table<'s> {
    // The table of the tree under `root`, which should be the root of a tree, attached or
    // detached.
    pub(crate) fn new(source: &'s str, nodes: &[Node], root: NodeId) -> Self {
        let mut table = Self::default();
        let mut current = Some(root);
        while let Some(id) = current {
            match &nodes[id.index()].kind {
                EventKind::Definition { label, .. } => {
                    let key = match label {
                        Text::Source(range) => reference_key(&source[range.clone()]),
                        label => Cow::Owned(owned_reference_key(text(label, source))),
                    };
                    table.definitions.push((key, id));
                }
                EventKind::Start(Tag::FootnoteDefinition { identifier, .. }) => {
                    // The parser gives a footnote definition the identifier its label
                    // normalizes to, which is not in the source.
                    let key = Cow::Owned(text(identifier, source).into());
                    table.footnotes.push((key, id));
                }
                _ => {}
            }
            current = next_descendant(nodes, root, id);
        }
        first_of_each(&mut table.definitions);
        first_of_each(&mut table.footnotes);
        table
    }

    // This table with keys of its own, borrowing nothing.
    pub(crate) fn into_owned(self) -> Table<'static> {
        let own = |entries: Vec<(Cow<'s, str>, NodeId)>| {
            entries
                .into_iter()
                .map(|(key, id)| (Cow::Owned(key.into_owned()), id))
                .collect()
        };
        Table {
            definitions: own(self.definitions),
            footnotes: own(self.footnotes),
        }
    }

    // The definition a reference labelled `label` leads to.
    pub(crate) fn definition(&self, label: &str) -> Option<NodeId> {
        find(&self.definitions, reference_key(label).as_ref())
    }

    // The footnote definition a call with identifier `identifier` calls.
    pub(crate) fn footnote_definition(&self, identifier: &str) -> Option<NodeId> {
        find(&self.footnotes, identifier)
    }

    // The definition the reference `kind`, a link or image, leads to.
    pub(crate) fn definition_of(&self, kind: &EventKind<'_>, source: &str) -> Option<NodeId> {
        let EventKind::Start(Tag::Link(target) | Tag::Image(target)) = kind else {
            unreachable!("only a reference is bound")
        };
        let reference = target.reference().expect("only a reference is bound");
        self.definition(text(&reference.label, source))
    }

    // Whether `kind` names nothing in this table: an unbound reference or footnote call.
    pub(crate) fn unbound(&self, kind: &EventKind<'_>, source: &str) -> bool {
        match kind {
            EventKind::Start(
                Tag::Link(Target::Reference(reference)) | Tag::Image(Target::Reference(reference)),
            ) => self.definition(text(&reference.label, source)).is_none(),
            EventKind::FootnoteReference { identifier, .. } => {
                self.footnote_definition(text(identifier, source)).is_none()
            }
            _ => false,
        }
    }
}

impl<'d> Bindings<'d> {
    // The bindings of the tree under `root`, or none without one.
    pub(crate) fn new(source: &'d str, nodes: &'d [Node], root: Option<NodeId>) -> Self {
        Self {
            source,
            nodes,
            table: root.map_or_else(Table::default, |root| Table::new(source, nodes, root)),
        }
    }

    /// The link reference definition that a reference labelled `label` leads to, if any.
    pub fn definition(&self, label: &str) -> Option<NodeId> {
        self.table.definition(label)
    }

    /// The footnote definition that a footnote call with identifier `identifier` calls, if any.
    /// The identifier is the normalized one the call stores.
    pub fn footnote_definition(&self, identifier: &str) -> Option<NodeId> {
        self.table.footnote_definition(identifier)
    }

    /// The destination and title that the link or image `id` leads to: its own for a resource,
    /// and its definition's for a bound reference. `None` for an unbound reference or a node that
    /// is not a link or image.
    pub fn destination(&self, id: NodeId) -> Option<(&'d str, Option<&'d str>)> {
        let EventKind::Start(Tag::Link(target) | Tag::Image(target)) =
            &self.nodes.get(id.index())?.kind
        else {
            return None;
        };
        let (destination, title) = match target.destination() {
            Some(pair) => pair,
            None => self.definition_target(self.definition_of(target)?),
        };
        Some((
            text(destination, self.source),
            title.map(|title| text(title, self.source)),
        ))
    }

    // The definition the reference `target` names.
    pub(crate) fn definition_of(&self, target: &Target<'_>) -> Option<NodeId> {
        self.definition(text(&target.reference()?.label, self.source))
    }

    // The destination and title of definition `id`, which a lookup in the table found.
    pub(crate) fn definition_target(
        &self,
        id: NodeId,
    ) -> (&'d Text<'static>, Option<&'d Text<'static>>) {
        let EventKind::Definition {
            destination, title, ..
        } = &self.nodes[id.index()].kind
        else {
            unreachable!("a binding is a definition")
        };
        (destination, title.as_ref())
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
