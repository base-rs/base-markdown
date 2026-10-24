// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024-2025, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

//! Markdown construct formatting adapted from `mdast-util-to-markdown` and `mdast-util-math`.
use super::{
    configure::IndentOptions,
    construct_name::ConstructName,
    state::{flow_children, unsupported_content, Info, State},
    util::{
        check_bullet::check_bullet, check_bullet_ordered::check_bullet_ordered,
        check_bullet_other::check_bullet_other, check_emphasis::check_emphasis,
        check_fence::check_fence, check_quote::check_quote, check_rule::check_rule,
        check_rule_repetition::check_rule_repetition, check_strong::check_strong,
        contains_control_or_whitespace::contains_control_or_whitespace,
        format_code_as_indented::format_code_as_indented,
        format_heading_as_setext::format_heading_as_setext,
        format_link_as_auto_link::auto_link_text, longest_char_streak::longest_char_streak,
        pattern_in_scope::pattern_in_scope,
    },
};
use crate::{
    message::Message,
    stream::{EventKind, Tag},
    CodeFence, Document, NodeId, NodeSyntax, ReferenceKind,
};
use alloc::{
    borrow::Cow,
    format,
    string::{String, ToString},
};
use core::mem;

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
        if self
            .r#unsafe
            .iter()
            .any(|pattern| pattern.character == '\n' && pattern_in_scope(&self.stack, pattern))
        {
            if !info
                .before(&self.output)
                .chars()
                .any(|c| matches!(c, '\t' | ' '))
            {
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
        let marker = check_fence(self)?;
        let start = self.output.len();
        if format_code_as_indented(content, language.is_none(), self) {
            self.enter(ConstructName::CodeIndented);
            self.output.push_str(content);
            self.indent_lines(start, |line, _, blank, output| {
                if !blank {
                    output.push_str("    ");
                }
                output.push_str(line);
            });
            self.exit();
            return Ok(());
        }
        let size = (longest_char_streak(content, marker) + 1).max(3);
        self.enter(ConstructName::CodeFenced);
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
            self.output.push_str(content);
            self.output.push('\n');
        }
        self.output.extend(core::iter::repeat(marker).take(size));
        self.exit();
        Ok(())
    }

    pub(super) fn definition(
        &mut self,
        label: &str,
        destination: &str,
        title: Option<&str>,
    ) -> Result<(), Message> {
        let quote = check_quote(self)?;
        let start = self.output.len();
        self.enter(ConstructName::Definition);
        self.enter(ConstructName::Label);
        self.output.push('[');
        self.write_safe(label, &Info::at(start, self.output.len(), "]"), None);
        self.output.push_str("]: ");
        self.exit();
        if destination.is_empty() || contains_control_or_whitespace(destination) {
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
        self.exit();
        Ok(())
    }

    pub(super) fn emphasis<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        node: NodeId,
        info: &Info,
    ) -> Result<(), Message> {
        let marker = check_emphasis(self)?;
        self.enter(ConstructName::Emphasis);
        self.output.push(marker);
        self.container_phrasing(document, node, info)?;
        self.output.push(marker);
        self.exit();
        Ok(())
    }

    pub(super) fn heading<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        node: NodeId,
        rank: u8,
    ) -> Result<(), Message> {
        let start = self.output.len();
        if format_heading_as_setext(document, node, self) {
            self.enter(ConstructName::HeadingSetext);
            self.enter(ConstructName::Phrasing);
            self.container_phrasing(document, node, &Info::new("\n", "\n"))?;
            self.exit();
            self.exit();
            let content = &self.output[start..];
            let last_line = content.rfind(['\r', '\n']).map_or(0, |index| index + 1);
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
        self.enter(ConstructName::Paragraph);
        self.enter(ConstructName::Phrasing);
        self.container_phrasing(document, node, info)?;
        self.exit();
        self.exit();
        Ok(())
    }

    pub(super) fn strong<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        node: NodeId,
        info: &Info,
    ) -> Result<(), Message> {
        let marker = check_strong(self)?;
        self.enter(ConstructName::Strong);
        self.output.push(marker);
        self.output.push(marker);
        self.container_phrasing(document, node, info)?;
        self.output.push(marker);
        self.output.push(marker);
        self.exit();
        Ok(())
    }

    pub(super) fn thematic_break(&mut self) -> Result<(), Message> {
        let marker = check_rule(self)?;
        for index in 0..check_rule_repetition(self)? {
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
        self.output.push_str(content);
        if pad {
            self.output.push(' ');
        }
        // Flatten a physical line ending only when a block-opening rule would become active.
        let patterns = &mut self.r#unsafe;
        let mut scratch = core::mem::take(&mut self.scratch);
        scratch.clear();
        scratch.push_str(&self.output[start..]);
        for pattern in patterns {
            if !pattern.at_break {
                continue;
            }
            State::compile_pattern(pattern);
            let regex = pattern
                .compiled
                .as_ref()
                .expect("compiling an unsafe pattern sets its regex");
            while let Some(found) = regex.find(&scratch) {
                let at = found.start();
                let from = at
                    - usize::from(
                        at > 0
                            && scratch.as_bytes()[at - 1] == b'\r'
                            && scratch.as_bytes()[at] == b'\n',
                    );
                scratch.replace_range(from..at + 1, " ");
            }
        }
        self.output.truncate(start);
        self.output.push_str(&scratch);
        self.scratch = scratch;
        self.output.extend(core::iter::repeat(marker).take(size));
    }

    pub(super) fn math(&mut self, content: &str, metadata: Option<&str>) -> Result<(), Message> {
        let size = (longest_char_streak(content, '$') + 1).max(2);
        self.enter(ConstructName::MathFlow);
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
        self.exit();
        Ok(())
    }

    pub(super) fn text(&mut self, content: &str, info: &Info) -> Result<(), Message> {
        let value = match self.options.line_wrapping {
            crate::markdown::LineWrapping::Preserve => Cow::Borrowed(content),
            crate::markdown::LineWrapping::Unwrap => Cow::Owned(unwrap_soft_line_breaks(content)),
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
        let quote = check_quote(self)?;
        let start = self.output.len();
        if !image {
            if let Some(text) = auto_link_text(document, node, destination, title.is_some(), self) {
                let stack = mem::take(&mut self.stack);
                self.enter(ConstructName::Autolink);
                self.output.push('<');
                self.write_safe(text, &Info::at(start, self.output.len(), ">"), None);
                self.output.push('>');
                self.exit();
                self.stack = stack;
                return Ok(());
            }
        }
        self.enter(if image {
            ConstructName::Image
        } else {
            ConstructName::Link
        });
        self.enter(ConstructName::Label);
        self.output.push_str(if image { "![" } else { "[" });
        let info = Info::at(start, self.output.len(), "](");
        if image {
            self.write_safe(&image_alt(document, node), &info, None);
        } else {
            self.container_phrasing(document, node, &info)?;
        }
        self.output.push_str("](");
        self.exit();
        if destination.is_empty() && title.is_some() || contains_control_or_whitespace(destination)
        {
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
        self.output.push(')');
        self.exit();
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
        self.enter(if image {
            ConstructName::ImageReference
        } else {
            ConstructName::LinkReference
        });
        self.enter(ConstructName::Label);
        let start = self.output.len();
        self.output.push_str(if image { "![" } else { "[" });
        let text_start = self.output.len();
        let info = Info::at(start, text_start, "]");
        if image {
            self.write_safe(&image_alt(document, node), &info, None);
        } else {
            self.container_phrasing(document, node, &info)?;
        }
        let text_end = self.output.len();
        self.output.push_str("][");
        self.exit();
        let stack = mem::take(&mut self.stack);
        self.enter(ConstructName::Reference);
        let reference_start = self.output.len();
        self.write_safe(label, &Info::at(start, reference_start, "]"), None);
        self.exit();
        self.stack = stack;
        self.exit();
        if matches!(kind, ReferenceKind::Full)
            || text_start == text_end
            || self.output[text_start..text_end] != self.output[reference_start..]
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

    pub(super) fn list<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        node: NodeId,
        ordered: bool,
    ) -> Result<(), Message> {
        self.enter(ConstructName::List);
        let bullet_current = self.bullet_current;
        let mut bullet = if ordered {
            check_bullet_ordered(self)?
        } else {
            check_bullet(self)?
        };
        let bullet_other = if ordered {
            if bullet == '.' {
                ')'
            } else {
                '.'
            }
        } else {
            check_bullet_other(self)?
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
            if check_rule(self)? == bullet && has_rule {
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
        let mut bullet = self
            .bullet_current
            .unwrap_or(check_bullet(self)?)
            .to_string();
        let parent = document.node(node).unwrap().parent();
        if let Some(parent) = parent {
            if let EventKind::Start(Tag::List {
                start: Some(start), ..
            }) = document.node(parent).unwrap().kind
            {
                let position = if self.options.increment_list_marker {
                    flow_children(document, parent)
                        .position(|id| id == node)
                        .expect("item belongs to its parent list")
                } else {
                    0
                };
                bullet = format!("{}{}", start as usize + position, bullet);
            }
        }
        if let Some((_, checked)) = item_task(document, node) {
            bullet.push_str(if checked { " [x]" } else { " [ ]" });
        }
        let mut size = bullet.len() + 1;
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
                output.push_str(&bullet);
                if !blank {
                    output.extend(core::iter::repeat(' ').take(size - bullet.len()));
                }
            } else if !blank {
                output.extend(core::iter::repeat(' ').take(size));
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
        if (matches!(fence.character, '`' | '~') && info.is_empty())
            || authored_info.contains(['\r', '\n'])
            || (fence.character == '`' && authored_info.contains('`'))
        {
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
                .split(['\r', '\n'])
                .any(|line| line.trim_end_matches([' ', '\t']) == delimiter)
            {
                marker = '~';
            }
        }
        let (opening_length, closing_length) = if matches!(marker, '-' | '+') {
            (3, 3)
        } else {
            let minimum = (longest_char_streak(body, marker) + 1).max(3);
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
        self.output.push_str(body);
        if !body.is_empty() && !body.ends_with(['\r', '\n']) {
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

fn unwrap_soft_line_breaks(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let mut characters = value.chars().peekable();

    while let Some(character) = characters.next() {
        match character {
            '\r' | '\n' => {
                if character == '\r' && characters.peek() == Some(&'\n') {
                    characters.next();
                }

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

    result
}
