// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024-2025, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

//! State.
//!
//! JS equivalent: https://github.com/syntax-tree/mdast-util-to-markdown/blob/fd6a508/lib/types.js#L195.
use crate::markdown::{
    construct_name::ConstructName,
    r#unsafe::Unsafe,
    util::{
        format_code_as_indented::format_code_as_indented,
        format_heading_as_setext::format_heading_as_setext,
        format_link_as_auto_link::auto_link_text,
        pattern_in_scope::pattern_in_scope,
        safe::{escape_backslashes, EscapeInfos, SafeConfig},
    },
    Options,
};
use crate::{
    message::Message,
    stream::{EventKind, Tag},
    util::normalize_identifier::{normalize_identifier, normalize_identifier_into},
    AlignKind, Document, NodeId, NodeSyntax,
};
use alloc::{
    borrow::Cow,
    collections::BTreeMap,
    format,
    string::{String, ToString},
    vec::Vec,
};
use regex::Regex;

pub struct Info<'a> {
    pub after: &'a str,
    pub before: &'a str,
}

#[derive(Debug)]
/// Different ways to join two (container, flow) nodes.
enum Join {
    /// Join the two nodes with `1` blank line.
    Break,
    /// Join the two nodes with an HTML comment.
    HtmlComment,
    /// Join the two nodes with `d` blank lines.
    Lines(usize),
}

pub struct State<'a> {
    pub bullet_current: Option<char>,
    pub bullet_last_used: Option<char>,
    pub index_stack: Vec<usize>,
    pub options: &'a Options,
    pub stack: Vec<ConstructName>,
    pub r#unsafe: Vec<Unsafe<'a>>,
    pub serialization_root: Option<NodeId>,
    definitions: Option<BTreeMap<String, NodeId>>,
    reference_key: String,
}

impl<'a> Info<'a> {
    pub fn new(before: &'a str, after: &'a str) -> Self {
        Info { after, before }
    }
}

impl<'a> State<'a> {
    fn between<S: AsRef<str>>(
        &self,
        document: &Document<S>,
        left: NodeId,
        right: NodeId,
        parent: NodeId,
        results: &mut String,
    ) {
        let join = if self.options.tight_definitions {
            self.tight_definition(document, left, right)
        } else {
            self.join_defaults(document, left, right, parent)
        };
        Self::set_between(&join, results);
    }

    /// JS equivalent: <https://github.com/syntax-tree/mdast-util-to-markdown/blob/fd6a508/lib/util/compile-pattern.js>.
    pub fn compile_pattern(pattern: &mut Unsafe) {
        if pattern.compiled.is_none() {
            let mut pattern_to_compile = String::new();

            if let Some(pattern_before) = pattern.before {
                pattern_to_compile.push('(');
                if pattern.at_break {
                    pattern_to_compile.push_str("[\\r\\n][\\t ]*");
                }
                pattern_to_compile.push_str("(?:");
                pattern_to_compile.push_str(pattern_before);
                pattern_to_compile.push(')');
                pattern_to_compile.push(')');
            } else if pattern.at_break {
                pattern_to_compile.push('(');
                pattern_to_compile.push_str("[\\r\\n][\\t ]*");
                pattern_to_compile.push(')');
            }

            if matches!(
                pattern.character,
                '|' | '\\'
                    | '{'
                    | '}'
                    | '('
                    | ')'
                    | '['
                    | ']'
                    | '^'
                    | '$'
                    | '+'
                    | '*'
                    | '?'
                    | '.'
                    | '-'
            ) {
                pattern_to_compile.push('\\');
            }

            pattern_to_compile.push(pattern.character);

            if let Some(pattern_after) = pattern.after {
                pattern_to_compile.push_str("(?:");
                pattern_to_compile.push_str(pattern_after);
                pattern_to_compile.push(')');
            }

            pattern.set_compiled(
                Regex::new(&pattern_to_compile).expect("A valid unsafe regex pattern"),
            );
        }
    }

    pub fn container_flow<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        parent: NodeId,
    ) -> Result<String, Message> {
        let mut results = String::new();
        let mut children = flow_children(document, parent).peekable();
        self.index_stack.push(0);
        let mut index = 0;
        while let Some(child) = children.next() {
            *self.index_stack.last_mut().unwrap() = index;
            if !matches!(
                document.node(child).unwrap().kind,
                EventKind::Start(Tag::List { .. })
            ) {
                self.bullet_last_used = None;
            }
            results.push_str(&self.handle(document, child, &Info::new("\n", "\n"))?);
            if let Some(&next) = children.peek() {
                self.between(document, child, next, parent, &mut results);
            }
            index += 1;
        }
        self.index_stack.pop();
        Ok(results)
    }

    pub fn container_phrasing<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        parent: NodeId,
        info: &Info,
    ) -> Result<String, Message> {
        let mut results = String::new();
        let mut children = flow_children(document, parent).peekable();
        self.index_stack.push(0);
        let mut index = 0;
        let mut after_hard_break = false;
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
            let mut new_info = Info::new(info.before, info.after);
            let mut buffer = [0u8; 4];
            if let Some(&next) = children.peek() {
                if let Some(first) = self.peek_node(document, next) {
                    new_info.after = first.encode_utf8(&mut buffer);
                } else {
                    new_info.after = self
                        .handle(document, next, &Info::new("", ""))?
                        .chars()
                        .next()
                        .unwrap_or_default()
                        .encode_utf8(&mut buffer);
                }
            }
            // Inline HTML after a source line break would otherwise become a flow block.
            if !results.is_empty() {
                if (info.before == "\r" || info.before == "\n")
                    && matches!(document.node(child).unwrap().kind, EventKind::Html { .. })
                {
                    if results.ends_with(['\n', '\r']) {
                        results.pop();
                        if results.ends_with('\r') {
                            results.pop();
                        }
                    }
                    results.push(' ');
                    new_info.before = " ";
                } else {
                    new_info.before = &results;
                }
            }
            results.push_str(&self.handle(document, child, &new_info)?);
            index += 1;
        }
        self.index_stack.pop();
        Ok(results)
    }

    fn encode_char(character: char) -> String {
        let hex_code = u32::from(character);
        format!("&#x{:X};", hex_code)
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
                Tag::Emphasis => self.emphasis(document, id, info),
                Tag::Strong => self.strong(document, id, info),
                Tag::Delete => Ok(format!(
                    "~~{}~~",
                    self.container_phrasing(document, id, info)?
                )),
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
                    let mut value = format!("[^{}]:", label);
                    if flow_children(document, id).next().is_some() {
                        let content = self.container_flow(document, id)?;
                        value.push(' ');
                        value.push_str(&self.indent_lines(&content, |line, index, blank| {
                            if index == 0 || blank {
                                line.to_string()
                            } else {
                                format!("    {}", line)
                            }
                        }));
                    }
                    Ok(value)
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
            EventKind::Html { value, .. } => Ok(value.resolve(source).unwrap().to_string()),
            EventKind::TaskMarker(checked) => {
                if self.is_item_task(document, id) {
                    Ok(String::new())
                } else {
                    Ok(if *checked { "[x]" } else { "[ ]" }.into())
                }
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
                    Ok(value.resolve(source).unwrap().into())
                }
            }
            EventKind::FootnoteReference(label) => {
                let syntax = document.syntax(id);
                let label = match &syntax {
                    Some(NodeSyntax::Footnote { label }) => label.resolve(source).unwrap(),
                    _ => label.resolve(source).unwrap(),
                };
                Ok(format!("[^{}]", label))
            }
            EventKind::Separator { .. } => Ok(String::new()),
            EventKind::End(_) => unreachable!("arena nodes store starts and leaves only"),
        }
    }

    fn render_table<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        id: NodeId,
    ) -> Result<String, Message> {
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
    ) -> Result<String, Message> {
        let mut output = String::new();
        for (index, row) in rows.enumerate() {
            if !matches!(
                document.node(row).unwrap().kind,
                EventKind::Start(Tag::TableRow)
            ) {
                return Err(unsupported_content());
            }
            if index > 0 {
                output.push('\n');
            }
            output.push_str(&self.render_table_row(document, row)?);
            if header && index == 0 {
                output.push_str("\n| ");
                for (index, cell) in flow_children(document, row).enumerate() {
                    if index > 0 {
                        output.push_str(" | ");
                    }
                    let alignment = match document.node(cell).unwrap().kind {
                        EventKind::Start(Tag::TableCell { alignment, .. }) => alignment,
                        _ => unreachable!("rendered table rows contain only cells"),
                    };
                    output.push_str(match alignment {
                        AlignKind::Left => ":---",
                        AlignKind::Right => "---:",
                        AlignKind::Center => ":---:",
                        AlignKind::None => "---",
                    });
                }
                output.push_str(" |");
            }
        }
        Ok(output)
    }

    fn render_table_row<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        row: NodeId,
    ) -> Result<String, Message> {
        let mut value = String::from("| ");
        for (index, cell) in flow_children(document, row).enumerate() {
            if !matches!(
                document.node(cell).unwrap().kind,
                EventKind::Start(Tag::TableCell { .. })
            ) {
                return Err(unsupported_content());
            }
            if index > 0 {
                value.push_str(" | ");
            }
            let content = self.container_phrasing(document, cell, &Info::new("", ""))?;
            value.push_str(&escape_table_pipes(&content));
        }
        value.push_str(" |");
        Ok(value)
    }

    /// JS equivalent: <https://github.com/syntax-tree/mdast-util-to-markdown/blob/main/lib/util/indent-lines.js>.
    pub fn indent_lines(&self, value: &str, map: impl Fn(&str, usize, bool) -> String) -> String {
        let mut result = String::new();
        let mut start = 0;
        let mut line = 0;
        for ending in super::util::scan::line_endings(value) {
            let value_slice = &value[start..ending.start];
            result.push_str(&map(value_slice, line, value_slice.is_empty()));
            result.push_str(&value[ending.clone()]);
            start = ending.end;
            line += 1;
        }

        result.push_str(&map(&value[start..], line, value.is_empty()));
        result
    }

    fn join_defaults<S: AsRef<str>>(
        &self,
        document: &Document<S>,
        left: NodeId,
        right: NodeId,
        parent: NodeId,
    ) -> Join {
        let left_kind = &document.node(left).unwrap().kind;
        let right_kind = &document.node(right).unwrap().kind;
        if self.indented_code(document, right)
            && (matches!(left_kind, EventKind::Start(Tag::List { .. }))
                || self.indented_code(document, left))
        {
            return Join::HtmlComment;
        }
        if matches!(
            document.node(parent).unwrap().kind,
            EventKind::Start(Tag::Item { .. } | Tag::List { .. })
        ) {
            if matches!(left_kind, EventKind::Start(Tag::Paragraph { .. })) {
                if matches!(
                    right_kind,
                    EventKind::Start(Tag::Paragraph { .. }) | EventKind::Definition { .. }
                ) {
                    return Join::Break;
                }
                if matches!(right_kind, EventKind::Start(Tag::Heading(_)))
                    && format_heading_as_setext(document, right, self)
                {
                    return Join::Break;
                }
            }
            let spread = match document.node(parent).unwrap().kind {
                EventKind::Start(Tag::List { tight, .. }) => !tight,
                _ => self.item_spread(document, parent),
            };
            return Join::Lines(usize::from(spread));
        }
        Join::Break
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
            bullet_current: None,
            bullet_last_used: None,
            index_stack: Vec::new(),
            options,
            stack: Vec::new(),
            r#unsafe: Unsafe::get_default_unsafe(options),
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

    /// JS equivalent: <https://github.com/syntax-tree/mdast-util-to-markdown/blob/main/lib/util/safe.js>.
    pub fn safe(&mut self, input: &str, config: &SafeConfig) -> String {
        let value = format!("{}{}{}", config.before, input, config.after);
        let mut positions: Vec<usize> = Vec::new();
        let mut result: String = String::new();
        let mut infos: BTreeMap<usize, EscapeInfos> = BTreeMap::new();

        for pattern in &mut self.r#unsafe {
            if !pattern_in_scope(&self.stack, pattern) {
                continue;
            }

            Self::compile_pattern(pattern);

            let regex = pattern
                .compiled
                .as_ref()
                .expect("compiling an unsafe pattern sets its regex");
            for m in regex.captures_iter(&value) {
                let full_match = m.get(0).expect("Guaranteed to have a match");
                let captured_group_len = m
                    .get(1)
                    .map(|captured_group| captured_group.len())
                    .unwrap_or(0);
                let before = pattern.before.is_some() || pattern.at_break;
                let after = pattern.after.is_some();
                let position = full_match.start() + if before { captured_group_len } else { 0 };

                if positions.contains(&position) {
                    let entry = infos
                        .get_mut(&position)
                        .expect("every escaped position has matching escape info");
                    if entry.before && !before {
                        entry.before = false;
                    }
                    if entry.after && !after {
                        entry.after = false;
                    }
                } else {
                    infos.insert(position, EscapeInfos { after, before });
                    positions.push(position);
                }
            }
        }

        positions.sort_unstable();

        let mut start = config.before.len();
        let end = value.len() - config.after.len();

        for (index, position) in positions.iter().enumerate() {
            if *position < start || *position >= end {
                continue;
            }

            // If this character is supposed to be escaped because it has a condition on
            // the next character, and the next character is definitly being escaped,
            // then skip this escape.
            // This will never panic because the bounds are properly checked, and we
            // guarantee that the positions are already keys in the `infos` map before this
            // point in execution.
            if index + 1 < positions.len()
                && position + 1 < end
                && positions[index + 1] == position + 1
                && infos[position].after
                && !infos[&(position + 1)].before
                && !infos[&(position + 1)].after
                || index > 0
                    && positions[index - 1] == position - 1
                    && infos[position].before
                    && !infos[&(position - 1)].before
                    && !infos[&(position - 1)].after
            {
                continue;
            }

            if start != *position {
                result.push_str(&escape_backslashes(&value[start..*position], r"\"));
            }
            start = *position;

            let character = value[*position..]
                .chars()
                .next()
                .expect("escape position must identify a character");
            match character {
                '!'..='/' | ':'..='@' | '['..='`' | '{'..='~' => {
                    if let Some(encode) = &config.encode {
                        if *encode != character {
                            result.push('\\');
                        } else {
                            let encoded_char = Self::encode_char(character);
                            result.push_str(&encoded_char);
                            start += character.len_utf8();
                        }
                    } else {
                        result.push('\\');
                    }
                }
                character => {
                    let encoded_char = Self::encode_char(character);
                    result.push_str(&encoded_char);
                    start += character.len_utf8();
                }
            };
        }

        result.push_str(&escape_backslashes(&value[start..end], config.after));

        result
    }

    /// No real JS equivalent, but see:
    /// <https://github.com/syntax-tree/mdast-util-to-markdown/blob/fd6a508/lib/util/container-flow.js#L66>.
    fn set_between(join: &Join, results: &mut String) {
        match join {
            Join::Break => results.push_str("\n\n"),
            Join::Lines(1) => results.push_str("\n\n"),
            Join::Lines(n) => results.push_str("\n".repeat(1 + n).as_ref()),
            Join::HtmlComment => results.push_str("\n\n<!---->\n\n"),
        }
    }

    fn tight_definition<S: AsRef<str>>(
        &self,
        document: &Document<S>,
        left: NodeId,
        right: NodeId,
    ) -> Join {
        if matches!(
            document.node(left).unwrap().kind,
            EventKind::Definition { .. }
        ) && matches!(
            document.node(right).unwrap().kind,
            EventKind::Definition { .. }
        ) {
            Join::Lines(0)
        } else {
            Join::Break
        }
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

pub(super) fn trim_terminal_ending(value: &str) -> &str {
    value
        .strip_suffix("\r\n")
        .or_else(|| value.strip_suffix(['\r', '\n']))
        .unwrap_or(value)
}

pub(super) fn unsupported_content() -> Message {
    Message {
        place: None,
        reason: "This content shape cannot be represented in Markdown".into(),
        rule_id: alloc::boxed::Box::new("unsupported-node".into()),
        source: alloc::boxed::Box::new("markdown".into()),
    }
}

pub(super) fn invalid_node() -> Message {
    Message {
        place: None,
        reason: "Node does not belong to the document".into(),
        rule_id: alloc::boxed::Box::new("invalid-node".into()),
        source: alloc::boxed::Box::new("markdown".into()),
    }
}

fn escape_table_pipes(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut result = String::new();
    let mut index = 0;
    let mut code_ticks = None;

    while index < bytes.len() {
        if bytes[index] == b'`' {
            let start = index;
            while index < bytes.len() && bytes[index] == b'`' {
                index += 1;
            }
            let ticks = index - start;
            if code_ticks == Some(ticks) {
                code_ticks = None;
            } else if code_ticks.is_none() && has_matching_ticks(bytes, index, ticks) {
                code_ticks = Some(ticks);
            }
            result.push_str(&value[start..index]);
        } else if bytes[index] == b'|' && code_ticks.is_none() {
            let slashes = result
                .chars()
                .rev()
                .take_while(|character| *character == '\\')
                .count();
            if slashes % 2 == 0 {
                result.push('\\');
            }
            result.push('|');
            index += 1;
        } else {
            let character = value[index..].chars().next().expect("valid UTF-8");
            result.push(character);
            index += character.len_utf8();
        }
    }

    result
}

fn has_matching_ticks(bytes: &[u8], mut index: usize, ticks: usize) -> bool {
    while index < bytes.len() {
        if bytes[index] == b'`' {
            let start = index;
            while index < bytes.len() && bytes[index] == b'`' {
                index += 1;
            }
            if index - start == ticks {
                return true;
            }
        } else {
            index += 1;
        }
    }
    false
}

#[cfg(test)]
#[path = "state/tests.rs"]
mod tests;
