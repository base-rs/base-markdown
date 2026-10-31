// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024-2025, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

//! State.
//!
//! JS equivalent: https://github.com/syntax-tree/mdast-util-to-markdown/blob/fd6a508/lib/types.js#L195.
use crate::markdown::{
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
    /// Positions of `<` that need an escape only before an email autolink.
    email_candidates: Vec<usize>,
    /// Positions that receive an escape, in order.
    applied: Vec<usize>,
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
        let start = self.output.len();
        let mut children = flow_children(document, parent).peekable();
        self.index_stack.push(0);
        let mut index = 0;
        let mut after_hard_break = false;
        // Whether the output ends with a hard break's line ending, past any skipped soft break.
        let mut hard_break_line = false;
        while let Some(child) = children.next() {
            let kind = &document.node(child).unwrap().kind;
            if !is_phrasing(kind) {
                return Err(unsupported_content());
            }
            if after_hard_break && matches!(kind, EventKind::SoftBreak(_)) {
                after_hard_break = false;
                continue;
            }
            after_hard_break = matches!(kind, EventKind::HardBreak);
            *self.index_stack.last_mut().unwrap() = index;
            let mut buffer = [0u8; 4];
            let after = if let Some(&next) = children.peek() {
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
            self.write_node(document, child, &Info { before, after })?;
            if self.output.len() != length {
                hard_break_line = after_hard_break;
            }
            index += 1;
        }
        self.index_stack.pop();
        Ok(())
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
                Tag::Emphasis => {
                    let marker = self.options.check_emphasis()?;
                    self.delimited_phrasing(document, id, info, marker, 1)
                }
                Tag::Strong => {
                    let marker = self.options.check_strong()?;
                    self.delimited_phrasing(document, id, info, marker, 2)
                }
                Tag::Delete => self.delimited_phrasing(document, id, info, '~', 2),
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
            self.enter(ConstructName::TableCell);
            self.enter(ConstructName::Phrasing);
            self.container_phrasing(document, cell, &Info::new("", ""))?;
            self.exit();
            self.exit();
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
            email_candidates: Vec::new(),
            applied: Vec::new(),
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
                left.1.email &= right.1.email;
                true
            } else {
                false
            }
        });
        self.email_candidates.clear();
        let candidates = &mut self.email_candidates;
        self.escapes.retain(|&(position, ref flags)| {
            if flags.email {
                candidates.push(position);
            }
            !flags.email
        });
        self.applied.clear();
        let mut skip_until = 0;
        let before_last = before.chars().next_back();
        let before_email = email_domain(before.as_bytes().iter().rev());
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
            self.applied.push(position);
        }
        // Email-only escapes depend on which other escapes apply, so decide them afterwards.
        let applied = self.applied.len();
        for &position in &self.email_candidates {
            if position >= start
                && position < end
                && starts_email(
                    &input[position - start..],
                    position,
                    &self.applied[..applied],
                )
            {
                self.applied.push(position);
            }
        }
        if self.applied.len() > applied {
            self.applied.sort_unstable();
        }
        let mut cursor = start;
        for &position in &self.applied {
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

/// Whether the `<` that starts `rest`, at output `position`, opens an email autolink within this
/// text as written. An applied escape inserts a backslash, which ends the autolink, and the scan
/// stops at the next `<` because no autolink contains one.
fn starts_email(rest: &str, position: usize, applied: &[usize]) -> bool {
    let mut link = crate::util::autolink::Autolink::default();
    for (offset, &byte) in rest.as_bytes().iter().enumerate().skip(1) {
        if applied.binary_search(&(position + offset)).is_ok() {
            return false;
        }
        match link.advance(byte) {
            None => return false,
            Some(true) => return link.email(),
            Some(false) => {}
        }
    }
    false
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
    // Characters next to attention markers can become character references later.
    if classify(previous) != Kind::Other
        || escaped(position - previous.len_utf8())
        || position - previous.len_utf8() == start && matches!(before_last, Some('*' | '_'))
    {
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
        && !escaped(run_end)
        && !(run_end + next.len_utf8() == end && after.starts_with(['*', '_'])))
    .then_some(run_end)
}

/// Whether reversed text ends in a domain after `@`, or `None` when it is all domain characters.
fn email_domain<'a>(mut reversed: impl Iterator<Item = &'a u8>) -> Option<bool> {
    reversed
        .find(|byte| !(byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_')))
        .map(|&byte| byte == b'@')
}
