// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Copying content from one document into another.
use super::{borrow_kind, defines, ends_item, tree_root, Document, NodeId};
use crate::bindings::{Lookup, Scratch, Table};
use crate::stream::{EventKind, StreamError, Tag, Target, Text};
use crate::NodeSyntax;
use alloc::{
    collections::{BTreeMap, BTreeSet},
    string::String,
    vec::Vec,
};
use core::convert::TryFrom;
use core::fmt::Write;

/// What [`Document::insert_from`] does with a footnote definition that it copies when this
/// document already binds its identifier, or has a call of it that nothing binds, which the copy
/// would bind.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FootnotePolicy {
    /// Give the copy a fresh identifier, one of `identifier-2`, `identifier-3` and so on that
    /// this document neither binds nor calls and that no other copied definition takes, and
    /// rewrite the copied calls to match. It is chosen deterministically, and is the first such
    /// one when those taken run from `identifier-2` without a gap, as relabelling makes them, but
    /// past a gap it can be a later one: with `x-2`, `x-3` and `x-5` taken, `x` is relabelled
    /// `x-6`, not `x-4`, which keeps finding it logarithmic in the number taken.
    #[default]
    Relabel,
    /// Reject the insert with [`StreamError::InvalidValue`].
    Reject,
}

/// How [`Document::insert_from`] copies content.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InsertOptions {
    /// What to do with a copied footnote definition whose identifier is already bound.
    pub footnotes: FootnotePolicy,
    /// Added to the level of each copied heading. A level outside 1 to 6 is rejected with
    /// [`StreamError::InvalidValue`], not clamped.
    pub heading_shift: i8,
}

/// The nodes that [`Document::insert_from`] added.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Inserted {
    /// The children that the insert added to the parent, in order: the copies of the selected
    /// nodes, the separators that go between them and the parent's children beside them, and,
    /// for a selected reference that nothing binds, the text around its children and the copies
    /// of its children, which it becomes. Definitions and footnote definitions among the
    /// selected nodes are not copied, nor the separators they leave redundant.
    pub nodes: Vec<NodeId>,
    /// The copies of the footnote definitions that the copied content calls, in the order they
    /// are first called, after the copied nodes.
    pub footnote_definitions: Vec<NodeId>,
}

// What an insert copies and where, decided before anything changes.
pub(super) struct Plan {
    // The selected source nodes that are copied into the parent, in order, with `None` for a new
    // separator.
    top: Vec<Option<NodeId>>,
    // The source footnote definitions that are copied, in the order first called, each with the
    // suffix of its fresh identifier when it is relabelled.
    definitions: Vec<(NodeId, Option<String>)>,
    // Where each copied source footnote definition is in `definitions`.
    by_source: BTreeMap<NodeId, usize>,
    // Where the footnote definitions go: the container, and its child after which they go, or
    // `None` for after the last copied node.
    definitions_in: Option<(NodeId, Option<NodeId>)>,
    // The spread of the parent when it is an item.
    spread: Option<bool>,
    heading_shift: i8,
}

// The kept children of `id` in `source`: all of them for a container that cannot hold a
// definition, and otherwise all but the definitions and footnote definitions, which are not
// copied, and the separators they leave redundant.
fn kept_children<T: AsRef<str>>(source: &Document<T>, id: NodeId) -> Vec<NodeId> {
    if crate::bindings::holds_blocks(&source.nodes[id.index()].kind) {
        prune(source, source.children(id), true)
    } else {
        source.children(id).collect()
    }
}

// `children` but for definitions and footnote definitions, and for a separator that follows one
// when it would then be first or follow another separator, and, when `nested`, as the parser
// writes no separator after the last block of a container other than the root, the separators
// left after the last block when a definition followed it.
fn prune<T: AsRef<str>>(
    source: &Document<T>,
    children: impl IntoIterator<Item = NodeId>,
    nested: bool,
) -> Vec<NodeId> {
    let mut kept: Vec<NodeId> = Vec::new();
    let mut removed = false;
    let mut trailing = false;
    for child in children {
        let kind = &source.nodes[child.index()].kind;
        if defines(kind) {
            removed = true;
            trailing = true;
            continue;
        }
        let separator = source.separator(child);
        if separator && removed && kept.last().map_or(true, |&last| source.separator(last)) {
            continue;
        }
        trailing &= separator;
        removed = false;
        kept.push(child);
    }
    if nested && trailing {
        while kept.last().is_some_and(|&last| source.separator(last)) {
            kept.pop();
        }
    }
    kept
}

// Visit each node copied from `roots` of `source` in preorder: each root and its kept children.
fn each_kept<T: AsRef<str>>(
    source: &Document<T>,
    roots: Vec<NodeId>,
    mut visit: impl FnMut(NodeId) -> Result<(), StreamError>,
) -> Result<(), StreamError> {
    let mut stack = alloc::vec![roots.into_iter()];
    while let Some(children) = stack.last_mut() {
        let Some(id) = children.next() else {
            stack.pop();
            continue;
        };
        visit(id)?;
        if source.nodes[id.index()].first_child.is_some() {
            stack.push(kept_children(source, id).into_iter());
        }
    }
    Ok(())
}

// The level of a heading of `level` shifted by `shift`, if it is one.
fn shifted(level: u8, shift: i8) -> Result<u8, StreamError> {
    u8::try_from(i16::from(level) + i16::from(shift))
        .ok()
        .filter(|level| (1..=6).contains(level))
        .ok_or(StreamError::InvalidValue)
}

// `value`, resolved in `source`, with `suffix` after it.
fn suffixed(value: &Text<'_>, source: &str, suffix: &str) -> Text<'static> {
    let mut text = String::from(value.resolve(source).expect("document text is validated"));
    text.push_str(suffix);
    Text::from(text)
}

// The suffix `-j` of the first fresh identifier from `identifier` that `taken` does not hold,
// of `-2`, `-3` and so on, found with as few lookups as `taken` needs, logarithmic in `j`: it
// gallops to a free suffix and searches back for the first after the taken ones before it, which
// is the first free one when the taken ones run from `-2` without a gap, as relabelling makes
// them, and otherwise a free one all the same.
fn fresh_suffix(identifier: &str, mut taken: impl FnMut(&str) -> bool) -> String {
    let mut candidate = String::new();
    let mut free = |j: u64| {
        candidate.clear();
        write!(candidate, "{identifier}-{j}").expect("a string takes any text");
        !taken(&candidate)
    };
    let (mut low, mut high) = (1, 2);
    let mut step = 1;
    while !free(high) {
        low = high;
        high += step;
        step *= 2;
    }
    while high - low > 1 {
        let middle = low + (high - low) / 2;
        if free(middle) {
            high = middle;
        } else {
            low = middle;
        }
    }
    alloc::format!("-{high}")
}

impl<S: AsRef<str>> Document<S> {
    /// Copy nodes of another document into this one, as children of `parent` after `after`, or
    /// first when it is `None`, materializing their source-backed values into this document.
    ///
    /// The place is given by the child to insert after, not before, unlike
    /// [`Document::attach`]'s `before`: a node links only to its next sibling, so the place
    /// after a child is found without walking the children, and the end of a heading's section,
    /// the common place to insert, is the node that [`Document::heading_section_last`] gives.
    ///
    /// `nodes` are nodes of `source`'s attached document, often its root's children, copied in
    /// order with what they hold. The copies render as the nodes render in `source`:
    ///
    /// - A reference bound there becomes a resource to where it leads, as this document may
    ///   define its label otherwise, so link reference definitions are not copied, wherever they
    ///   are among the nodes. A reference or footnote call that nothing binds there becomes the
    ///   text it renders as, as [`Document::normalize`] makes it: a reference becomes the text
    ///   around its children, which follow it, so one copy can be several children.
    /// - A footnote call brings the footnote definition it calls in `source`, wherever it is,
    ///   copied once however many calls there are, and so do the calls in a copied definition.
    ///   The other footnote definitions among the nodes are not copied. The copied definitions go
    ///   after the copied nodes, in the order they are first called: in `parent`, or, when it
    ///   cannot hold one, such as a paragraph or list, or is an item of a tight list, which one
    ///   would make loose, after its nearest ancestor in a container that can.
    /// - A copied definition's identifier is taken when this document, in the tree that holds
    ///   `parent`, the attached document or a detached subtree, binds it, or has a call of it
    ///   that nothing binds, which the copy would make call it. [`InsertOptions::footnotes`]
    ///   decides what a copy of a taken identifier gets: by default a fresh one, as
    ///   [`FootnotePolicy::Relabel`] describes, and its copied calls are rewritten to match. So
    ///   a call here keeps calling what it called, or nothing.
    /// - Into a container of blocks, the separators between the copies, and between them and
    ///   the children beside them, are those the parser writes there: one after each block that
    ///   something follows, but for one after a definition, after a paragraph of an item of a
    ///   tight list before a block other than a paragraph, and, as for
    ///   [`Document::parse_children`], after the last block of a container other than the root.
    ///   So two paragraphs of an item of a tight list stay two. An item is spread when it was,
    ///   or, unless it is in a tight list, when the copied blocks had a blank line between two of
    ///   them.
    /// - [`InsertOptions::heading_shift`] is added to each copied heading's level.
    ///
    /// Frontmatter is never copied. This takes time linear in the copied content, plus one
    /// lookup in each document's index for each reference and footnote call, and, for each
    /// relabelled definition, lookups logarithmic in the number of its identifier's relabelled
    /// copies already here. Copying a footnote definition costs one pass over the tree that holds
    /// `parent` when this document has no index of it, a detached subtree, as for the lookups of
    /// #226, and when it may have a call that nothing binds: when it was edited, other than by
    /// this, since it was parsed, built with every reference bound, or normalized. So many inserts
    /// of footnotes into one detached subtree, or into a document so edited, take quadratic time.
    /// Inserting into phrasing walks the parent's phrasing ancestors, in time linear in the depth
    /// of the phrasing nest (#233).
    ///
    /// To insert another document's content under a heading, after the heading's section:
    ///
    /// ```
    /// use base_markdown::{to_markdown, Document, HeadingOptions, InsertOptions, ParseOptions};
    ///
    /// let options = ParseOptions::gfm();
    /// let mut document = Document::parse(
    ///     "# Guide\n\n## Install\n\nRun it.[^1]\n\n## Use\n\nCall it.\n\n[^1]: Once.\n".to_owned(),
    ///     &options,
    /// )
    /// .unwrap();
    /// let other = Document::parse(
    ///     "# Upgrade\n\nRun it again.[^1]\n\n[^1]: After each release.\n".to_owned(),
    ///     &options,
    /// )
    /// .unwrap();
    ///
    /// // Find the heading, and the last node of its section and subsections.
    /// let install = document
    ///     .headings(&HeadingOptions::default())
    ///     .into_iter()
    ///     .find(|heading| heading.text == "Install")
    ///     .unwrap()
    ///     .node;
    /// let last = document.heading_section_last(install, true).unwrap();
    ///
    /// // Insert the other document there, one level deeper than its own headings were.
    /// let options = InsertOptions { heading_shift: 2, ..InsertOptions::default() };
    /// document
    ///     .insert_from(document.root(), Some(last), &other, other.children(other.root()), &options)
    ///     .unwrap();
    /// assert_eq!(
    ///     to_markdown(&document).unwrap(),
    ///     "# Guide\n\n## Install\n\nRun it.[^1]\n\n### Upgrade\n\nRun it again.[^1-2]\n\n\
    ///      [^1-2]: After each release.\n\n## Use\n\nCall it.\n\n[^1]: Once.\n"
    /// );
    /// ```
    ///
    /// # Errors
    /// Rejects an invalid parent, `after` that is not a child of `parent`, and a node that is not
    /// in `source`'s attached document with [`StreamError::InvalidRange`], and a leaf parent
    /// with [`StreamError::Unbalanced`]. Rejects with [`StreamError::InvalidNesting`] a copy that
    /// the parent cannot hold, as for [`Document::append_child`], such as a document root, a
    /// block in a paragraph or phrasing in a root that holds blocks, or that holds a footnote
    /// call when the parent is in a link, or a soft or hard break when it is in a table cell, and
    /// a copied footnote definition that no container from the parent up can hold. Rejects with
    /// [`StreamError::InvalidValue`] a shifted heading level outside 1 to 6, a copied footnote
    /// definition whose identifier is taken here, bound or called by a call that nothing binds,
    /// when [`InsertOptions::footnotes`] is [`FootnotePolicy::Reject`], and a fresh identifier, label or call too long to be one.
    /// Nothing is changed when an error is returned.
    pub fn insert_from<T: AsRef<str>>(
        &mut self,
        parent: NodeId,
        after: Option<NodeId>,
        source: &Document<T>,
        nodes: impl IntoIterator<Item = NodeId>,
        options: &InsertOptions,
    ) -> Result<Inserted, StreamError> {
        let settled = self.settled;
        let inserted = self.edit(|document| {
            let next = match document.node(parent) {
                Some(node) => match after {
                    None => node.first_child,
                    Some(after) => match document.node(after) {
                        Some(node) if node.parent == Some(parent) => node.next_sibling,
                        _ => return Err(StreamError::InvalidRange),
                    },
                },
                None => return Err(StreamError::InvalidRange),
            };
            let plan = document.plan(
                parent,
                (after, next),
                source,
                nodes,
                options,
                (&[], &[]),
                settled,
            )?;
            Ok(document.apply(&plan, parent, after, next, source))
        })?;
        // The copies' references and calls are bound or text, and the copied definitions bind no
        // identifier that a call here names, so every reference that was bound still is.
        self.settled = settled;
        Ok(inserted)
    }

    /// The last node of the section of the heading `heading`: the last of its following
    /// siblings before the next heading, or, with `subsections`, before the next heading no
    /// deeper than it, or the heading itself when the section is empty. Inserting after it with
    /// [`Document::insert_from`] inserts at the end of the section. `None` when `heading` is not
    /// a heading. This takes time linear in the siblings in the section.
    pub fn heading_section_last(&self, heading: NodeId, subsections: bool) -> Option<NodeId> {
        let EventKind::Start(Tag::Heading(level)) = self.node(heading)?.kind else {
            return None;
        };
        let mut last = heading;
        while let Some(next) = self.nodes[last.index()].next_sibling {
            if matches!(self.nodes[next.index()].kind,
                EventKind::Start(Tag::Heading(deeper)) if !subsections || deeper <= level)
            {
                break;
            }
            last = next;
        }
        Some(last)
    }

    // Plan copying `nodes` of `source` into `parent` after `after` and before `next`, checking
    // everything that can fail. `replacing` holds the sorted children of `parent` that the insert
    // replaces, and `leaving` the sorted definitions in them, which no longer bind. `settled` is
    // whether every reference and footnote call of this document was bound before the edit.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn plan<T: AsRef<str>>(
        &self,
        parent: NodeId,
        (after, next): (Option<NodeId>, Option<NodeId>),
        source: &Document<T>,
        nodes: impl IntoIterator<Item = NodeId>,
        options: &InsertOptions,
        (replacing, leaving): (&[NodeId], &[NodeId]),
        settled: bool,
    ) -> Result<Plan, StreamError> {
        // Callers check that the parent is a node of this document.
        let EventKind::Start(tag) = &self.nodes[parent.index()].kind else {
            return Err(StreamError::Unbalanced);
        };
        let mut selected = Vec::new();
        for id in nodes {
            if !source.node(id).is_some_and(|_| source.attached(id)) {
                return Err(StreamError::InvalidRange);
            }
            selected.push(id);
        }
        let nested = parent != self.root();
        let tight = nested && self.tight_item(parent);
        let pruned = prune(source, selected, false);
        // Into a container of blocks, the separators between the copies and the parent's children
        // beside them are those the parser writes there: one after a block that something
        // follows, but for none after a definition, which takes the place of the separator after
        // it, none after a paragraph of an item of a tight list before a block other than a
        // paragraph, and none after the last block of a container other than the root. A copied
        // separator is kept where one goes, and a new one goes where one goes and none was copied.
        let top: Vec<Option<NodeId>> = if matches!(
            tag.kind().content(),
            crate::stream::Content::Root | crate::stream::Content::Blocks
        ) {
            let paragraph =
                |kind: &EventKind<'_>| matches!(kind, EventKind::Start(Tag::Paragraph { .. }));
            // Whether a separator goes between a block, a paragraph or not, and the next.
            let apart = |left: bool, right: bool| !(tight && left && !right);
            // Whether the child `id` is a paragraph, if it is not a separator.
            let block =
                |id: NodeId| (!self.separator(id)).then(|| paragraph(&self.nodes[id.index()].kind));
            // Whether the block before the next copy is a paragraph, if one that a separator
            // follows is right before it.
            let mut left = after
                .filter(|&id| !defines(&self.nodes[id.index()].kind))
                .and_then(block);
            let mut copied_separator = None;
            let mut top = Vec::with_capacity(pruned.len() + 2);
            for id in pruned {
                if source.separator(id) {
                    copied_separator = copied_separator.or(Some(id));
                    continue;
                }
                let right = paragraph(&source.nodes[id.index()].kind);
                if left.is_some_and(|left| apart(left, right)) {
                    top.push(copied_separator);
                }
                copied_separator = None;
                top.push(Some(id));
                left = Some(right);
            }
            match next {
                Some(next) => {
                    if let (Some(left), Some(right)) = (left, block(next)) {
                        if apart(left, right) {
                            top.push(copied_separator);
                        }
                    }
                }
                // The root's last line ending is a separator after its last block.
                None if !nested && left.is_some() => top.extend(copied_separator.map(Some)),
                None => {}
            }
            top
        } else {
            pruned.into_iter().map(Some).collect()
        };
        // The content model: each copy against the parent, whose first child is its own or the
        // first copy.
        let first = self.nodes[parent.index()]
            .first_child
            .map(|id| &self.nodes[id.index()].kind)
            .or_else(|| {
                top.iter()
                    .flatten()
                    .next()
                    .map(|id| &source.nodes[id.index()].kind)
            });
        if top
            .iter()
            .flatten()
            .any(|id| !source.nodes[id.index()].kind.nests_in(tag, first))
        {
            return Err(StreamError::InvalidNesting);
        }
        let reach = self.reach(parent);
        let shift = options.heading_shift;
        let cx = source.context();
        let mut lookup = Lookup::new(cx, source.root());
        let mut scratch = Scratch::default();
        // The copied calls that bind, to be checked once their definitions have identifiers, and
        // the definitions they call, each once, in the order first called.
        let mut calls = Vec::new();
        let mut definitions: Vec<(NodeId, Option<String>)> = Vec::new();
        let mut by_source = BTreeMap::new();
        let mut visit = |calls: &mut Vec<NodeId>,
                         definitions: &mut Vec<(NodeId, Option<String>)>,
                         id: NodeId,
                         restricted: bool| {
            let kind = &source.nodes[id.index()].kind;
            if restricted && !reach.allows(kind) {
                return Err(StreamError::InvalidNesting);
            }
            match kind {
                EventKind::Start(Tag::Heading(level)) => {
                    shifted(*level, shift)?;
                }
                EventKind::FootnoteReference { identifier, .. } => {
                    let identifier = identifier
                        .resolve(source.source())
                        .expect("document text is validated");
                    if let Some(definition) =
                        lookup.footnote_definition(cx, identifier, &mut scratch)
                    {
                        calls.push(id);
                        by_source.entry(definition).or_insert_with(|| {
                            definitions.push((definition, None));
                            definitions.len() - 1
                        });
                    }
                }
                _ => {}
            }
            Ok(())
        };
        each_kept(source, top.iter().flatten().copied().collect(), |id| {
            visit(&mut calls, &mut definitions, id, !reach.is_empty())
        })?;
        // The calls of the copied definitions bring definitions too.
        let mut at = 0;
        while at < definitions.len() {
            let definition = definitions[at].0;
            each_kept(source, alloc::vec![definition], |id| {
                visit(&mut calls, &mut definitions, id, false)
            })?;
            at += 1;
        }
        drop(visit);
        let definitions_in = if definitions.is_empty() {
            None
        } else {
            Some(self.definitions_place(
                parent,
                first,
                &source.nodes[definitions[0].0.index()].kind,
            )?)
        };
        // Identifiers: those this document binds, other than `leaving`, clash.
        let attached = self.attached(parent);
        let table = (!attached && !definitions.is_empty()).then(|| {
            Table::without(
                self.source(),
                &self.nodes,
                tree_root(&self.nodes, parent),
                |id| leaving.binary_search(&id).is_ok(),
            )
        });
        let binds = |key: &str| match &table {
            Some(table) => table.binds(true, key),
            None => self
                .index
                .binds(true, key, |id| leaving.binary_search(&id).is_ok()),
        };
        let identifier = |id: NodeId| {
            let EventKind::Start(Tag::FootnoteDefinition { identifier, .. }) =
                &source.nodes[id.index()].kind
            else {
                unreachable!("a call binds a footnote definition")
            };
            identifier.resolve(source.source()).expect("validated")
        };
        // The identifiers that a call of this document's tree names but nothing binds, other than
        // in the replaced children, which a copied definition would make it call. A settled
        // document has none. A call bound only by a definition that leaves is bound, and calls the
        // copy that takes its identifier, as the replaced children's definitions did.
        let mut unbound = BTreeSet::new();
        if !definitions.is_empty() && !settled {
            let mut left = BTreeSet::new();
            for &id in leaving {
                let (footnote, key) =
                    crate::bindings::kind_key(&self.nodes[id.index()].kind, self.source());
                if footnote {
                    left.insert(key);
                }
            }
            let tree = tree_root(&self.nodes, parent);
            let mut current = Some(tree);
            while let Some(id) = current {
                let node = &self.nodes[id.index()];
                if let EventKind::FootnoteReference { identifier, .. } = &node.kind {
                    let key = identifier
                        .resolve(self.source())
                        .expect("document text is validated");
                    if !binds(key) && !left.contains(key) {
                        unbound.insert(String::from(key));
                    }
                }
                current = match node.first_child {
                    Some(child) if replacing.binary_search(&id).is_err() => Some(child),
                    _ => crate::bindings::next_after(&self.nodes, tree, id),
                };
            }
        }
        let taken = |key: &str| binds(key) || unbound.contains(key);
        let mut clashes = Vec::new();
        let mut used = BTreeSet::new();
        for (index, (definition, _)) in definitions.iter().enumerate() {
            let key = identifier(*definition);
            if taken(key) {
                if options.footnotes == FootnotePolicy::Reject {
                    return Err(StreamError::InvalidValue);
                }
                clashes.push(index);
            } else {
                used.insert(String::from(key));
            }
        }
        // A fresh identifier, label or call can be too long to be one.
        let mut valid = true;
        for index in clashes {
            let key = identifier(definitions[index].0);
            let suffix = fresh_suffix(key, |candidate| {
                used.contains(candidate) || taken(candidate)
            });
            let mut fresh = String::from(key);
            fresh.push_str(&suffix);
            used.insert(fresh);
            let copy = Self::relabelled(source, definitions[index].0, &suffix);
            valid &= copy.validate(None).is_ok();
            definitions[index].1 = Some(suffix);
        }
        for call in calls {
            if let Some(kind) = Self::relabelled_call(source, &definitions, &by_source, call) {
                valid &= kind.validate(None).is_ok();
            }
        }
        if !valid {
            return Err(StreamError::InvalidValue);
        }
        // An item's spread is read from the source between its blocks, which the copies are not
        // in, so it is kept, and is the copied blocks', unless it is in a tight list.
        let spread = matches!(
            self.nodes[parent.index()].kind,
            EventKind::Start(Tag::Item { .. })
        )
        .then(|| {
            let mut spread = matches!(
                self.syntax(parent),
                Some(NodeSyntax::ListItem { spread: true })
            );
            if !tight && !spread {
                let mut previous: Option<NodeId> = None;
                for &id in top.iter().flatten().filter(|&&id| !source.separator(id)) {
                    if let Some(previous) = previous {
                        let end = source.nodes[previous.index()].span.end;
                        let start = source.nodes[id.index()].span.start;
                        spread |= source.blank_line_between(end, start)
                            || source.ends_with_blank_line(previous);
                    }
                    previous = Some(id);
                }
            }
            spread && !tight
        });
        Ok(Plan {
            top,
            definitions,
            by_source,
            definitions_in,
            spread,
            heading_shift: shift,
        })
    }

    // Where copied footnote definitions go when copying into `parent`: the container, and the
    // child of it after which they go, or `None` for after the last copy.
    fn definitions_place(
        &self,
        parent: NodeId,
        parent_first: Option<&EventKind<'_>>,
        definition: &EventKind<'_>,
    ) -> Result<(NodeId, Option<NodeId>), StreamError> {
        let mut container = parent;
        let mut anchor = None;
        loop {
            let EventKind::Start(tag) = &self.nodes[container.index()].kind else {
                unreachable!("a parent is a container");
            };
            // The first child of the parent is its own or the first copy, and an ancestor has
            // one.
            let first = if container == parent {
                parent_first
            } else {
                self.nodes[container.index()]
                    .first_child
                    .map(|id| &self.nodes[id.index()].kind)
            };
            // In an item of a tight list, a definition after a paragraph is written after a blank
            // line, which would make the list loose.
            if definition.nests_in(tag, first.or(Some(definition))) && !self.tight_item(container) {
                return Ok((container, anchor));
            }
            anchor = Some(container);
            container = self.nodes[container.index()]
                .parent
                .ok_or(StreamError::InvalidNesting)?;
        }
    }

    // The footnote definition `id` of `source`, relabelled with `suffix`.
    fn relabelled<T: AsRef<str>>(
        source: &Document<T>,
        id: NodeId,
        suffix: &str,
    ) -> EventKind<'static> {
        let EventKind::Start(Tag::FootnoteDefinition { identifier, label }) =
            &source.nodes[id.index()].kind
        else {
            unreachable!("only a footnote definition is relabelled")
        };
        EventKind::Start(Tag::FootnoteDefinition {
            identifier: suffixed(identifier, source.source(), suffix),
            label: label
                .as_ref()
                .map(|label| suffixed(label, source.source(), suffix)),
        })
    }

    // The footnote call `id` of `source`, relabelled as the definition it calls is, if it is.
    fn relabelled_call<T: AsRef<str>>(
        source: &Document<T>,
        definitions: &[(NodeId, Option<String>)],
        by_source: &BTreeMap<NodeId, usize>,
        id: NodeId,
    ) -> Option<EventKind<'static>> {
        let EventKind::FootnoteReference { identifier, label } = &source.nodes[id.index()].kind
        else {
            unreachable!("only a footnote call is relabelled")
        };
        let definition = source
            .footnote_definition(
                identifier
                    .resolve(source.source())
                    .expect("document text is validated"),
            )
            .expect("only a bound call is relabelled");
        let suffix = definitions[by_source[&definition]].1.as_deref()?;
        Some(EventKind::FootnoteReference {
            identifier: suffixed(identifier, source.source(), suffix),
            label: label
                .as_ref()
                .map(|label| suffixed(label, source.source(), suffix)),
        })
    }

    // Copy what `plan` copies of `source` into `parent`, after `after` and before `next`.
    pub(super) fn apply<T: AsRef<str>>(
        &mut self,
        plan: &Plan,
        parent: NodeId,
        after: Option<NodeId>,
        next: Option<NodeId>,
        source: &Document<T>,
    ) -> Inserted {
        let attached = self.attached(parent);
        let mut inserted = Inserted::default();
        let mut previous = after;
        let separator = |document: &mut Self, previous: Option<NodeId>| {
            let kind = EventKind::Separator {
                value: Text::Literal("\n"),
                conditional: false,
            };
            document.insert_child(parent, kind, previous, next)
        };
        for &id in &plan.top {
            let Some(id) = id else {
                let id = separator(self, previous);
                inserted.nodes.push(id);
                previous = Some(id);
                continue;
            };
            let (first, last) = self.copy_kept(source, plan, id, parent, previous, next);
            self.inserted_run(attached, first, last, &mut inserted.nodes);
            previous = Some(last);
        }
        // A footnote definition ends no item, so its container's ending stays as it was.
        if let Some((container, anchor)) = plan.definitions_in {
            let mut previous = anchor.or(previous);
            let next = previous.map_or(self.nodes[container.index()].first_child, |previous| {
                self.nodes[previous.index()].next_sibling
            });
            for &(id, _) in &plan.definitions {
                let (first, last) = self.copy_kept(source, plan, id, container, previous, next);
                self.inserted_run(attached, first, last, &mut inserted.footnote_definitions);
                previous = Some(last);
            }
        }
        if let Some(spread) = plan.spread {
            self.syntax
                .insert(parent, Some(NodeSyntax::ListItem { spread }));
        }
        // An item ends as its last child that can end it does: in a loose list, any such child
        // ends it as a block, and in a tight list, the last such copy, unless a child after it can,
        // which is found without passing those before. A tight item holds no copied footnote
        // definition, so only the separators and definitions it had can come between.
        if matches!(
            self.nodes[parent.index()].kind,
            EventKind::Start(Tag::Item { .. })
        ) {
            let ending = |document: &Self, id: NodeId| ends_item(&document.nodes[id.index()].kind);
            if let Some(&last) = inserted.nodes.iter().rev().find(|&&id| ending(self, id)) {
                let later = self.tight_item(parent)
                    && core::iter::successors(self.nodes[last.index()].next_sibling, |id| {
                        self.nodes[id.index()].next_sibling
                    })
                    .any(|id| ending(self, id));
                if !later {
                    self.refresh_item_after_append(parent, last);
                }
            }
        }
        inserted
    }

    // Index and list the siblings from `first` to `last` that copying one node made.
    fn inserted_run(&mut self, attached: bool, first: NodeId, last: NodeId, ids: &mut Vec<NodeId>) {
        let mut at = first;
        loop {
            if attached {
                self.index
                    .insert_tree(self.source.as_ref(), &self.nodes, at);
            }
            ids.push(at);
            if at == last {
                break;
            }
            at = self.nodes[at.index()]
                .next_sibling
                .expect("a copy's run ends in its last node");
        }
    }

    // Copy the subtree at `root` of `source` and its kept children into `parent`, after
    // `previous` and before `next`, as `plan` copies it, giving the first and last of the
    // siblings it makes: one, unless it is a reference that nothing binds, which becomes the text
    // around its children.
    fn copy_kept<T: AsRef<str>>(
        &mut self,
        source: &Document<T>,
        plan: &Plan,
        root: NodeId,
        parent: NodeId,
        previous: Option<NodeId>,
        next: Option<NodeId>,
    ) -> (NodeId, NodeId) {
        let cx = source.context();
        let mut lookup = Lookup::new(cx, source.root());
        let mut scratch = Scratch::default();
        // The copies of the references and footnote calls that nothing binds in `source`, which
        // become the text they read back as there.
        let mut unbound = Vec::new();
        let mut copy = |document: &mut Self,
                        id: NodeId,
                        parent: NodeId,
                        previous: Option<NodeId>,
                        next: Option<NodeId>| {
            let node = &source.nodes[id.index()];
            let mut kind =
                borrow_kind(&node.kind).map_text(|value| value.materialized(source.source()));
            let mut unbound_here = false;
            match &mut kind {
                EventKind::Start(Tag::Heading(level)) => {
                    *level = shifted(*level, plan.heading_shift).expect("planned");
                }
                // A bound reference crosses documents as a resource to where it leads.
                EventKind::Start(Tag::Link(target) | Tag::Image(target))
                    if target.reference().is_some() =>
                {
                    match lookup.definition_of(cx, &node.kind, &mut scratch) {
                        Some(definition) => {
                            let (destination, title) =
                                crate::bindings::definition_target(&source.nodes, definition);
                            *target = Target::Resource {
                                destination: destination.clone().materialized(source.source()),
                                title: title
                                    .map(|title| title.clone().materialized(source.source())),
                            };
                        }
                        None => unbound_here = true,
                    }
                }
                EventKind::FootnoteReference { .. } => {
                    if lookup.unbound(cx, &node.kind, &mut scratch) {
                        unbound_here = true;
                    } else if let Some(relabelled) =
                        Self::relabelled_call(source, &plan.definitions, &plan.by_source, id)
                    {
                        kind = relabelled;
                    }
                }
                EventKind::Start(Tag::FootnoteDefinition { .. }) => {
                    let index = plan.by_source[&id];
                    if let Some(suffix) = &plan.definitions[index].1 {
                        kind = Self::relabelled(source, id, suffix);
                    }
                }
                _ => {}
            }
            let copy = document.insert_child(parent, kind, previous, next);
            if unbound_here {
                unbound.push(copy);
            }
            // An item's spread and a link's form describe only the copied node, so they survive.
            let syntax = match source.syntax(id) {
                Some(NodeSyntax::ListItem { spread }) => Some(NodeSyntax::ListItem { spread }),
                Some(NodeSyntax::Link { form }) => Some(NodeSyntax::Link { form }),
                _ => None,
            };
            if let Some(syntax) = syntax {
                document
                    .set_syntax(copy, Some(syntax))
                    .expect("syntax copied from the same kind of node");
            }
            copy
        };
        let copied_root = copy(self, root, parent, previous, next);
        let mut stack = Vec::new();
        if source.nodes[root.index()].first_child.is_some() {
            stack.push((kept_children(source, root).into_iter(), copied_root, None));
        }
        while let Some((children, parent, previous)) = stack.last_mut() {
            let Some(id) = children.next() else {
                stack.pop();
                continue;
            };
            let (parent, last) = (*parent, *previous);
            let copied = copy(self, id, parent, last, None);
            *previous = Some(copied);
            if source.nodes[id.index()].first_child.is_some() {
                stack.push((kept_children(source, id).into_iter(), copied, None));
            }
        }
        for id in unbound {
            self.unbind_node(id);
        }
        let mut last = copied_root;
        while let Some(after) = self.nodes[last.index()].next_sibling {
            if Some(after) == next {
                break;
            }
            last = after;
        }
        self.mark_added();
        self.refresh_context(copied_root);
        (copied_root, last)
    }
}
