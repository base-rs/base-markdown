// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Markdown construct formatting adapted from `mdast-util-to-markdown` and `mdast-util-math`.
use super::{
    configure::IndentOptions,
    construct_name::ConstructName,
    state::{flow_children, unsupported_content, Info, State},
    util::{
        contains_control_or_whitespace::contains_control_or_whitespace,
        format_code_as_indented::format_code_as_indented,
        format_heading_as_setext::format_heading_as_setext,
        format_link_as_auto_link::{boundary, written_link, WrittenLink},
        scan::longest_run,
    },
};
use crate::{
    message::Message,
    stream::{EventKind, Tag},
    util::line_ending::line_endings,
    CodeFence, Document, NodeId, NodeSyntax, ReferenceKind,
};
use alloc::{
    borrow::Cow,
    string::{String, ToString},
};
use core::{fmt::Write, mem};

impl State<'_> {
    pub(super) fn blockquote<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        node: NodeId,
    ) -> Result<(), Message> {
        self.enter(ConstructName::Blockquote);
        let start = self.output.len();
        self.container_flow(document, node)?;
        self.indent_lines(start, |line, _, blank, output| {
            output.push('>');
            if !blank {
                output.push(' ');
            }
            output.push_str(line);
        });
        self.exit();
        Ok(())
    }

    pub(super) fn hard_break(&mut self, info: &Info) -> Result<(), Message> {
        if super::escape::unsafe_newline(&self.stack) {
            if !info.before(&self.output)[1].contains(['\t', ' ']) {
                self.output.push(' ');
            }
        } else {
            self.output.push_str("\\\n");
        }
        Ok(())
    }

    pub(super) fn code(
        &mut self,
        content: &str,
        language: Option<&str>,
        metadata: Option<&str>,
    ) -> Result<(), Message> {
        let marker = self.options.check_fence()?;
        let start = self.output.len();
        if format_code_as_indented(content, language.is_none(), self) {
            self.output.push_str(content);
            self.indent_lines(start, |line, _, blank, output| {
                if !blank {
                    output.push_str("    ");
                }
                output.push_str(line);
            });
            return Ok(());
        }
        let size = (longest_run(content, marker as u8) + 1).max(3);
        self.output.extend(core::iter::repeat(marker).take(size));
        if let Some(language) = language {
            self.enter(if marker == '`' {
                ConstructName::CodeFencedLangGraveAccent
            } else {
                ConstructName::CodeFencedLangTilde
            });
            self.write_safe(
                language,
                &Info::at(start, self.output.len(), " "),
                Some('`'),
            );
            self.exit();
            if let Some(metadata) = metadata {
                self.enter(if marker == '`' {
                    ConstructName::CodeFencedMetaGraveAccent
                } else {
                    ConstructName::CodeFencedMetaTilde
                });
                self.output.push(' ');
                self.write_safe(
                    metadata,
                    &Info::at(start, self.output.len(), "\n"),
                    Some('`'),
                );
                self.exit();
            }
        }
        self.output.push('\n');
        if !content.is_empty() {
            // Grow once for the body, closing fence and terminating line ending.
            self.output.reserve(content.len() + size + 2);
            self.output.push_str(content);
            self.output.push('\n');
        }
        self.output.extend(core::iter::repeat(marker).take(size));
        Ok(())
    }

    pub(super) fn definition(
        &mut self,
        label: &str,
        destination: &str,
        title: Option<&str>,
    ) -> Result<(), Message> {
        let quote = self.options.check_quote()?;
        let start = self.output.len();
        self.enter(ConstructName::Label);
        self.output.push('[');
        self.write_label(label, &Info::at(start, self.output.len(), "]"));
        self.output.push_str("]: ");
        self.exit();
        self.destination_title(
            start,
            destination,
            title,
            quote,
            destination.is_empty() || contains_control_or_whitespace(destination),
        );
        Ok(())
    }

    fn destination_title(
        &mut self,
        start: usize,
        destination: &str,
        title: Option<&str>,
        quote: char,
        literal: bool,
    ) {
        if literal {
            self.enter(ConstructName::DestinationLiteral);
            self.output.push('<');
            self.write_safe(destination, &Info::at(start, self.output.len(), ">"), None);
            self.output.push('>');
        } else {
            self.enter(ConstructName::DestinationRaw);
            self.write_safe(
                destination,
                &Info::at(
                    start,
                    self.output.len(),
                    if title.is_some() { " " } else { ")" },
                ),
                None,
            );
        }
        self.exit();
        if let Some(title) = title {
            self.enter(if quote == '"' {
                ConstructName::TitleQuote
            } else {
                ConstructName::TitleApostrophe
            });
            self.output.push(' ');
            self.output.push(quote);
            let mut buffer = [0u8; 4];
            self.write_safe(
                title,
                &Info::new(destination, quote.encode_utf8(&mut buffer)),
                None,
            );
            self.output.push(quote);
            self.exit();
        }
    }

    pub(super) fn heading<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        node: NodeId,
        rank: u8,
    ) -> Result<(), Message> {
        let start = self.output.len();
        if format_heading_as_setext(document, node, self) {
            self.enter(ConstructName::Phrasing);
            self.container_phrasing(document, node, &Info::new("\n", "\n"))?;
            self.exit();
            let content = &self.output[start..];
            let last_line = content.rfind('\n').map_or(0, |index| index + 1);
            let size = content.len() - last_line;
            self.output.push('\n');
            self.output
                .extend(core::iter::repeat(if rank == 1 { '=' } else { '-' }).take(size));
            return Ok(());
        }
        self.enter(ConstructName::HeadingAtx);
        self.enter(ConstructName::Phrasing);
        self.output
            .extend(core::iter::repeat('#').take(usize::from(rank)));
        self.output.push(' ');
        let content_start = self.output.len();
        self.container_phrasing(document, node, &Info::new("# ", "\n"))?;
        if self.output.len() == content_start {
            self.output.pop();
        } else if let Some(character) = self.output[content_start..]
            .chars()
            .next()
            .filter(|c| matches!(c, ' ' | '\t'))
        {
            self.output.replace_range(
                content_start..content_start + 1,
                if character == ' ' { "&#x20;" } else { "&#x9;" },
            );
        }
        if self.options.close_atx {
            self.output.push(' ');
            self.output
                .extend(core::iter::repeat('#').take(usize::from(rank)));
        }
        self.exit();
        self.exit();
        Ok(())
    }

    pub(super) fn paragraph<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        node: NodeId,
        info: &Info,
    ) -> Result<(), Message> {
        self.enter(ConstructName::Phrasing);
        self.container_phrasing(document, node, info)?;
        self.exit();
        Ok(())
    }

    pub(super) fn thematic_break(&mut self) -> Result<(), Message> {
        let marker = self.options.check_rule()?;
        for index in 0..self.options.check_rule_repetition()? {
            if index > 0 && self.options.rule_spaces {
                self.output.push(' ');
            }
            self.output.push(marker);
        }
        Ok(())
    }

    pub(super) fn inline_code(
        &mut self,
        content: &str,
        language: Option<&str>,
    ) -> Result<(), Message> {
        if let Some(language) = language {
            self.output.push_str(language);
            self.output.push(':');
        }
        self.inline_literal(content, '`', 1);
        Ok(())
    }

    pub(super) fn inline_math(&mut self, content: &str) -> Result<(), Message> {
        self.inline_literal(
            content,
            '$',
            if self.options.single_dollar_text_math {
                1
            } else {
                2
            },
        );
        Ok(())
    }

    fn inline_literal(&mut self, content: &str, marker: char, minimum: usize) {
        let size = super::util::scan::delimiter_size(content, marker as u8, minimum);
        self.output.extend(core::iter::repeat(marker).take(size));
        let start = self.output.len();
        let non_whitespace = !content.chars().all(char::is_whitespace);
        let pad = non_whitespace
            && (content.starts_with(char::is_whitespace) && content.ends_with(char::is_whitespace)
                || content.starts_with(marker)
                || content.ends_with(marker));
        if pad {
            self.output.push(' ');
        }
        // GFM splits table cells before parsing code and math, so their pipes need escapes too.
        if self.stack.contains(&ConstructName::TableCell) {
            for (index, part) in content.split('|').enumerate() {
                if index > 0 {
                    self.output.push_str("\\|");
                }
                self.output.push_str(part);
            }
        } else {
            self.output.push_str(content);
        }
        if pad {
            self.output.push(' ');
        }
        // Flatten a physical line ending only when a block-opening rule would become active.
        if line_endings(content).next().is_some() {
            self.rewrite(start, |value, output| {
                super::escape::normalize_inline(value);
                output.push_str(value);
            });
        }
        self.output.extend(core::iter::repeat(marker).take(size));
    }

    pub(super) fn math(&mut self, content: &str, metadata: Option<&str>) -> Result<(), Message> {
        let size = (longest_run(content, b'$') + 1).max(2);
        let start = self.output.len();
        self.output.extend(core::iter::repeat('$').take(size));
        if let Some(metadata) = metadata {
            self.enter(ConstructName::MathFlowMeta);
            self.write_safe(
                metadata,
                &Info::at(start, self.output.len(), "\n"),
                Some('$'),
            );
            self.exit();
        }
        self.output.push('\n');
        if !content.is_empty() {
            self.output.push_str(content);
            self.output.push('\n');
        }
        self.output.extend(core::iter::repeat('$').take(size));
        Ok(())
    }

    pub(super) fn text(&mut self, content: &str, info: &Info) -> Result<(), Message> {
        let value = match self.options.line_wrapping {
            crate::markdown::LineWrapping::Preserve => Cow::Borrowed(content),
            crate::markdown::LineWrapping::Unwrap => unwrap_soft_line_breaks(content),
        };
        self.write_safe(&value, info, None);
        Ok(())
    }

    pub(super) fn resource<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        node: NodeId,
        destination: &str,
        title: Option<&str>,
        image: bool,
    ) -> Result<(), Message> {
        let quote = self.options.check_quote()?;
        let start = self.output.len();
        if !image {
            let after_boundary = self.output.len() == self.line_start
                || self.output.chars().next_back().map_or(true, boundary);
            match written_link(
                document,
                node,
                destination,
                title.is_some(),
                after_boundary,
                self,
            ) {
                WrittenLink::Bare(text) => {
                    // Between boundaries, nothing next to the link can change where it ends.
                    self.output.push_str(text);
                    return Ok(());
                }
                WrittenLink::Angle(text) => {
                    let stack = mem::take(&mut self.stack);
                    self.enter(ConstructName::Autolink);
                    // Autolinks do not process escapes, and `auto_link_text` only accepts text that reads back unchanged.
                    self.output.push('<');
                    self.output.push_str(text);
                    self.output.push('>');
                    self.exit();
                    self.stack = stack;
                    return Ok(());
                }
                WrittenLink::Explicit => {}
            }
        }
        self.link_label(document, node, image, "](")?;
        self.output.push_str("](");
        self.destination_title(
            start,
            destination,
            title,
            quote,
            destination.is_empty() && title.is_some()
                || contains_control_or_whitespace(destination),
        );
        self.output.push(')');
        Ok(())
    }

    pub(super) fn reference<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        node: NodeId,
        kind: ReferenceKind,
        label: &str,
        image: bool,
    ) -> Result<(), Message> {
        let start = self.output.len();
        let text = self.link_label(document, node, image, "]")?;
        self.output.push_str("][");
        let stack = mem::take(&mut self.stack);
        self.enter(ConstructName::Reference);
        let reference_start = self.output.len();
        self.write_label(label, &Info::at(start, reference_start, "]"));
        self.exit();
        if stack.contains(&ConstructName::TableCell) {
            escape_table_pipes(&mut self.output, reference_start);
        }
        self.stack = stack;
        if matches!(kind, ReferenceKind::Full)
            || text.is_empty()
            || self.output[text] != self.output[reference_start..]
        {
            self.output.push(']');
        } else {
            self.output.truncate(reference_start);
            if matches!(kind, ReferenceKind::Shortcut) {
                self.output.pop();
            } else {
                self.output.push(']');
            }
        }
        Ok(())
    }

    fn link_label<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        node: NodeId,
        image: bool,
        after: &str,
    ) -> Result<core::ops::Range<usize>, Message> {
        self.enter(ConstructName::Label);
        let start = self.output.len();
        self.output.push_str(if image { "![" } else { "[" });
        let text_start = self.output.len();
        let info = Info::at(start, text_start, after);
        if image {
            self.write_safe(&image_alt(document, node), &info, None);
        } else {
            self.container_phrasing(document, node, &info)?;
        }
        self.exit();
        Ok(text_start..self.output.len())
    }

    pub(super) fn list<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        node: NodeId,
        ordered: bool,
    ) -> Result<(), Message> {
        self.enter(ConstructName::List);
        let bullet_current = self.bullet_current;
        let mut bullet = if ordered {
            self.options.check_bullet_ordered()?
        } else {
            self.options.check_bullet()?
        };
        let bullet_other = if ordered {
            if bullet == '.' {
                ')'
            } else {
                '.'
            }
        } else {
            self.options.check_bullet_other()?
        };
        let mut first_empty = false;
        let mut has_rule = false;
        for (index, child) in flow_children(document, node).enumerate() {
            if !matches!(
                document.node(child).unwrap().kind,
                EventKind::Start(Tag::Item { .. })
            ) {
                return Err(unsupported_content());
            }
            let first = flow_children(document, child).next();
            if index == 0 {
                first_empty = first.is_none();
            }
            has_rule |= first
                .is_some_and(|first| matches!(document.node(first).unwrap().kind, EventKind::Rule));
        }
        let mut different = self.bullet_last_used == Some(bullet);
        if !ordered {
            let depth = self.stack.len();
            let positions = self.index_stack.len();
            if matches!(bullet, '*' | '-')
                && depth >= 4
                && positions >= 3
                && first_empty
                && self.stack[depth - 1] == ConstructName::List
                && self.stack[depth - 2] == ConstructName::ListItem
                && self.stack[depth - 3] == ConstructName::List
                && self.stack[depth - 4] == ConstructName::ListItem
                && self.index_stack[positions - 1] == 0
                && self.index_stack[positions - 2] == 0
                && self.index_stack[positions - 3] == 0
            {
                different = true;
            }
            if self.options.check_rule()? == bullet && has_rule {
                different = true;
            }
        }
        if different {
            bullet = bullet_other;
        }
        self.bullet_current = Some(bullet);
        self.container_flow(document, node)?;
        self.bullet_last_used = Some(bullet);
        self.bullet_current = bullet_current;
        self.exit();
        Ok(())
    }

    pub(super) fn list_item<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        node: NodeId,
    ) -> Result<(), Message> {
        let mut bullet = self.bullet_current.unwrap_or(self.options.check_bullet()?);
        let prefix_start = self.output.len();
        let parent = document.node(node).unwrap().parent();
        if let Some(parent) = parent {
            if let EventKind::Start(Tag::List {
                start: Some(start), ..
            }) = document.node(parent).unwrap().kind
            {
                if self.bullet_current.is_none() {
                    bullet = self.options.check_bullet_ordered()?;
                }
                let position = if self.options.increment_list_marker {
                    self.index_stack.last().copied().unwrap_or_else(|| {
                        // A directly serialized item retains its position in the source list.
                        flow_children(document, parent)
                            .position(|id| id == node)
                            .expect("item belongs to its parent list")
                    })
                } else {
                    0
                };
                write!(self.output, "{}", start as usize + position)
                    .expect("writing to a String cannot fail");
            }
        }
        self.output.push(bullet);
        // The checkbox starts the item's content, as in `mdast-util-gfm-task-list-item`, so continuation lines indent to the content start the parser uses, which excludes the checkbox.
        let task =
            item_task(document, node).map(|(_, checked)| if checked { "[x]" } else { "[ ]" });
        let prefix_size = self.output.len() - prefix_start;
        let mut size = prefix_size + 1;
        let expand = match self.options.list_item_indent {
            IndentOptions::Mixed => {
                self.item_spread(document, node)
                    || parent.is_some_and(|parent| {
                        matches!(
                            document.node(parent).unwrap().kind,
                            EventKind::Start(Tag::List { tight: false, .. })
                        )
                    })
            }
            IndentOptions::Tab => true,
            IndentOptions::One => false,
        };
        if expand {
            size = size.div_ceil(4) * 4;
        }
        self.enter(ConstructName::ListItem);
        let start = self.output.len();
        self.container_flow(document, node)?;
        self.indent_lines(start, |line, index, blank, output| {
            if index == 0 {
                if let Some(task) = task {
                    output.extend(core::iter::repeat(' ').take(size - prefix_size));
                    // A task item's first line holds its paragraph, so it is never blank.
                    output.push_str(task);
                    output.push(' ');
                    output.push_str(line);
                    return;
                }
            }
            if !blank {
                let indent = if index == 0 { size - prefix_size } else { size };
                output.extend(core::iter::repeat(' ').take(indent));
            }
            output.push_str(line);
        });
        self.exit();
        Ok(())
    }

    pub(super) fn is_item_task<S: AsRef<str>>(&self, document: &Document<S>, node: NodeId) -> bool {
        document
            .node(node)
            .unwrap()
            .parent()
            .and_then(|parent| document.node(parent).unwrap().parent())
            .and_then(|item| item_task(document, item))
            .is_some_and(|(marker, _)| marker == node)
    }

    pub(super) fn item_spread<S: AsRef<str>>(&self, document: &Document<S>, node: NodeId) -> bool {
        matches!(
            document.syntax(node),
            Some(NodeSyntax::ListItem { spread: true })
        )
    }

    pub(super) fn frontmatter(
        &mut self,
        body: &str,
        authored_info: &str,
        fence: CodeFence,
        closing_fence_length: usize,
    ) -> Result<(), Message> {
        let mut info = authored_info.trim_matches([' ', '\t']);
        if !fence.valid_frontmatter_info(authored_info.as_bytes()) {
            return Err(Message {
                span: None,
                reason: "Frontmatter requires a valid fence and a single-line info string; code fences require a language hint".into(),
                rule_id: "invalid-frontmatter",
                source: "mdast-util-to-markdown",
            });
        }
        let mut marker = fence.character;
        if matches!(marker, '-' | '+') {
            let delimiter = marker.to_string().repeat(3);
            if body
                .split('\n')
                .any(|line| line.trim_end_matches([' ', '\t']) == delimiter)
            {
                marker = '~';
            }
        }
        let (opening_length, closing_length) = if matches!(marker, '-' | '+') {
            (3, 3)
        } else {
            let minimum = (longest_run(body, marker as u8) + 1).max(3);
            let opening = fence.length.max(minimum);
            (opening, closing_fence_length.max(opening))
        };
        if marker != fence.character && authored_info.split_ascii_whitespace().next().is_none() {
            info = if fence.character == '-' {
                "yaml"
            } else {
                "toml"
            };
        }
        let separator =
            if !info.is_empty() && (matches!(marker, '-' | '+') || info.starts_with(marker)) {
                " "
            } else {
                ""
            };
        self.output
            .extend(core::iter::repeat(marker).take(opening_length));
        self.output.push_str(separator);
        self.output.push_str(info);
        self.output.push('\n');
        let needs_line_ending = !body.is_empty() && !body.ends_with('\n');
        self.output
            .reserve(body.len() + closing_length + usize::from(needs_line_ending) + 1);
        self.output.push_str(body);
        if needs_line_ending {
            self.output.push('\n');
        }
        self.output
            .extend(core::iter::repeat(marker).take(closing_length));
        Ok(())
    }
}

fn item_task<S: AsRef<str>>(document: &Document<S>, item: NodeId) -> Option<(NodeId, bool)> {
    if !matches!(
        document.node(item).expect("task item node exists").kind,
        EventKind::Start(Tag::Item { .. })
    ) {
        return None;
    }
    let paragraph = flow_children(document, item).find(|child| {
        !matches!(
            document.node(*child).unwrap().kind,
            EventKind::Definition { .. }
        )
    })?;
    if !matches!(
        document
            .node(paragraph)
            .expect("task paragraph node exists")
            .kind,
        EventKind::Start(Tag::Paragraph { .. })
    ) {
        return None;
    }
    let marker = document.children(paragraph).next()?;
    match document.node(marker).expect("task marker node exists").kind {
        EventKind::TaskMarker(checked) => Some((marker, checked)),
        _ => None,
    }
}

fn image_alt<S: AsRef<str>>(document: &Document<S>, node: NodeId) -> Cow<'_, str> {
    let mut children = document.children(node);
    if let Some(child) = children.next() {
        if children.next().is_none() {
            if let EventKind::Text(value) = &document.node(child).unwrap().kind {
                return Cow::Borrowed(value.resolve(document.source()).unwrap());
            }
        }
    }
    Cow::Owned(document.text_content(node))
}

// Line endings in values are LF, and a CR is a character reference, so it is kept.
fn unwrap_soft_line_breaks(value: &str) -> Cow<'_, str> {
    if !value.contains('\n') {
        return Cow::Borrowed(value);
    }
    let mut result = String::with_capacity(value.len());
    let mut characters = value.chars().peekable();

    while let Some(character) = characters.next() {
        match character {
            '\n' => {
                while matches!(result.chars().next_back(), Some(' ' | '\t')) {
                    result.pop();
                }

                while characters
                    .peek()
                    .is_some_and(|next| matches!(*next, ' ' | '\t'))
                {
                    characters.next();
                }

                result.push(' ');
            }
            _ => result.push(character),
        }
    }

    Cow::Owned(result)
}

/// Escape each pipe after `start` that an even run of backslashes leaves splitting a table cell.
fn escape_table_pipes(output: &mut String, start: usize) {
    let mut position = start;
    while let Some(offset) = memchr::memchr(b'|', &output.as_bytes()[position..]) {
        let pipe = position + offset;
        let backslashes = output.as_bytes()[start..pipe]
            .iter()
            .rev()
            .take_while(|&&byte| byte == b'\\')
            .count();
        if backslashes % 2 == 0 {
            output.insert(pipe, '\\');
            position = pipe + 2;
        } else {
            position = pipe + 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::unwrap_soft_line_breaks;
    use alloc::borrow::Cow;

    #[test]
    fn unwrapping_borrows_unchanged_text_and_keeps_non_line_whitespace() {
        for value in ["", "plain", " é\tα \u{a0}\0", "a\rb"] {
            assert!(
                matches!(unwrap_soft_line_breaks(value), Cow::Borrowed(actual) if actual == value)
            );
        }
        for (value, expected) in [
            ("a \t\n \tb", "a b"),
            ("\n\n", " "),
            ("\t\n \t\nb\t ", " b\t "),
            ("é\u{a0} \n \tα", "é\u{a0} α"),
            ("a\r\nb", "a\r b"),
            ("a\0\nb\0", "a\0 b\0"),
        ] {
            assert_eq!(unwrap_soft_line_breaks(value), expected);
        }
    }
}
