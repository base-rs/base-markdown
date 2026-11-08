// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Copying content from one document into another.
use super::{borrow_kind, defines, tree_root, Document, NodeId};
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
/// document already binds its identifier.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FootnotePolicy {
    /// Give the copy a fresh identifier, the first of `identifier-2`, `identifier-3` and so on
    /// that neither this document nor another copied definition binds, and rewrite the copied
    /// calls to match.
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
    /// The copies of the selected nodes, children of the parent, in order. Definitions and
    /// footnote definitions among the selected nodes are not copied, nor the separators they
    /// leave redundant.
    pub nodes: Vec<NodeId>,
    /// The copies of the footnote definitions that the copied content calls, in the order they
    /// are first called, after the copied nodes.
    pub footnote_definitions: Vec<NodeId>,
}

// What an insert copies and where, decided before anything changes.
pub(super) struct Plan {
    // The selected source nodes that are copied into the parent, in order.
    top: Vec<NodeId>,
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
    /// `nodes` are nodes of `source`'s attached document, often its root's children, copied in
    /// order with what they hold. The copies render as the nodes render in `source`:
    ///
    /// - A reference bound there becomes a resource to where it leads, as this document may
    ///   define its label otherwise, so link reference definitions are not copied, wherever they
    ///   are among the nodes. A reference or footnote call that nothing binds there becomes the
    ///   text it renders as, as [`Document::normalize`] makes it.
    /// - A footnote call brings the footnote definition it calls in `source`, wherever it is,
    ///   copied once however many calls there are, and so do the calls in a copied definition.
    ///   The other footnote definitions among the nodes are not copied. The copied definitions go
    ///   after the copied nodes, in the order they are first called: in `parent`, or, when it
    ///   cannot hold one, such as a paragraph or list, or is an item of a tight list, which one
    ///   would make loose, after its nearest ancestor in a container that can. When this document already binds a copied definition's identifier, in the
    ///   tree that holds `parent`, the attached document or a detached subtree,
    ///   [`InsertOptions::footnotes`] decides: by default it gets a fresh one, the first of
    ///   `identifier-2`, `identifier-3` and so on that is neither bound nor another copy's, and
    ///   its copied calls are rewritten to match. A copied definition binds its identifier for
    ///   the rest of this document, so a call of it here that nothing bound renders as a call of
    ///   it.
    /// - Separators that a definition left out leaves redundant are left out. Into a container
    ///   other than the root, as for [`Document::parse_children`], the separators after the last
    ///   copied block are left out when nothing follows, and so are those after a paragraph that
    ///   an item of a tight list makes tight, except before another paragraph, and an item is
    ///   spread when it was, or, unless it is in a tight list, when the copied blocks had a blank
    ///   line between two of them.
    /// - [`InsertOptions::heading_shift`] is added to each copied heading's level.
    ///
    /// Frontmatter is never copied. This takes time linear in the copied content, plus one
    /// lookup in each document's index for each reference and footnote call, and, for each
    /// relabelled definition, lookups logarithmic in the number of its identifier's relabelled
    /// copies already here. Into a detached subtree, which has no index, the first footnote
    /// definition copied costs one pass over the subtree, as for the lookups of #226. Inserting
    /// into phrasing walks the parent's phrasing ancestors, in time linear in the depth of the
    /// phrasing nest (#233).
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
    /// definition whose identifier is bound here when [`InsertOptions::footnotes`] is
    /// [`FootnotePolicy::Reject`], and a fresh identifier, label or call too long to be one.
    /// Nothing is changed when an error is returned.
    pub fn insert_from<T: AsRef<str>>(
        &mut self,
        parent: NodeId,
        after: Option<NodeId>,
        source: &Document<T>,
        nodes: impl IntoIterator<Item = NodeId>,
        options: &InsertOptions,
    ) -> Result<Inserted, StreamError> {
        self.edit(|document| {
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
            let plan = document.plan(parent, next, source, nodes, options, &[])?;
            Ok(document.apply(&plan, parent, after, next, source))
        })
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

    // Plan copying `nodes` of `source` into `parent` before `next`, checking everything that can
    // fail. `leaving` holds the sorted definitions of this document that the insert replaces,
    // which no longer bind.
    pub(super) fn plan<T: AsRef<str>>(
        &self,
        parent: NodeId,
        next: Option<NodeId>,
        source: &Document<T>,
        nodes: impl IntoIterator<Item = NodeId>,
        options: &InsertOptions,
        leaving: &[NodeId],
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
        let mut top = prune(source, selected, false);
        if nested && next.is_none() {
            while top.last().is_some_and(|&last| source.separator(last)) {
                top.pop();
            }
        }
        let tight = nested && self.tight_item(parent);
        if tight {
            // Two paragraphs are apart only with a separator between them, as in a loose list.
            let before_paragraph = |at: usize| {
                top[at + 1..]
                    .iter()
                    .find(|&&id| !source.separator(id))
                    .map_or_else(
                        || {
                            core::iter::successors(next, |id| self.nodes[id.index()].next_sibling)
                                .find(|&id| !self.separator(id))
                                .is_some_and(|id| {
                                    matches!(
                                        self.nodes[id.index()].kind,
                                        EventKind::Start(Tag::Paragraph { .. })
                                    )
                                })
                        },
                        |&id| {
                            matches!(
                                source.nodes[id.index()].kind,
                                EventKind::Start(Tag::Paragraph { .. })
                            )
                        },
                    )
            };
            let mut kept = Vec::with_capacity(top.len());
            let mut after_paragraph = false;
            for (at, &id) in top.iter().enumerate() {
                let separator = source.separator(id);
                if !(separator && after_paragraph && !before_paragraph(at)) {
                    kept.push(id);
                }
                after_paragraph = matches!(
                    source.nodes[id.index()].kind,
                    EventKind::Start(Tag::Paragraph { .. })
                );
            }
            top = kept;
        }
        // The content model: each copy against the parent, whose first child is its own or the
        // first copy.
        let first = self.nodes[parent.index()]
            .first_child
            .map(|id| &self.nodes[id.index()].kind)
            .or_else(|| top.first().map(|id| &source.nodes[id.index()].kind));
        if top
            .iter()
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
        each_kept(source, top.clone(), |id| {
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
        let mut clashes = Vec::new();
        let mut used = BTreeSet::new();
        for (index, (definition, _)) in definitions.iter().enumerate() {
            let key = identifier(*definition);
            if binds(key) {
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
                used.contains(candidate) || binds(candidate)
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
                for &id in top.iter().filter(|&&id| !source.separator(id)) {
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
        for &id in &plan.top {
            let copy = self.copy_kept(source, plan, id, parent, previous, next);
            if attached {
                self.index
                    .insert_tree(self.source.as_ref(), &self.nodes, copy);
            }
            inserted.nodes.push(copy);
            previous = Some(copy);
        }
        if let Some((container, anchor)) = plan.definitions_in {
            let mut previous = anchor.or(previous);
            let next = previous.map_or(self.nodes[container.index()].first_child, |previous| {
                self.nodes[previous.index()].next_sibling
            });
            for &(id, _) in &plan.definitions {
                let copy = self.copy_kept(source, plan, id, container, previous, next);
                if attached {
                    self.index
                        .insert_tree(self.source.as_ref(), &self.nodes, copy);
                }
                inserted.footnote_definitions.push(copy);
                previous = Some(copy);
            }
            self.refresh_item(Some(container));
        }
        if let Some(spread) = plan.spread {
            self.syntax
                .insert(parent, Some(NodeSyntax::ListItem { spread }));
        }
        self.refresh_item(Some(parent));
        inserted
    }

    // Copy the subtree at `root` of `source` and its kept children into `parent`, after
    // `previous` and before `next`, as `plan` copies it.
    fn copy_kept<T: AsRef<str>>(
        &mut self,
        source: &Document<T>,
        plan: &Plan,
        root: NodeId,
        parent: NodeId,
        previous: Option<NodeId>,
        next: Option<NodeId>,
    ) -> NodeId {
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
        self.mark_added();
        self.refresh_context(copied_root);
        copied_root
    }
}
