// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024-2025, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

//! Markdown construct formatting adapted from `mdast-util-to-markdown` and `mdast-util-math`.
use super::{
    configure::IndentOptions,
    construct_name::ConstructName,
    state::{flow_children, Info, State},
    util::{
        check_bullet::check_bullet, check_bullet_ordered::check_bullet_ordered,
        check_bullet_other::check_bullet_other, check_emphasis::check_emphasis,
        check_fence::check_fence, check_quote::check_quote, check_rule::check_rule,
        check_rule_repetition::check_rule_repetition, check_strong::check_strong,
        contains_control_or_whitespace::contains_control_or_whitespace,
        format_code_as_indented::format_code_as_indented,
        format_heading_as_setext::format_heading_as_setext,
        format_link_as_auto_link::auto_link_text, longest_char_streak::longest_char_streak,
        pattern_in_scope::pattern_in_scope, safe::SafeConfig,
    },
};
use crate::{
    message::Message,
    stream::{EventKind, Tag},
    CodeFence, Document, NodeId, NodeSyntax, ReferenceKind,
};
use alloc::{
    borrow::Cow,
    boxed::Box,
    format,
    string::{String, ToString},
};
use core::mem;
use regex::Regex;

impl State<'_> {
    pub(super) fn blockquote<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        node: NodeId,
    ) -> Result<String, Message> {
        self.enter(ConstructName::Blockquote);
        let value = self.container_flow(document, node)?;
        let value = self.indent_lines(&value, |line, _, blank| {
            format!(">{}{}", if blank { "" } else { " " }, line)
        });
        self.exit();
        Ok(value)
    }

    pub(super) fn hard_break(&mut self, info: &Info) -> Result<String, Message> {
        for pattern in self.r#unsafe.iter() {
            // If we can’t put eols in this construct (setext headings, tables), use a
            // space instead.
            if pattern.character == '\n' && pattern_in_scope(&self.stack, pattern) {
                let space_or_tab = info.before.chars().any(|c| c == '\t' || c == ' ');

                if space_or_tab {
                    return Ok("".to_string());
                }

                return Ok(" ".to_string());
            }
        }

        Ok("\\\n".to_string())
    }

    pub(super) fn code(
        &mut self,
        content: &str,
        language: Option<&str>,
        metadata: Option<&str>,
    ) -> Result<String, Message> {
        let marker = check_fence(self)?;

        if format_code_as_indented(content, language.is_none(), self) {
            self.enter(ConstructName::CodeIndented);
            let value = self.indent_lines(content, |line, _, blank| {
                if blank {
                    String::new()
                } else {
                    format!("    {}", line)
                }
            });
            self.exit();
            return Ok(value);
        }

        let sequence = marker
            .to_string()
            .repeat((longest_char_streak(content, marker) + 1).max(3));

        self.enter(ConstructName::CodeFenced);
        let mut value = sequence.clone();

        if let Some(lang) = language {
            let code_fenced_lang_construct = if marker == '`' {
                ConstructName::CodeFencedLangGraveAccent
            } else {
                ConstructName::CodeFencedLangTilde
            };
            self.enter(code_fenced_lang_construct);

            value.push_str(&self.safe(lang, &SafeConfig::new(&value, " ", Some('`'))));

            self.exit();

            if let Some(meta) = metadata {
                let code_fenced_meta_construct = if marker == '`' {
                    ConstructName::CodeFencedMetaGraveAccent
                } else {
                    ConstructName::CodeFencedMetaTilde
                };

                self.enter(code_fenced_meta_construct);
                value.push(' ');

                value.push_str(&self.safe(meta, &SafeConfig::new(&value, "\n", Some('`'))));

                self.exit();
            }
        }

        value.push('\n');

        if !content.is_empty() {
            value.push_str(content);
            value.push('\n');
        }

        value.push_str(&sequence);
        self.exit();

        Ok(value)
    }

    pub(super) fn definition(
        &mut self,
        label: &str,
        destination: &str,
        title: Option<&str>,
    ) -> Result<String, Message> {
        let quote = check_quote(self)?;

        self.enter(ConstructName::Definition);
        self.enter(ConstructName::Label);

        let mut value = String::from('[');

        value.push_str(&self.safe(label, &SafeConfig::new(&value, "]", None)));

        value.push_str("]: ");

        self.exit();

        if destination.is_empty() || contains_control_or_whitespace(destination) {
            self.enter(ConstructName::DestinationLiteral);
            value.push('<');
            value.push_str(&self.safe(destination, &SafeConfig::new(&value, ">", None)));
            value.push('>');
        } else {
            self.enter(ConstructName::DestinationRaw);
            let after = if title.is_some() { " " } else { ")" };
            value.push_str(&self.safe(destination, &SafeConfig::new(&value, after, None)));
        }

        self.exit();

        if let Some(title) = title {
            let title_construct_name = if quote == '"' {
                ConstructName::TitleQuote
            } else {
                ConstructName::TitleApostrophe
            };

            self.enter(title_construct_name);
            value.push(' ');
            value.push(quote);

            let mut before_buffer = [0u8; 4];
            let before = quote.encode_utf8(&mut before_buffer);
            value.push_str(&self.safe(title, &SafeConfig::new(destination, before, None)));

            value.push(quote);
            self.exit();
        }

        self.exit();

        Ok(value)
    }

    pub(super) fn emphasis<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        node: NodeId,
        info: &Info,
    ) -> Result<String, Message> {
        let marker = check_emphasis(self)?;

        self.enter(ConstructName::Emphasis);

        let mut value = format!(
            "{}{}",
            marker,
            self.container_phrasing(document, node, info)?
        );
        value.push(marker);

        self.exit();

        Ok(value)
    }

    pub(super) fn heading<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        node: NodeId,
        rank: u8,
    ) -> Result<String, Message> {
        if format_heading_as_setext(document, node, self) {
            self.enter(ConstructName::HeadingSetext);
            self.enter(ConstructName::Phrasing);
            let mut value = self.container_phrasing(document, node, &Info::new("\n", "\n"))?;

            self.exit();
            self.exit();

            let underline_char = if rank == 1 { "=" } else { "-" };
            let last_line_rank = value
                .rfind('\n')
                .unwrap_or(0)
                .max(value.rfind('\r').unwrap_or(0));

            let last_line_rank = if last_line_rank > 0 {
                last_line_rank + 1
            } else {
                0
            };

            let setext_underline = underline_char.repeat(value.len() - last_line_rank);
            value.push('\n');
            value.push_str(&setext_underline);

            return Ok(value);
        }

        let sequence = "#".repeat(rank as usize);
        self.enter(ConstructName::HeadingAtx);
        self.enter(ConstructName::Phrasing);

        let mut value = self.container_phrasing(document, node, &Info::new("# ", "\n"))?;

        if let Some(first_char) = value.chars().nth(0) {
            if first_char == ' ' || first_char == '\t' {
                let hex_code = u32::from(first_char);
                value = format!("&#x{:X};{}", hex_code, &value[1..])
            }
        }

        if value.is_empty() {
            value.push_str(&sequence);
        } else {
            value = format!("{} {}", sequence, value);
        }

        if self.options.close_atx {
            value.push(' ');
            value.push_str(&sequence);
        }

        self.exit();
        self.exit();

        Ok(value)
    }

    pub(super) fn paragraph<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        node: NodeId,
        info: &Info,
    ) -> Result<String, Message> {
        self.enter(ConstructName::Paragraph);
        self.enter(ConstructName::Phrasing);
        let value = self.container_phrasing(document, node, info)?;
        self.exit();
        self.exit();
        Ok(value)
    }

    pub(super) fn strong<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        node: NodeId,
        info: &Info,
    ) -> Result<String, Message> {
        let marker = check_strong(self)?;

        self.enter(ConstructName::Strong);

        let mut value = format!(
            "{}{}{}",
            marker,
            marker,
            self.container_phrasing(document, node, info)?
        );
        value.push(marker);
        value.push(marker);

        self.exit();

        Ok(value)
    }

    pub(super) fn thematic_break(&mut self) -> Result<String, Message> {
        let marker = check_rule(self)?;
        let space = if self.options.rule_spaces { " " } else { "" };
        let mut value =
            format!("{}{}", marker, space).repeat(check_rule_repetition(self)? as usize);

        if self.options.rule_spaces {
            // Remove the last space.
            value.pop();
            Ok(value)
        } else {
            Ok(value)
        }
    }

    pub(super) fn inline_code(
        &mut self,
        content: &str,
        language: Option<&str>,
    ) -> Result<String, Message> {
        let mut value = content.to_string();
        let mut sequence = String::from('`');
        let mut grave_accent_match = Regex::new(&format!(r"(^|[^`]){}([^`]|$)", sequence)).unwrap();
        while grave_accent_match.is_match(&value) {
            sequence.push('`');
            grave_accent_match = Regex::new(&format!(r"(^|[^`]){}([^`]|$)", sequence)).unwrap();
        }

        let no_whitespaces = !value.chars().all(char::is_whitespace);
        let starts_with_whitespace = value.starts_with(char::is_whitespace);
        let ends_with_whitespace = value.ends_with(char::is_whitespace);
        let starts_with_tick = value.starts_with('`');
        let ends_with_tick = value.ends_with('`');

        if no_whitespaces
            && ((starts_with_whitespace && ends_with_whitespace)
                || starts_with_tick
                || ends_with_tick)
        {
            value = format!("{}{}{}", ' ', value, ' ');
        }

        for pattern in &mut self.r#unsafe {
            if !pattern.at_break {
                continue;
            }

            State::compile_pattern(pattern);

            let regex = pattern
                .compiled
                .as_ref()
                .expect("compiling an unsafe pattern sets its regex");
            while let Some(m) = regex.find(&value) {
                let match_start = m.start();
                let (start, end) = if match_start > 0
                    && value.as_bytes()[match_start - 1] == b'\r'
                    && value.as_bytes()[match_start] == b'\n'
                {
                    (match_start - 1, match_start + 1)
                } else {
                    (match_start, match_start + 1)
                };

                value.replace_range(start..end, " ");
            }
        }

        let prefix = language
            .map(|lang| format!("{}:", lang))
            .unwrap_or_default();

        Ok(format!("{}{}{}{}", prefix, sequence, value, sequence))
    }

    pub(super) fn inline_math(&mut self, content: &str) -> Result<String, Message> {
        let mut size: usize = if !self.options.single_dollar_text_math {
            2
        } else {
            1
        };

        let pattern = format!("(^|[^$]){}([^$]|$)", "\\$".repeat(size));
        let mut dollar_sign_match = Regex::new(&pattern).unwrap();
        while dollar_sign_match.is_match(content) {
            size += 1;
            let pattern = format!("(^|[^$]){}([^$]|$)", "\\$".repeat(size));
            dollar_sign_match = Regex::new(&pattern).unwrap();
        }

        let sequence = "$".repeat(size);

        let no_whitespaces = !content.chars().all(char::is_whitespace);
        let starts_with_whitespace = content.starts_with(char::is_whitespace);
        let ends_with_whitespace = content.ends_with(char::is_whitespace);
        let starts_with_dollar = content.starts_with('$');
        let ends_with_dollar = content.ends_with('$');

        let mut value = content.to_string();
        if no_whitespaces
            && ((starts_with_whitespace && ends_with_whitespace)
                || starts_with_dollar
                || ends_with_dollar)
        {
            value = format!(" {} ", value);
        }

        for pattern in &mut self.r#unsafe {
            if !pattern.at_break {
                continue;
            }

            State::compile_pattern(pattern);

            let regex = pattern
                .compiled
                .as_ref()
                .expect("compiling an unsafe pattern sets its regex");
            while let Some(m) = regex.find(&value) {
                let match_start = m.start();
                let (start, end) = if match_start > 0
                    && value.as_bytes()[match_start - 1] == b'\r'
                    && value.as_bytes()[match_start] == b'\n'
                {
                    (match_start - 1, match_start + 1)
                } else {
                    (match_start, match_start + 1)
                };

                value.replace_range(start..end, " ");
            }
        }

        Ok(format!("{}{}{}", sequence, value, sequence))
    }

    pub(super) fn math(
        &mut self,
        content: &str,
        metadata: Option<&str>,
    ) -> Result<String, Message> {
        let sequence = "$".repeat((longest_char_streak(content, '$') + 1).max(2));
        self.enter(ConstructName::MathFlow);

        let mut value = String::new();
        value.push_str(&sequence);

        if let Some(meta) = metadata {
            self.enter(ConstructName::MathFlowMeta);
            value.push_str(&self.safe(meta, &SafeConfig::new(&value, "\n", Some('$'))));
            self.exit();
        }

        value.push('\n');

        if !content.is_empty() {
            value.push_str(content);
            value.push('\n');
        }

        value.push_str(&sequence);
        self.exit();
        Ok(value)
    }

    pub(super) fn text(&mut self, content: &str, info: &Info) -> Result<String, Message> {
        let value = match self.options.line_wrapping {
            crate::markdown::LineWrapping::Preserve => Cow::Borrowed(content),
            crate::markdown::LineWrapping::Unwrap => Cow::Owned(unwrap_soft_line_breaks(content)),
        };

        Ok(self.safe(&value, &SafeConfig::new(info.before, info.after, None)))
    }

    pub(super) fn resource<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        node: NodeId,
        destination: &str,
        title: Option<&str>,
        image: bool,
    ) -> Result<String, Message> {
        let quote = check_quote(self)?;
        if !image {
            if let Some(text) = auto_link_text(document, node, destination, title.is_some(), self) {
                let old_stack = mem::take(&mut self.stack);
                self.enter(ConstructName::Autolink);
                let mut value = String::from("<");
                value.push_str(&self.safe(text, &SafeConfig::new(&value, ">", None)));
                value.push('>');
                self.exit();
                self.stack = old_stack;
                return Ok(value);
            }
        }
        self.enter(if image {
            ConstructName::Image
        } else {
            ConstructName::Link
        });
        self.enter(ConstructName::Label);
        let mut value = String::from(if image { "![" } else { "[" });
        if image {
            let alt = image_alt(document, node);
            value.push_str(&self.safe(&alt, &SafeConfig::new(&value, "]", None)));
        } else {
            value.push_str(&self.container_phrasing(document, node, &Info::new(&value, "]("))?);
        }
        value.push_str("](");
        self.exit();
        if destination.is_empty() && title.is_some() || contains_control_or_whitespace(destination)
        {
            self.enter(ConstructName::DestinationLiteral);
            value.push('<');
            value.push_str(&self.safe(destination, &SafeConfig::new(&value, ">", None)));
            value.push('>');
        } else {
            self.enter(ConstructName::DestinationRaw);
            value.push_str(&self.safe(
                destination,
                &SafeConfig::new(&value, if title.is_some() { " " } else { ")" }, None),
            ));
        }
        self.exit();
        if let Some(title) = title {
            self.enter(if quote == '"' {
                ConstructName::TitleQuote
            } else {
                ConstructName::TitleApostrophe
            });
            value.push(' ');
            value.push(quote);
            let mut quote_buffer = [0u8; 4];
            value.push_str(&self.safe(
                title,
                &SafeConfig::new(destination, quote.encode_utf8(&mut quote_buffer), None),
            ));
            value.push(quote);
            self.exit();
        }
        value.push(')');
        self.exit();
        Ok(value)
    }

    pub(super) fn reference<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        node: NodeId,
        kind: ReferenceKind,
        label: &str,
        image: bool,
    ) -> Result<String, Message> {
        self.enter(if image {
            ConstructName::ImageReference
        } else {
            ConstructName::LinkReference
        });
        self.enter(ConstructName::Label);
        let mut value = String::from(if image { "![" } else { "[" });
        let text = if image {
            self.safe(
                &image_alt(document, node),
                &SafeConfig::new(&value, "]", None),
            )
        } else {
            self.container_phrasing(document, node, &Info::new(&value, "]"))?
        };
        value.push_str(&text);
        value.push_str("][");
        self.exit();
        let old_stack = mem::take(&mut self.stack);
        self.enter(ConstructName::Reference);
        let reference = self.safe(label, &SafeConfig::new(&value, "]", None));
        self.exit();
        self.stack = old_stack;
        self.exit();
        if matches!(kind, ReferenceKind::Full) || text.is_empty() || text != reference {
            value.push_str(&reference);
            value.push(']');
        } else if matches!(kind, ReferenceKind::Shortcut) {
            value.pop();
        } else {
            value.push(']');
        }
        Ok(value)
    }

    pub(super) fn list<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        node: NodeId,
        ordered: bool,
    ) -> Result<String, Message> {
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
        let mut different = self.bullet_last_used == Some(bullet);
        if !ordered {
            let first_empty = flow_children(document, node)
                .next()
                .is_some_and(|first| flow_children(document, first).next().is_none());
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
            if check_rule(self)? == bullet {
                for child in flow_children(document, node) {
                    if matches!(
                        document.node(child).unwrap().kind,
                        EventKind::Start(Tag::Item { .. })
                    ) && flow_children(document, child).next().is_some_and(|first| {
                        matches!(document.node(first).unwrap().kind, EventKind::Rule)
                    }) {
                        different = true;
                        break;
                    }
                }
            }
        }
        if different {
            bullet = bullet_other;
        }
        self.bullet_current = Some(bullet);
        let value = self.container_flow(document, node)?;
        self.bullet_last_used = Some(bullet);
        self.bullet_current = bullet_current;
        self.exit();
        Ok(value)
    }

    pub(super) fn list_item<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        node: NodeId,
    ) -> Result<String, Message> {
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
        let value = self.container_flow(document, node)?;
        let value = self.indent_lines(&value, |line, index, blank| {
            if index > 0 {
                if blank {
                    line.into()
                } else {
                    format!("{}{}", " ".repeat(size), line)
                }
            } else if blank {
                format!("{}{}", bullet, line)
            } else {
                format!("{}{}{}", bullet, " ".repeat(size - bullet.len()), line)
            }
        });
        self.exit();
        Ok(value)
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
    ) -> Result<String, Message> {
        let mut info = authored_info.trim_matches([' ', '\t']);
        if !matches!(fence.character, '`' | '~' | '-' | '+')
            || (matches!(fence.character, '`' | '~') && info.is_empty())
            || authored_info.contains(['\r', '\n'])
            || (fence.character == '`' && authored_info.contains('`'))
        {
            return Err(Message {
                place: None,
                reason: "Frontmatter requires a valid fence and a single-line info string; code fences require a language hint".into(),
                rule_id: Box::new("invalid-frontmatter".into()),
                source: Box::new("mdast-util-to-markdown".into()),
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
        let mut output = format!(
            "{}{}{}\n",
            marker.to_string().repeat(opening_length),
            separator,
            info
        );
        output.push_str(body);
        if !body.is_empty() && !body.ends_with(['\r', '\n']) {
            output.push('\n');
        }
        output.push_str(&marker.to_string().repeat(closing_length));
        Ok(output)
    }
}

fn item_task<S: AsRef<str>>(document: &Document<S>, item: NodeId) -> Option<(NodeId, bool)> {
    if !matches!(
        document.node(item)?.kind,
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
        document.node(paragraph)?.kind,
        EventKind::Start(Tag::Paragraph { .. })
    ) {
        return None;
    }
    let marker = document.children(paragraph).next()?;
    match document.node(marker)?.kind {
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
