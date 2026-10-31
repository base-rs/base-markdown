// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Markdown construct formatting adapted from `mdast-util-to-markdown` and `mdast-util-math`.
use super::{
    configure::IndentOptions,
    construct_name::ConstructName,
    state::{flow_children, unsupported_content, BareLink, Info, State},
    util::{
        contains_control_or_whitespace::contains_control_or_whitespace,
        format_code_as_indented::format_code_as_indented,
        format_heading_as_setext::format_heading_as_setext,
        format_link_as_auto_link::{written_link, WrittenLink},
        scan::longest_run,
    },
};
use crate::{
    inline_parser::web_literal_end,
    message::Message,
    stream::{EventKind, Tag},
    util::{
        char::{classify, Kind},
        line_ending::line_endings,
        scheme::{scheme, SchemeKind},
    },
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

    pub(super) fn delimited_phrasing<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        node: NodeId,
        info: &Info,
        marker: char,
        length: usize,
    ) -> Result<(), Message> {
        // Whitespace next to a marker would stop it opening or closing, so it is written as a
        // reference. A marker between a reference and a letter neither opens nor closes either, so
        // that letter is a reference too.
        let leading = starts_with_whitespace(document, node);
        if leading && self.bare_link.map(|link| link.next) != Some(self.output.len()) {
            // Output before the floor is final, so the character is only rewritten above it.
            if let Some(previous) = self.output.chars().next_back().filter(|&previous| {
                classify(previous) == Kind::Other
                    && self.output.len() - previous.len_utf8() >= self.write_floor
            }) {
                let end = self.output.len() - previous.len_utf8();
                self.output.truncate(end);
                write!(self.output, "&#x{:X};", u32::from(previous))
                    .expect("String writing is infallible");
            }
        }
        let start = self.output.len();
        self.output.extend(core::iter::repeat(marker).take(length));
        let content = self.output.len();
        if leading {
            self.attention_start = Some(content);
        }
        // The content follows the opening marker, so it never starts a line.
        let inner = Info::at(start, content, info.after);
        self.container_phrasing(document, node, &inner)?;
        let trailing = encode_trailing_whitespace(&mut self.output, content);
        // A web link reads on across the closing marker, so text after it may still join it.
        let link = self
            .bare_link
            .filter(|link| link.next == self.output.len() && !link.email);
        self.output.extend(core::iter::repeat(marker).take(length));
        self.bare_link = link.map(|link| BareLink {
            next: self.output.len(),
            ..link
        });
        if trailing && info.after.chars().next().map(classify) == Some(Kind::Other) {
            self.attention_end = Some(self.output.len());
        }
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
            self.enter(ConstructName::Phrasing);
            self.container_phrasing(document, node, &Info::new("\n", "\n"))?;
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
        let at = Some(self.output.len());
        // Next to an attention marker or after a bare link, the first character may need another form.
        let consumed = if let Some(first) = value.chars().next().filter(|&first| {
            self.attention_start == at && edge_whitespace(first)
                || self.attention_end == at && classify(first) == Kind::Other
        }) {
            write!(self.output, "&#x{:X};", u32::from(first))
                .expect("String writing is infallible");
            first.len_utf8()
        } else if let Some(byte) = self
            .bare_link
            .filter(|link| Some(link.next) == at)
            .and_then(|link| {
                if link.email {
                    joining_byte(&value, info.after, link.resource)
                } else {
                    extends_web_link(
                        &self.output[link.start..link.next],
                        link.end - link.start,
                        &value,
                        info.after,
                    )
                }
            })
        {
            // A bare link would take this character, so it is written in a form that ends the link.
            if byte.is_ascii_punctuation() {
                self.output.push('\\');
                self.output.push(char::from(byte));
            } else {
                write!(self.output, "&#x{:X};", byte).expect("String writing is infallible");
            }
            1
        } else {
            0
        };
        // An empty text writes nothing, so the text after it is still next to the same neighbour.
        if !value.is_empty() {
            self.clear_neighbours();
        }
        if consumed == 0 {
            self.write_safe(&value, info, None);
        } else {
            let info = info.extended(self.output.len());
            self.write_safe(&value[consumed..], &info, None);
        }
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
            match written_link(document, node, destination, title.is_some(), self) {
                WrittenLink::Bare(text) => {
                    escape_angle_before(&mut self.output, self.write_floor, text);
                    let text_start = self.output.len();
                    self.output.push_str(text);
                    // GFM splits table cells at `|` before it finds literal autolinks.
                    if self.stack.contains(&ConstructName::TableCell) {
                        escape_table_pipes(&mut self.output, text_start);
                    }
                    let address = destination
                        .split_once(':')
                        .and_then(|(name, _)| scheme(name.as_bytes()))
                        .and_then(|scheme| match scheme.kind {
                            SchemeKind::Address { resource } => Some(resource),
                            _ => None,
                        });
                    self.bare_link = Some(BareLink {
                        start: text_start,
                        end: self.output.len(),
                        next: self.output.len(),
                        email: address.is_some(),
                        resource: address == Some(true),
                    });
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
                .split(['\r', '\n'])
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
        let needs_line_ending = !body.is_empty() && !body.ends_with(['\r', '\n']);
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

fn unwrap_soft_line_breaks(value: &str) -> Cow<'_, str> {
    if line_endings(value).next().is_none() {
        return Cow::Borrowed(value);
    }
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

    Cow::Owned(result)
}

/// Whitespace that a character reference can stand for. Line endings are left alone, since they
/// belong to the line structure.
fn edge_whitespace(character: char) -> bool {
    character.is_whitespace() && !matches!(character, '\n' | '\r')
}

/// Whether the first text written in `node` is its own and starts with whitespace.
fn starts_with_whitespace<S: AsRef<str>>(document: &Document<S>, node: NodeId) -> bool {
    document
        .children(node)
        .map(
            |child| match &document.node(child).expect("child exists").kind {
                EventKind::Text(text) => text.resolve(document.source()).unwrap().chars().next(),
                // Other nodes write their own first character, which is not whitespace.
                _ => Some('.'),
            },
        )
        .find(Option::is_some)
        .flatten()
        .is_some_and(edge_whitespace)
}

/// Replace whitespace that ends the output after `start` with a character reference.
fn encode_trailing_whitespace(output: &mut String, start: usize) -> bool {
    let Some(last) = output[start..]
        .chars()
        .next_back()
        .filter(|&character| edge_whitespace(character))
    else {
        return false;
    };
    let end = output.len() - last.len_utf8();
    output.truncate(end);
    write!(output, "&#x{:X};", u32::from(last)).expect("String writing is infallible");
    true
}

/// The first byte of text after a bare email address, when the address would take it. An address
/// stops at any escape or character reference, and takes a `/` only when its scheme allows a
/// `/resource`.
fn joining_byte(text: &str, after: &str, resource: bool) -> Option<u8> {
    let mut bytes = text.bytes().chain(after.bytes());
    let byte = bytes.next();
    let joins = match byte {
        Some(b'-' | b'_' | b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z') => true,
        Some(b'/') => resource,
        Some(b'.') => bytes
            .next()
            .is_some_and(|next| next.is_ascii_alphanumeric()),
        _ => false,
    };
    byte.filter(|_| joins)
}

/// The first byte of text after a bare web link, when the text would change where the link ends
/// and a backslash, which ends the link, can escape it. Other characters are left as they are.
/// `written` is the link's text, `length` bytes, then any closing markers written after it.
fn extends_web_link(written: &str, length: usize, text: &str, after: &str) -> Option<u8> {
    let byte = text.bytes().next().filter(u8::is_ascii_punctuation)?;
    if web_literal_end(&written[..length]) != Some(length) {
        return None;
    }
    // The link can only reach as far as the next whitespace.
    let run = text
        .find(|character: char| character.is_whitespace())
        .unwrap_or(text.len());
    let mut joined = String::with_capacity(written.len() + run + after.len());
    joined.push_str(written);
    joined.push_str(&text[..run]);
    if run == text.len() {
        joined.push_str(after);
    }
    (web_literal_end(&joined) != Some(length)).then_some(byte)
}

/// Escape a `<` that ends `output` when `<text>` would be an autolink, where `text` is a bare link
/// written next. The `<` was text before the link, since the parser had not linked it. Output
/// before `floor` is final, so a `<` there is left as it is.
fn escape_angle_before(output: &mut String, floor: usize, text: &str) {
    let Some(angle) = output
        .len()
        .checked_sub(1)
        .filter(|&angle| angle >= floor && output.as_bytes()[angle] == b'<')
    else {
        return;
    };
    let backslashes = output.as_bytes()[..angle]
        .iter()
        .rev()
        .take_while(|&&byte| byte == b'\\')
        .count();
    let mut link = crate::util::autolink::Autolink::default();
    // The link ends before a `>`, so `<text>` would read as an autolink when a `>` completes it.
    if backslashes % 2 == 0
        && text.bytes().all(|byte| link.advance(byte) == Some(false))
        && link.advance(b'>') == Some(true)
    {
        output.insert(angle, '\\');
    }
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
        for value in ["", "plain", " é\tα \u{a0}\0"] {
            assert!(
                matches!(unwrap_soft_line_breaks(value), Cow::Borrowed(actual) if actual == value)
            );
        }
        for (value, expected) in [
            ("a \t\r\n \tb", "a b"),
            ("\r\n\r\n", " "),
            ("\t\n \t\r\nb\t ", " b\t "),
            ("é\u{a0} \r\n \tα", "é\u{a0} α"),
            ("a\n\r\nb", "a b"),
            ("a\0\rb\0", "a\0 b\0"),
        ] {
            assert_eq!(unwrap_soft_line_breaks(value), expected);
        }
    }
}
