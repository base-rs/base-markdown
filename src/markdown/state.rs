// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024-2025, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

//! State.
//!
//! JS equivalent: https://github.com/syntax-tree/mdast-util-to-markdown/blob/fd6a508/lib/types.js#L195.
use crate::markdown::{
    attention::{self, Edge, Surroundings},
    construct_name::ConstructName,
    util::{
        format_code_as_indented::format_code_as_indented,
        format_heading_as_setext::format_heading_as_setext,
        format_link_as_auto_link::auto_link_text, safe::EscapeInfos,
    },
    Options,
};
use crate::{
    message::Message,
    stream::{EventKind, Tag},
    util::{
        line_ending::{line_endings, trim_terminal_ending},
        normalize_identifier::{normalize_identifier, normalize_identifier_into},
    },
    AlignKind, Document, NodeId, NodeSyntax,
};
use alloc::{borrow::Cow, collections::BTreeMap, string::String, vec::Vec};

pub struct Info<'a> {
    pub after: &'a str,
    before: Before<'a>,
}

enum Before<'a> {
    Literal(&'a str),
    Output(core::ops::Range<usize>),
}

pub struct State<'a> {
    pub output: String,
    pub(super) scratch: String,
    escapes: Vec<(usize, EscapeInfos)>,
    /// How many attention nodes are open, by marker (`*`, `_`) and sequence size (1, 2).
    attention_open: [[usize; 2]; 2],
    /// The size of the run of markers the last opening sequence ends, including enclosing opening sequences it joins.
    opening_run: usize,
    pub bullet_current: Option<char>,
    pub bullet_last_used: Option<char>,
    pub index_stack: Vec<usize>,
    pub options: &'a Options,
    pub stack: Vec<ConstructName>,
    pub serialization_root: Option<NodeId>,
    definitions: Option<BTreeMap<String, NodeId>>,
    reference_key: String,
}

impl<'a> Info<'a> {
    pub fn new(before: &'a str, after: &'a str) -> Self {
        Self {
            before: Before::Literal(before),
            after,
        }
    }
    pub fn at(start: usize, end: usize, after: &'a str) -> Self {
        Self {
            before: Before::Output(start..end),
            after,
        }
    }
    pub fn before<'o>(&'o self, output: &'o str) -> &'o str {
        match &self.before {
            Before::Literal(value) => value,
            Before::Output(range) => &output[range.clone()],
        }
    }
}

impl<'a> State<'a> {
    fn between<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        left: NodeId,
        right: NodeId,
        parent: NodeId,
    ) {
        let separator = if self.options.tight_definitions {
            self.definition_separator(document, left, right)
        } else {
            self.default_separator(document, left, right, parent)
        };
        self.output.push_str(separator);
    }

    pub fn container_flow<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        parent: NodeId,
    ) -> Result<(), Message> {
        let mut children = flow_children(document, parent).peekable();
        self.index_stack.push(0);
        for index in 0.. {
            let Some(child) = children.next() else { break };
            *self.index_stack.last_mut().unwrap() = index;
            if !matches!(
                document.node(child).unwrap().kind,
                EventKind::Start(Tag::List { .. })
            ) {
                self.bullet_last_used = None;
            }
            self.write_node(document, child, &Info::new("\n", "\n"))?;
            if let Some(&next) = children.peek() {
                self.between(document, child, next, parent);
            }
        }
        self.index_stack.pop();
        Ok(())
    }

    pub fn container_phrasing<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        parent: NodeId,
        info: &Info,
    ) -> Result<(), Message> {
        let children: Vec<NodeId> = flow_children(document, parent).collect();
        self.phrasing(document, &children, info, &Edge::default())?;
        Ok(())
    }

    fn attention_sequences(
        &self,
        kind: &EventKind<'_>,
    ) -> Result<Option<&'static [&'static str]>, Message> {
        Ok(match kind {
            EventKind::Start(Tag::Emphasis) => Some(if self.options.check_emphasis()? == '*' {
                attention::EMPHASIS_ASTERISK
            } else {
                attention::EMPHASIS_UNDERSCORE
            }),
            EventKind::Start(Tag::Strong) => Some(if self.options.check_strong()? == '*' {
                attention::STRONG_ASTERISK
            } else {
                attention::STRONG_UNDERSCORE
            }),
            EventKind::Start(Tag::Delete) => Some(attention::STRIKETHROUGH),
            _ => None,
        })
    }

    /// Write adjacent attention of one kind as a single node, returning the edits for the text after it.
    ///
    /// Text written directly before may be edited so the opening sequence forms.
    fn attention<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        members: &[NodeId],
        sequences: &'static [&'static str],
        info: &Info,
        previous: &Previous,
        last_child: bool,
    ) -> Result<Edge, Message> {
        let children: Vec<NodeId> = members
            .iter()
            .flat_map(|&member| flow_children(document, member))
            .collect();
        let before = info.before(&self.output).chars().next_back();
        let first = children
            .first()
            .and_then(|&child| edge_character(document, child, true));
        let last = children
            .last()
            .and_then(|&child| edge_character(document, child, false));
        let after = info.after.chars().next();
        let choice = attention::choose(
            sequences,
            &Surroundings {
                before,
                before_closes: matches!(previous, Previous::Attention),
                before_opens: matches!(previous, Previous::Start),
                opening_run: self.opening_run,
                first,
                first_size: children.first().map_or(0, |&child| {
                    match document.node(child).unwrap().kind {
                        EventKind::Start(Tag::Emphasis) => 1,
                        EventKind::Start(Tag::Strong) => 2,
                        _ => 0,
                    }
                }),
                last,
                after,
                after_closes: last_child,
                enclosing: [
                    [self.attention_open[0][0] > 0, self.attention_open[0][1] > 0],
                    [self.attention_open[1][0] > 0, self.attention_open[1][1] > 0],
                ],
            },
        );
        let sequence = choice.sequence;
        let tilde = sequence.starts_with('~');
        if let (Previous::Text(start), Some(character)) = (previous, before) {
            let start = *start;
            if choice.encode_before && !matches!(character, '\n' | '\r') {
                attention::encode_end(&mut self.output, start, character);
            }
            if tilde {
                attention::escape_tilde_end(&mut self.output, start);
            }
        }
        let joins = matches!(previous, Previous::Start) && before == sequence.chars().next();
        self.opening_run = sequence.len() + if joins { self.opening_run } else { 0 };
        self.output.push_str(sequence);
        // Strikethrough has one sequence, so enclosing strikethrough is not tracked.
        let open = (!tilde).then(|| usize::from(sequence.starts_with('_')));
        if let Some(open) = open {
            self.attention_open[open][sequence.len() - 1] += 1;
        }
        let edge = Edge {
            encode: first.filter(|_| choice.encode_first),
            tilde: true,
        };
        let last_text =
            self.phrasing(document, &children, &Info::new(sequence, sequence), &edge)?;
        if let Some(open) = open {
            self.attention_open[open][sequence.len() - 1] -= 1;
        }
        if let Some(start) = last_text {
            attention::escape_tilde_end(&mut self.output, start);
            if let Some(character) = last.filter(|_| choice.encode_last) {
                attention::encode_end(&mut self.output, start, character);
            }
        }
        self.output.push_str(sequence);
        Ok(Edge {
            encode: after.filter(|_| choice.encode_after),
            tilde,
        })
    }

    /// Serialize phrasing children, returning where the last one begins if it is text.
    ///
    /// `edge` edits the first child if it is text.
    fn phrasing<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        children: &[NodeId],
        info: &Info,
        edge: &Edge,
    ) -> Result<Option<usize>, Message> {
        let start = self.output.len();
        self.index_stack.push(0);
        let mut index = 0;
        let mut position = 0;
        let mut after_hard_break = false;
        // Whether the output ends with a hard break's line ending, past any skipped soft break.
        let mut hard_break_line = false;
        let mut pending = Edge {
            encode: edge.encode,
            tilde: edge.tilde,
        };
        let mut previous = Previous::Start;
        while position < children.len() {
            let child = children[position];
            let kind = &document.node(child).unwrap().kind;
            if !is_phrasing(kind) {
                return Err(unsupported_content());
            }
            // Adjacent attention of the same kind is written as one node.
            let mut end = position + 1;
            let sequences = self.attention_sequences(kind)?;
            if sequences.is_some() {
                while children
                    .get(end)
                    .is_some_and(|&next| same_attention(kind, &document.node(next).unwrap().kind))
                {
                    end += 1;
                }
            }
            let members = &children[position..end];
            position = end;
            if after_hard_break && matches!(kind, EventKind::SoftBreak(_)) {
                after_hard_break = false;
                continue;
            }
            after_hard_break = matches!(kind, EventKind::HardBreak);
            *self.index_stack.last_mut().unwrap() = index;
            let mut buffer = [0u8; 4];
            let next = children.get(end).copied();
            let after = if let Some(next) = next {
                if let Some(first) = self.peek_node(document, next) {
                    first.encode_utf8(&mut buffer) as &str
                } else {
                    // Render nodes without a fixed prefix to determine the next character.
                    let checkpoint = self.output.len();
                    self.write_node(document, next, &Info::new("", ""))?;
                    let first = self.output[checkpoint..].chars().next().unwrap_or_default();
                    self.output.truncate(checkpoint);
                    first.encode_utf8(&mut buffer)
                }
            } else {
                info.after
            };
            let before = if self.output.len() == start {
                match &info.before {
                    Before::Literal(value) => Before::Literal(value),
                    Before::Output(range) => Before::Output(range.clone()),
                }
            } else if self.output.ends_with(['\n', '\r'])
                && matches!(kind, EventKind::Html { value, .. }
                if crate::block_parser::html_interrupts_paragraph(
                    value.resolve(document.source()).unwrap().as_bytes()
                ))
            {
                if hard_break_line {
                    // Indent the line so it continues the paragraph and keeps the break.
                    self.output.push_str("    ");
                } else {
                    // Replace the line ending before inline HTML so it cannot start an HTML block.
                    self.output.pop();
                    if self.output.ends_with('\r') {
                        self.output.pop();
                    }
                    self.output.push(' ');
                }
                Before::Literal(" ")
            } else {
                Before::Output(start..self.output.len())
            };
            let length = self.output.len();
            let info = Info { before, after };
            if let Some(sequences) = sequences {
                pending = self.attention(
                    document,
                    members,
                    sequences,
                    &info,
                    &previous,
                    next.is_none(),
                )?;
                previous = Previous::Attention;
            } else {
                self.write_node(document, child, &info)?;
                if matches!(kind, EventKind::Text(_)) && self.output.len() != length {
                    // Text split by the parser is edited as one, so its edges stay the same when serialized again.
                    if !attention::edit_start(&mut self.output, length, &pending) {
                        pending = Edge::default();
                    }
                    if !matches!(previous, Previous::Text(_)) {
                        previous = Previous::Text(length);
                    }
                } else if self.output.len() != length {
                    pending = Edge::default();
                    previous = Previous::Other;
                }
            }
            if self.output.len() != length {
                hard_break_line = after_hard_break;
            }
            index += 1;
        }
        self.index_stack.pop();
        Ok(match previous {
            Previous::Text(start) => Some(start),
            _ => None,
        })
    }

    pub fn enter(&mut self, name: ConstructName) {
        self.stack.push(name);
    }

    pub fn exit(&mut self) {
        self.stack.pop();
    }

    pub fn handle<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        id: NodeId,
        info: &Info,
    ) -> Result<String, Message> {
        self.write_node(document, id, info)?;
        Ok(core::mem::take(&mut self.output))
    }

    fn write_node<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        id: NodeId,
        info: &Info,
    ) -> Result<(), Message> {
        let node = document
            .node(id)
            .expect("serialization uses existing arena nodes");
        let source = document.source();
        match &node.kind {
            EventKind::Start(tag) => match tag {
                Tag::Document(_) => {
                    if flow_children(document, id)
                        .all(|child| is_phrasing(&document.node(child).unwrap().kind))
                    {
                        self.container_phrasing(document, id, info)
                    } else {
                        self.container_flow(document, id)
                    }
                }
                Tag::Paragraph { .. } => self.paragraph(document, id, info),
                Tag::Heading(level) => self.heading(document, id, *level),
                Tag::BlockQuote => self.blockquote(document, id),
                Tag::List { start, .. } => self.list(document, id, start.is_some()),
                Tag::Item { .. } => self.list_item(document, id),
                Tag::Emphasis | Tag::Strong | Tag::Delete => {
                    // Attention outside a phrasing container is written on its own.
                    self.enter(ConstructName::Phrasing);
                    self.phrasing(document, &[id], info, &Edge::default())?;
                    self.exit();
                    Ok(())
                }
                Tag::Link { destination, title } | Tag::Image { destination, title } => {
                    let image = matches!(tag, Tag::Image { .. });
                    let destination = destination.resolve(source).unwrap();
                    let title = title.as_ref().map(|value| value.resolve(source).unwrap());
                    if let Some(NodeSyntax::Reference { kind, label }) = document.syntax(id) {
                        let label = label.resolve(source).unwrap();
                        if self.reference_matches(document, id, label, destination, title) {
                            return self.reference(document, id, kind, label, image);
                        }
                    }
                    self.resource(document, id, destination, title, image)
                }
                Tag::Table => self.render_table(document, id),
                Tag::TableRow => self.render_table_row(document, id),
                Tag::TableCell { .. } => self.container_phrasing(document, id, info),
                Tag::TableHead | Tag::TableBody => {
                    self.render_table_rows(document, flow_children(document, id), false)
                }
                Tag::FootnoteDefinition(label) => {
                    let syntax = document.syntax(id);
                    let label = match &syntax {
                        Some(NodeSyntax::Footnote { label }) => label.resolve(source).unwrap(),
                        _ => label.resolve(source).unwrap(),
                    };
                    self.output.push_str("[^");
                    self.output.push_str(label);
                    self.output.push_str("]:");
                    if flow_children(document, id).next().is_some() {
                        self.output.push(' ');
                        let start = self.output.len();
                        self.container_flow(document, id)?;
                        self.indent_lines(start, |line, index, blank, output| {
                            if index > 0 && !blank {
                                output.push_str("    ");
                            }
                            output.push_str(line);
                        });
                    }
                    Ok(())
                }
            },
            EventKind::Text(text) => {
                let mut value = text.resolve(source).unwrap();
                if let Some(parent) = node.parent() {
                    let marker = document
                        .node(parent)
                        .unwrap()
                        .first_child()
                        .expect("an attached node belongs to a nonempty parent");
                    if document.node(marker).unwrap().next_sibling() == Some(id)
                        && self.is_item_task(document, marker)
                    {
                        value = value.trim_start_matches([' ', '\t']);
                    }
                }
                self.text(value, info)
            }
            EventKind::SoftBreak(text) => self.text(text.resolve(source).unwrap(), info),
            EventKind::HardBreak => self.hard_break(info),
            EventKind::Rule => self.thematic_break(),
            EventKind::Code { value, language } => self.inline_code(
                value.resolve(source).unwrap(),
                language
                    .as_ref()
                    .map(|language| language.resolve(source).unwrap()),
            ),
            EventKind::CodeBlock {
                value,
                language,
                math,
                ..
            } => {
                let value = trim_terminal_ending(value.resolve(source).unwrap());
                let language = language
                    .as_ref()
                    .map(|value| normalize_nul(value.resolve(source).unwrap()));
                let language = language.as_deref();
                let syntax = document.syntax(id);
                let authored_info = match &syntax {
                    Some(NodeSyntax::Code { info }) => {
                        Some(normalize_nul(info.resolve(source).unwrap()))
                    }
                    _ => None,
                };
                let authored_info = authored_info.as_deref();
                if *math {
                    self.math(
                        value,
                        authored_info.or(language).filter(|value| !value.is_empty()),
                    )
                } else {
                    let (language, meta) = match (authored_info, language) {
                        (Some(info), Some(language)) => {
                            let metadata = info
                                .strip_prefix(language)
                                .unwrap_or("")
                                .trim_matches([' ', '\t']);
                            (Some(language), (!metadata.is_empty()).then_some(metadata))
                        }
                        _ => (language, None),
                    };
                    self.code(value, language, meta)
                }
            }
            EventKind::Math(value) => self.inline_math(value.resolve(source).unwrap()),
            EventKind::Html { value, .. } => {
                self.output.push_str(value.resolve(source).unwrap());
                Ok(())
            }
            EventKind::TaskMarker(checked) => {
                if !self.is_item_task(document, id) {
                    self.output.push_str(if *checked { "[x]" } else { "[ ]" });
                }
                Ok(())
            }
            EventKind::Definition {
                label,
                destination,
                title,
            } => self.definition(
                label.resolve(source).unwrap(),
                destination.resolve(source).unwrap(),
                title.as_ref().map(|value| value.resolve(source).unwrap()),
            ),
            EventKind::Opaque { value, .. } => {
                if let Some(NodeSyntax::Frontmatter {
                    info,
                    fence,
                    closing_fence_length,
                }) = document.syntax(id)
                {
                    self.frontmatter(
                        document
                            .frontmatter_value(id)
                            .expect("frontmatter syntax identifies a frontmatter body"),
                        info.resolve(source).unwrap(),
                        fence,
                        closing_fence_length,
                    )
                } else {
                    {
                        self.output.push_str(value.resolve(source).unwrap());
                        Ok(())
                    }
                }
            }
            EventKind::FootnoteReference(label) => {
                let syntax = document.syntax(id);
                let label = match &syntax {
                    Some(NodeSyntax::Footnote { label }) => label.resolve(source).unwrap(),
                    _ => label.resolve(source).unwrap(),
                };
                self.output.push_str("[^");
                self.output.push_str(label);
                self.output.push(']');
                Ok(())
            }
            EventKind::Separator { .. } => Ok(()),
            EventKind::End(_) => unreachable!("arena nodes store starts and leaves only"),
        }
    }

    fn render_table<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        id: NodeId,
    ) -> Result<(), Message> {
        let rows = flow_children(document, id).flat_map(|child| {
            let group = matches!(
                document.node(child).unwrap().kind,
                EventKind::Start(Tag::TableHead | Tag::TableBody)
            );
            (!group)
                .then_some(child)
                .into_iter()
                .chain(flow_children(document, child).take(if group { usize::MAX } else { 0 }))
        });
        self.render_table_rows(document, rows, true)
    }

    fn render_table_rows<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        rows: impl Iterator<Item = NodeId>,
        header: bool,
    ) -> Result<(), Message> {
        for (index, row) in rows.enumerate() {
            if !matches!(
                document.node(row).unwrap().kind,
                EventKind::Start(Tag::TableRow)
            ) {
                return Err(unsupported_content());
            }
            if index > 0 {
                self.output.push('\n');
            }
            self.render_table_row(document, row)?;
            if header && index == 0 {
                self.output.push_str("\n| ");
                for (index, cell) in flow_children(document, row).enumerate() {
                    if index > 0 {
                        self.output.push_str(" | ");
                    }
                    let EventKind::Start(Tag::TableCell { alignment, .. }) =
                        document.node(cell).unwrap().kind
                    else {
                        unreachable!("rendered table rows contain only cells")
                    };
                    self.output.push_str(match alignment {
                        AlignKind::Left => ":---",
                        AlignKind::Right => "---:",
                        AlignKind::Center => ":---:",
                        AlignKind::None => "---",
                    });
                }
                self.output.push_str(" |");
            }
        }
        Ok(())
    }

    fn render_table_row<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        row: NodeId,
    ) -> Result<(), Message> {
        self.output.push_str("| ");
        for (index, cell) in flow_children(document, row).enumerate() {
            if !matches!(
                document.node(cell).unwrap().kind,
                EventKind::Start(Tag::TableCell { .. })
            ) {
                return Err(unsupported_content());
            }
            if index > 0 {
                self.output.push_str(" | ");
            }
            let start = self.output.len();
            self.container_phrasing(document, cell, &Info::new("", ""))?;
            if self.output[start..].contains('|') {
                self.rewrite(start, |value, output| write_table_pipes(value, output));
            }
        }
        self.output.push_str(" |");
        Ok(())
    }

    // Transform only the newly written range; nested formatting reuses this buffer after its children finish.
    pub(super) fn rewrite(
        &mut self,
        start: usize,
        transform: impl FnOnce(&mut String, &mut String),
    ) {
        let mut scratch = core::mem::take(&mut self.scratch);
        scratch.clear();
        scratch.push_str(&self.output[start..]);
        self.output.truncate(start);
        transform(&mut scratch, &mut self.output);
        self.scratch = scratch;
    }

    pub fn indent_lines(&mut self, start: usize, map: impl Fn(&str, usize, bool, &mut String)) {
        self.rewrite(start, |value, output| {
            let mut start = 0;
            let mut line = 0;
            for ending in line_endings(value) {
                let slice = &value[start..ending.start];
                map(slice, line, slice.is_empty(), output);
                output.push_str(&value[ending.clone()]);
                start = ending.end;
                line += 1;
            }
            map(&value[start..], line, value.is_empty(), output);
        });
    }

    fn default_separator<S: AsRef<str>>(
        &self,
        document: &Document<S>,
        left: NodeId,
        right: NodeId,
        parent: NodeId,
    ) -> &'static str {
        let left_kind = &document.node(left).unwrap().kind;
        let right_kind = &document.node(right).unwrap().kind;
        if self.indented_code(document, right)
            && (matches!(left_kind, EventKind::Start(Tag::List { .. }))
                || self.indented_code(document, left))
        {
            return "\n\n<!---->\n\n";
        }
        if matches!(
            document.node(parent).unwrap().kind,
            EventKind::Start(Tag::Item { .. } | Tag::List { .. })
        ) {
            // Without a blank line, a block that cannot interrupt a paragraph would lazily continue the nested list's last paragraph.
            if matches!(left_kind, EventKind::Start(Tag::List { .. }))
                && (matches!(
                    right_kind,
                    EventKind::Start(Tag::Paragraph { .. } | Tag::Table)
                        | EventKind::Definition { .. }
                ) || matches!(right_kind, EventKind::Start(Tag::Heading(_)))
                    && format_heading_as_setext(document, right, self))
            {
                return "\n\n";
            }
            if matches!(left_kind, EventKind::Start(Tag::Paragraph { .. })) {
                if matches!(
                    right_kind,
                    EventKind::Start(Tag::Paragraph { .. }) | EventKind::Definition { .. }
                ) {
                    return "\n\n";
                }
                if matches!(right_kind, EventKind::Start(Tag::Heading(_)))
                    && format_heading_as_setext(document, right, self)
                {
                    return "\n\n";
                }
            }
            let spread = match document.node(parent).unwrap().kind {
                EventKind::Start(Tag::List { tight, .. }) => !tight,
                _ => self.item_spread(document, parent),
            };
            return if spread { "\n\n" } else { "\n" };
        }
        "\n\n"
    }

    fn indented_code<S: AsRef<str>>(&self, document: &Document<S>, id: NodeId) -> bool {
        if let EventKind::CodeBlock {
            value,
            language,
            math: false,
            ..
        } = &document.node(id).unwrap().kind
        {
            format_code_as_indented(
                trim_terminal_ending(value.resolve(document.source()).unwrap()),
                language.is_none(),
                self,
            )
        } else {
            false
        }
    }

    /// No JS equivalent.
    pub fn new(options: &'a Options) -> Self {
        State {
            output: String::new(),
            scratch: String::new(),
            escapes: Vec::new(),
            attention_open: [[0; 2]; 2],
            opening_run: 0,
            bullet_current: None,
            bullet_last_used: None,
            index_stack: Vec::new(),
            options,
            stack: Vec::new(),
            serialization_root: None,
            definitions: None,
            reference_key: String::new(),
        }
    }

    /// A parsed reference is safe only while its first definition still resolves to
    /// the stored semantic destination and title. Explicit synthetic references may
    /// intentionally stand alone. Index only the serialized subtree, once on demand.
    fn reference_matches<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        id: NodeId,
        label: &str,
        destination: &str,
        title: Option<&str>,
    ) -> bool {
        let standalone = document.node(id).unwrap().span.is_empty();
        let source = document.source();
        let root = self.serialization_root.unwrap_or(document.root());
        let definitions = self.definitions.get_or_insert_with(|| {
            let mut definitions = BTreeMap::new();
            for id in document.descendants(root) {
                if let EventKind::Definition { label, .. } = &document.node(id).unwrap().kind {
                    definitions
                        .entry(normalize_identifier(label.resolve(source).unwrap()))
                        .or_insert(id);
                }
            }
            definitions
        });
        normalize_identifier_into(label, &mut self.reference_key);
        let Some(definition) = definitions.get(&self.reference_key) else {
            return standalone;
        };
        let EventKind::Definition {
            destination: target,
            title: target_title,
            ..
        } = &document.node(*definition).unwrap().kind
        else {
            unreachable!("definition index stores only definition nodes")
        };
        target.resolve(source).unwrap() == destination
            && target_title
                .as_ref()
                .map(|value| value.resolve(source).unwrap())
                == title
    }

    fn peek_node<S: AsRef<str>>(&mut self, document: &Document<S>, id: NodeId) -> Option<char> {
        match &document.node(id).unwrap().kind {
            EventKind::Text(value) | EventKind::SoftBreak(value) => {
                super::escape::literal_prefix(value.resolve(document.source()).unwrap())
            }
            EventKind::Start(Tag::Emphasis) => Some(self.options.emphasis),
            EventKind::Start(Tag::Strong) => Some(self.options.strong),
            EventKind::Start(Tag::Delete) => Some('~'),
            EventKind::Start(Tag::Image { .. }) => Some('!'),
            EventKind::Start(Tag::Link { destination, title }) => Some(
                if matches!(document.syntax(id), Some(NodeSyntax::Reference { label, .. })
                    if self.reference_matches(document, id, label.resolve(document.source()).unwrap(),
                        destination.resolve(document.source()).unwrap(),
                        title.as_ref().map(|value| value.resolve(document.source()).unwrap())))
                {
                    '['
                } else if auto_link_text(
                    document,
                    id,
                    destination.resolve(document.source()).unwrap(),
                    title.is_some(),
                    self,
                )
                .is_some()
                {
                    '>'
                } else {
                    '['
                },
            ),
            EventKind::Html { .. } => Some('<'),
            EventKind::Code { .. } => Some('`'),
            EventKind::Math(_) => Some('$'),
            _ => None,
        }
    }

    pub fn write_safe(&mut self, input: &str, info: &Info, encode: Option<char>) {
        let before = info.before(&self.output);
        let start = before.len();
        let end = start + input.len();
        self.escapes.clear();
        super::escape::collect(
            before,
            input,
            info.after,
            &self.stack,
            self.options.single_dollar_text_math,
            &mut self.escapes,
        );
        self.escapes.sort_unstable_by_key(|entry| entry.0);
        self.escapes.dedup_by(|right, left| {
            if right.0 == left.0 {
                left.1.before &= right.1.before;
                left.1.after &= right.1.after;
                true
            } else {
                false
            }
        });
        let mut cursor = start;
        let mut skip_until = 0;
        let before_last = before.chars().next_back();
        // The output before can be long, so it is only scanned for underscores that may stay unescaped.
        let before_email = self
            .escapes
            .iter()
            .any(|&(position, _)| {
                (start..end).contains(&position) && input.as_bytes()[position - start] == b'_'
            })
            .then(|| email_domain(before.as_bytes().iter().rev()))
            .flatten();
        for (index, &(position, ref flags)) in self.escapes.iter().enumerate() {
            if position < start || position >= end || position < skip_until {
                continue;
            }
            if let Some(run_end) = intraword_underscores(
                &self.escapes,
                index,
                start,
                (before_last, before_email),
                input,
                info.after,
            ) {
                skip_until = run_end;
                continue;
            }
            if self.escapes.get(index + 1).is_some_and(|next| {
                position + 1 < end
                    && next.0 == position + 1
                    && flags.after
                    && !next.1.before
                    && !next.1.after
            }) || index > 0
                && self.escapes[index - 1].0 + 1 == position
                && flags.before
                && !self.escapes[index - 1].1.before
                && !self.escapes[index - 1].1.after
            {
                continue;
            }
            super::util::safe::write_backslashes(
                &input[cursor - start..position - start],
                "\\",
                &mut self.output,
            );
            cursor = position;
            let character = input[position - start..]
                .chars()
                .next()
                .expect("escape positions are character boundaries");
            if character.is_ascii_punctuation() && encode != Some(character) {
                self.output.push('\\');
            } else {
                use core::fmt::Write as _;
                write!(self.output, "&#x{:X};", u32::from(character))
                    .expect("String writing is infallible");
                cursor += character.len_utf8();
            }
        }
        super::util::safe::write_backslashes(
            &input[cursor - start..],
            info.after,
            &mut self.output,
        );
    }

    fn definition_separator<S: AsRef<str>>(
        &self,
        document: &Document<S>,
        left: NodeId,
        right: NodeId,
    ) -> &'static str {
        if matches!(
            document.node(left).unwrap().kind,
            EventKind::Definition { .. }
        ) && matches!(
            document.node(right).unwrap().kind,
            EventKind::Definition { .. }
        ) {
            "\n"
        } else {
            "\n\n"
        }
    }
}

/// What a phrasing child was written after.
enum Previous {
    /// Attention, whose closing sequence ends the output.
    Attention,
    /// Text, which begins at this output offset.
    Text(usize),
    /// Nothing: the child starts its parent.
    Start,
    /// Anything else.
    Other,
}

/// Whether two nodes are attention of the same kind, which would form one run if adjacent.
fn same_attention(left: &EventKind<'_>, right: &EventKind<'_>) -> bool {
    matches!(
        (left, right),
        (
            EventKind::Start(Tag::Emphasis),
            EventKind::Start(Tag::Emphasis)
        ) | (EventKind::Start(Tag::Strong), EventKind::Start(Tag::Strong))
            | (EventKind::Start(Tag::Delete), EventKind::Start(Tag::Delete))
    )
}

/// The first or last character a phrasing node is written with, as far as attention next to it needs: text is approximated by its value and other nodes by punctuation.
fn edge_character<S: AsRef<str>>(document: &Document<S>, id: NodeId, first: bool) -> Option<char> {
    match &document.node(id).unwrap().kind {
        EventKind::Text(value) => {
            let value = value.resolve(document.source()).unwrap();
            if first {
                value.chars().next()
            } else {
                value.chars().next_back()
            }
        }
        // A hard break ends with a line ending, and starts with a backslash, punctuation like other nodes. Parsed attention cannot start or end with a soft break, as its sequences would not flank.
        EventKind::HardBreak if !first => Some('\n'),
        _ => Some('.'),
    }
}

pub(crate) fn is_phrasing(kind: &EventKind<'_>) -> bool {
    matches!(
        kind,
        EventKind::Start(
            Tag::Delete | Tag::Emphasis | Tag::Image { .. } | Tag::Link { .. } | Tag::Strong
        ) | EventKind::HardBreak
            | EventKind::SoftBreak(_)
            | EventKind::FootnoteReference(_)
            | EventKind::Code { .. }
            | EventKind::Math(_)
            | EventKind::TaskMarker(_)
            | EventKind::Text(_)
            | EventKind::Html { block: false, .. }
    )
}

pub(super) fn flow_children<S: AsRef<str>>(
    document: &Document<S>,
    id: NodeId,
) -> impl Iterator<Item = NodeId> + Clone + '_ {
    document.children(id).filter(move |child| {
        !matches!(
            document.node(*child).unwrap().kind,
            EventKind::Separator { .. }
        )
    })
}

fn normalize_nul(value: &str) -> Cow<'_, str> {
    if value.contains('\0') {
        Cow::Owned(value.replace('\0', "�"))
    } else {
        Cow::Borrowed(value)
    }
}

pub(super) fn unsupported_content() -> Message {
    Message {
        span: None,
        reason: "This content shape cannot be represented in Markdown".into(),
        rule_id: "unsupported-node",
        source: "markdown",
    }
}

pub(super) fn invalid_node() -> Message {
    Message {
        span: None,
        reason: "Node does not belong to the document".into(),
        rule_id: "invalid-node",
        source: "markdown",
    }
}

fn write_table_pipes(value: &str, result: &mut String) {
    let bytes = value.as_bytes();
    let mut start = 0;
    let mut index = 0;
    let mut code_ticks = None;

    while let Some(offset) = memchr::memchr2(b'`', b'|', &bytes[index..]) {
        index += offset;
        if bytes[index] == b'`' {
            let marker_start = index;
            index = super::util::scan::marker_run_end(bytes, index + 1, b'`');
            let ticks = index - marker_start;
            if code_ticks == Some(ticks) {
                code_ticks = None;
            } else if code_ticks.is_none() && has_matching_ticks(bytes, index, ticks) {
                code_ticks = Some(ticks);
            }
        } else {
            if code_ticks.is_none() {
                let slashes = bytes[..index]
                    .iter()
                    .rev()
                    .take_while(|&&byte| byte == b'\\')
                    .count();
                if slashes % 2 == 0 {
                    result.push_str(&value[start..index]);
                    result.push('\\');
                    start = index;
                }
            }
            index += 1;
        }
    }
    result.push_str(&value[start..]);
}

#[cfg(test)]
fn escape_table_pipes(value: &str) -> String {
    let mut output = String::new();
    write_table_pipes(value, &mut output);
    output
}

fn has_matching_ticks(bytes: &[u8], index: usize, ticks: usize) -> bool {
    super::util::scan::marker_runs(&bytes[index..], b'`').any(|run| run.len() == ticks)
}

#[cfg(test)]
#[path = "state/tests.rs"]
mod tests;

/// Returns the end of a run of underscores between word characters (`a_b`, `a__b`).
/// Such runs cannot form attention, so they stay unescaped, as in `mdast-util-to-markdown`.
fn intraword_underscores(
    escapes: &[(usize, EscapeInfos)],
    index: usize,
    start: usize,
    (before_last, before_email): (Option<char>, Option<bool>),
    input: &str,
    after: &str,
) -> Option<usize> {
    use crate::util::char::{classify, Kind};
    let end = start + input.len();
    let position = escapes[index].0;
    if input.as_bytes()[position - start] != b'_' {
        return None;
    }
    let escaped = |at: usize| escapes.binary_search_by_key(&at, |entry| entry.0).is_ok();
    let previous = if position > start {
        input[..position - start].chars().next_back()
    } else {
        before_last
    }?;
    // Underscores next to characters that attention encodes as references are escaped by `attention::encode_end` and `attention::edit_start`.
    if classify(previous) != Kind::Other || escaped(position - previous.len_utf8()) {
        return None;
    }
    // GFM email autolinks allow `_` in domains, so an escape may be what keeps `a\@b_c.d` from linking.
    if email_domain(input.as_bytes()[..position - start].iter().rev())
        .or(before_email)
        .unwrap_or(false)
    {
        return None;
    }
    let run_end = position
        + input.as_bytes()[position - start..]
            .iter()
            .take_while(|&&byte| byte == b'_')
            .count();
    // A missing next character counts as whitespace, so the run stays escaped.
    let next = if run_end < end {
        input[run_end - start..].chars().next()
    } else {
        after.chars().next()
    }
    .unwrap_or(' ');
    let skip = run_end - position - 1;
    (escapes.get(index + skip).map(|entry| entry.0) == Some(run_end - 1)
        && classify(next) == Kind::Other
        && !escaped(run_end))
    .then_some(run_end)
}

/// Whether reversed text ends in a domain after `@`, or `None` when it is all domain characters.
fn email_domain<'a>(mut reversed: impl Iterator<Item = &'a u8>) -> Option<bool> {
    reversed
        .find(|byte| !(byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_')))
        .map(|&byte| byte == b'@')
}
