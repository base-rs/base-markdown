// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Markdown construct formatting adapted from `mdast-util-to-markdown`.
use super::{
    configure::IndentOptions,
    construct_name::ConstructName,
    state::{flow_children, list_number_too_long, Info, State, MAX_LIST_NUMBER},
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
    util::line_ending::trim_terminal_ending,
    CodeFence, Document, NodeId, NodeSyntax, ReferenceKind,
};
use alloc::{borrow::Cow, string::String, vec::Vec};
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

    /// A break is written as a backslash and a line ending. Of the headings, only a setext one
    /// holds a break ([`crate::stream::Tag::Heading`]), and a heading of rank 1 or 2 that holds one
    /// is written setext (`format_heading_as_setext`), so a break is never written in an ATX
    /// heading, where a line ending would end it.
    ///
    /// The break is written as `\` before the line ending, or, where the document's options turn
    /// `Constructs::hard_break_escape` off, as two spaces, after `before_hard_break` has encoded
    /// a space or tab that the text before it ends in.
    pub(super) fn hard_break(&mut self) {
        if self.parse.constructs.hard_break_escape {
            self.output.push_str("\\\n");
            return;
        }
        self.output.push_str("  \n");
    }

    /// Encode the space or tab that the output ends in, before a hard break written with trailing
    /// spaces: the parser drops the spaces and tabs before the break, so the text's last one is
    /// encoded. This is done before the break is written, rather than as it is, because the break
    /// is also written to see its first character, and only what it writes is then removed.
    pub(super) fn before_hard_break(&mut self) {
        if self.parse.constructs.hard_break_escape {
            return;
        }
        // A backslash before the space would escape the reference's `&`, so it is escaped.
        let end = self.output.len().saturating_sub(1);
        if self.output.ends_with([' ', '\t']) && super::attention::escapes_next(&self.output[..end])
        {
            self.output.insert(end, '\\');
        }
        match self.output.chars().next_back() {
            Some(' ') => self.output.replace_range(self.output.len() - 1.., "&#x20;"),
            Some('\t') => self.output.replace_range(self.output.len() - 1.., "&#x9;"),
            _ => {}
        }
    }

    /// `value` is the stored value, whose lines each end in a line ending except perhaps the last, so a body of one blank line is `"\n"` and an empty body is `""`.
    /// `fence` is whether the code's place needs a fence whatever the options, as from
    /// `fences_code`.
    pub(super) fn code(
        &mut self,
        value: &str,
        info: Option<&str>,
        fence: bool,
    ) -> Result<(), Message> {
        let content = trim_terminal_ending(value);
        let mut marker = self.options.check_fence()?;
        // A backtick fence's info holds no backtick, which is encoded as a reference, so where
        // the options read no references the fence is tildes.
        if marker == '`'
            && !self.parse.constructs.character_reference
            && info.is_some_and(|info| info.contains('`'))
        {
            marker = '~';
        }
        let start = self.output.len();
        if !fence && format_code_as_indented(content, info.is_none(), self) {
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
        self.output.extend(core::iter::repeat_n(marker, size));
        if let Some(info) = info {
            self.enter(if marker == '`' {
                ConstructName::CodeFencedInfoGraveAccent
            } else {
                ConstructName::CodeFencedInfoTilde
            });
            // The info string is its own context, after the fence, so that a `~` at its start in
            // a tilde fence is escaped rather than lengthening the fence, and the spaces and tabs
            // at its edges, which the parser trims, are encoded.
            self.write_safe(info, &Info::new("", ""), Some('`'));
            self.exit();
        }
        self.output.push('\n');
        if !value.is_empty() {
            // Grow once for the body, closing fence and terminating line ending.
            self.output.reserve(content.len() + size + 2);
            self.output.push_str(content);
            self.output.push('\n');
        }
        self.output.extend(core::iter::repeat_n(marker, size));
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
        self.output.push('[');
        self.write_label(label);
        self.output.push_str("]: ");
        self.destination_title(
            start,
            destination,
            title,
            quote,
            destination.is_empty() || raw_destination_unsafe(destination),
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
        if !core::mem::take(&mut self.atx_heading) && format_heading_as_setext(document, node, self)
        {
            self.enter(ConstructName::Phrasing);
            self.container_phrasing(document, node, &Info::new("\n", "\n"))?;
            self.exit();
            let content = &self.output[start..];
            let last_line = content.rfind('\n').map_or(0, |index| index + 1);
            let size = content.len() - last_line;
            self.output.push('\n');
            self.output.extend(core::iter::repeat_n(
                if rank == 1 { '=' } else { '-' },
                size,
            ));
            return Ok(());
        }
        self.enter(ConstructName::HeadingAtx);
        self.enter(ConstructName::Phrasing);
        self.output
            .extend(core::iter::repeat_n('#', usize::from(rank)));
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
                .extend(core::iter::repeat_n('#', usize::from(rank)));
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
        if self.is_task_paragraph(document, node) {
            let item = document.node(node).unwrap().parent().unwrap();
            if flow_children(document, item).next() != Some(node) {
                let checked = matches!(
                    document.node(item).unwrap().kind,
                    EventKind::Start(Tag::Item {
                        checked: Some(true),
                        ..
                    })
                );
                let (task, separator) = checkbox(document, node, checked);
                self.output.push_str(task);
                self.output.push(separator);
            }
        }
        self.enter(ConstructName::Phrasing);
        self.container_phrasing(document, node, info)?;
        self.exit();
        Ok(())
    }

    pub(super) fn thematic_break(&mut self) -> Result<(), Message> {
        let mut marker = self.options.check_rule()?;
        if core::mem::take(&mut self.rule_after_text) && marker == '-' {
            marker = '*';
        }
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
        let size = super::util::scan::delimiter_size(content, b'`');
        self.output.extend(core::iter::repeat_n('`', size));
        // The parser strips one space from each side when the content starts and ends with one and is not only spaces, so a tab is not a space here. Other content with whitespace at both edges is padded too, which reads back the same. The content holds no line ending, which the parser would read as a space ([`EventKind::Code`][crate::stream::EventKind::Code]).
        let pad = !content.chars().all(|character| character == ' ')
            && (content.starts_with(' ') && content.ends_with(' ')
                || !content.chars().all(char::is_whitespace)
                    && content.starts_with(char::is_whitespace)
                    && content.ends_with(char::is_whitespace)
                || content.starts_with('`')
                || content.ends_with('`'));
        if pad {
            self.output.push(' ');
        }
        // GFM splits table cells before parsing code, so its pipes need escapes too.
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
        self.output.extend(core::iter::repeat_n('`', size));
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
                || self.alt_start == Some(self.output.len())
                || self.output.chars().next_back().is_none_or(boundary);
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
            destination.is_empty() && title.is_some() || raw_destination_unsafe(destination),
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
        after: &str,
    ) -> Result<bool, Message> {
        let start = self.output.len();
        let text = self.link_label(document, node, image, "]")?;
        self.output.push_str("][");
        let reference_start = self.output.len();
        self.write_label(label);
        if self.stack.contains(&ConstructName::TableCell) {
            escape_table_pipes(&mut self.output, reference_start);
        }
        if matches!(kind, ReferenceKind::Full)
            || text.is_empty()
            || self.output[text] != self.output[reference_start..]
        {
            // The parser reads a shortcut or collapsed reference's label at any size, but a full
            // reference's only as long as `label_fits` allows: one that is longer is not written.
            if !matches!(kind, ReferenceKind::Full) && !crate::node_syntax::label_fits(label) {
                self.output.truncate(start);
                return Ok(false);
            }
            self.output.push(']');
        } else {
            self.output.truncate(reference_start);
            // A shortcut reference followed by `[`, such as another reference, would read back
            // as a full reference with what follows as its label, so it is written collapsed.
            if matches!(kind, ReferenceKind::Shortcut) && !after.starts_with('[') {
                self.output.pop();
                if !image {
                    self.label_end = Some((self.output.len(), false));
                }
            } else {
                self.output.push(']');
            }
        }
        Ok(true)
    }

    fn link_label<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        node: NodeId,
        image: bool,
        after: &str,
    ) -> Result<core::ops::Range<usize>, Message> {
        // Image text is phrasing like link text, but literals form in it, unless the image is in
        // link text, and so does a bare link at its start, which reads back as the same link.
        let literals = image && super::escape::autolink_literals(&self.stack);
        self.enter(if image {
            ConstructName::ImageLabel
        } else {
            ConstructName::Label
        });
        let start = self.output.len();
        self.output.push_str(if image { "![" } else { "[" });
        let text_start = self.output.len();
        let alt_start = mem::replace(
            &mut self.alt_start,
            if literals { Some(text_start) } else { None },
        );
        let info = Info::at(start, text_start, after);
        let result = self.container_phrasing(document, node, &info);
        self.alt_start = alt_start;
        result?;
        // `![^` starts no image, as `[^` reads as a footnote call or text instead, and link text
        // that starts with `^` would read back as a footnote call where a definition binds it,
        // in a dialect with footnote calls and definitions (#215). Without
        // `Constructs::label_start_link`, only `[^` starts a link, one that no definition binds,
        // so it is not escaped there.
        let constructs = &self.parse.constructs;
        let footnotes = constructs.label_start_footnote && constructs.footnote_definition;
        if (image || footnotes && constructs.label_start_link)
            && self.output[text_start..].starts_with('^')
        {
            self.output.insert(text_start, '\\');
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
        // The first line of code that an item starts with, which written indented is on the
        // marker's line.
        let mut code_lines = Vec::new();
        // The content model holds only items in a list.
        for (index, child) in flow_children(document, node).enumerate() {
            let first = flow_children(document, child).next();
            if index == 0 {
                first_empty = first.is_none();
            }
            if let Some(first) = first {
                match &document.node(first).unwrap().kind {
                    EventKind::Rule => has_rule = true,
                    EventKind::CodeBlock { value, .. } => {
                        let value = value.resolve(document.source()).unwrap();
                        code_lines.push(value.split('\n').next().unwrap_or_default());
                    }
                    _ => {}
                }
            }
        }
        // Indented code of `--` after a `-` marker would make the line a thematic break.
        let breaks = |marker: char| {
            code_lines.iter().any(|line| {
                line.chars().all(|c| c == marker || c == ' ' || c == '\t')
                    && line.chars().filter(|&c| c == marker).count() >= 2
            })
        };
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
            if self.options.check_rule()? == bullet && has_rule || breaks(bullet) {
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
                // Each item's number is checked as it is written, so a list costs O(1) per item.
                // `u64` holds any `u32` start plus any item position without wrapping.
                let number = u64::from(start).saturating_add(position as u64);
                if number > MAX_LIST_NUMBER {
                    return Err(list_number_too_long(number));
                }
                write!(self.output, "{number}").expect("writing to a String cannot fail");
            }
        }
        self.output.push(bullet);
        // The checkbox starts the item's content, as in `mdast-util-gfm-task-list-item`, so continuation lines indent to the content start the parser uses, which excludes the checkbox.
        // The text after the marker starts with the space or tab that separates them, which the text writer drops, so the marker is written with that character and a tab reads back as one.
        // A paragraph after definitions writes its checkbox itself, on its own line, as the
        // parser reads no definition after a checkbox.
        let item_task = item_task(document, node);
        let task = item_task
            .filter(|&(paragraph, _)| flow_children(document, node).next() == Some(paragraph))
            .map(|(paragraph, checked)| checkbox(document, paragraph, checked));
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
        // An HTML block keeps up to three spaces of indentation, which after a marker would move
        // the content's column, so an item that starts with one starts with a blank line, and the
        // block is written on the next, indented as the lines after it.
        let blank_start = flow_children(document, node).next().is_some_and(|first| {
            matches!(&document.node(first).unwrap().kind, EventKind::Html { value, block: true }
                if value.resolve(document.source()).unwrap().starts_with(' '))
        });
        self.enter(ConstructName::ListItem);
        let start = self.output.len();
        let task_item = self
            .task_item
            .replace((node, item_task.map(|(paragraph, _)| paragraph)));
        self.container_flow(document, node)?;
        self.task_item = task_item;
        self.indent_lines(start, |line, index, blank, output| {
            if index == 0 {
                if let Some((task, separator)) = task {
                    output.extend(core::iter::repeat_n(' ', size - prefix_size));
                    // A task item's first line holds its paragraph, so it is never blank.
                    output.push_str(task);
                    output.push(separator);
                    output.push_str(line);
                    return;
                }
            }
            if !blank {
                let indent = if index > 0 {
                    size
                } else if blank_start {
                    output.push('\n');
                    size
                } else {
                    size - prefix_size
                };
                output.extend(core::iter::repeat_n(' ', indent));
            }
            output.push_str(line);
        });
        self.exit();
        Ok(())
    }

    /// Whether `node` is the paragraph that a task item's checkbox is written before. Finding an
    /// item's task paragraph reads past the definitions that start the item, so the paragraph of
    /// the item being written is found once, rather than for each node that asks (#161).
    pub(super) fn is_task_paragraph<S: AsRef<str>>(
        &self,
        document: &Document<S>,
        node: NodeId,
    ) -> bool {
        let Some(item) = document.node(node).unwrap().parent() else {
            return false;
        };
        let paragraph = match self.task_item {
            Some((written, paragraph)) if written == item => paragraph,
            _ => item_task(document, item).map(|(paragraph, _)| paragraph),
        };
        paragraph == Some(node)
    }

    pub(super) fn item_spread<S: AsRef<str>>(&self, document: &Document<S>, node: NodeId) -> bool {
        matches!(
            document.syntax(node),
            Some(NodeSyntax::ListItem { spread: true })
        )
    }

    /// Writes frontmatter with `---` or `+++`, the only fences that read back as frontmatter. A
    /// fence of another character, from a hand-built node or syntax recorded before code-fence
    /// frontmatter was removed, is written with `---`, keeping its info as the format hint. A body
    /// with a line that would close the fence switches to the other one, naming the format the
    /// fence implied when there is no hint, and a body that would close both cannot be written.
    /// The body is written as it is: a valid body holds no CR and, unless it is empty, ends in a
    /// line feed, as the parser stores it, so its last line ends before the closing fence.
    pub(super) fn frontmatter(
        &mut self,
        body: &str,
        authored_info: &str,
        fence: CodeFence,
    ) -> Result<(), Message> {
        if authored_info.contains(['\r', '\n']) {
            return Err(Message {
                span: None,
                reason: "Frontmatter info must be a single line".into(),
                rule_id: "invalid-frontmatter",
                source: "mdast-util-to-markdown",
            });
        }
        let mut info = authored_info.trim_matches([' ', '\t']);
        let closes = |marker: char| {
            body.split('\n').any(|line| {
                let line = line.trim_end_matches([' ', '\t']);
                line.len() == 3 && line.chars().all(|character| character == marker)
            })
        };
        let preferred = if fence.character == '+' { '+' } else { '-' };
        let other = if preferred == '+' { '-' } else { '+' };
        let marker = if !closes(preferred) {
            preferred
        } else if !closes(other) {
            if info.split_ascii_whitespace().next().is_none() {
                info = if preferred == '-' { "yaml" } else { "toml" };
            }
            other
        } else {
            return Err(Message {
                span: None,
                reason: "Frontmatter cannot be written when its body has both a `---` line and a `+++` line".into(),
                rule_id: "invalid-frontmatter",
                source: "mdast-util-to-markdown",
            });
        };
        let fence = if marker == '+' { "+++" } else { "---" };
        self.output.push_str(fence);
        if !info.is_empty() {
            self.output.push(' ');
            self.output.push_str(info);
        }
        self.output.push('\n');
        self.output.reserve(body.len() + fence.len() + 1);
        self.output.push_str(body);
        self.output.push_str(fence);
        Ok(())
    }
}

// The paragraph that the checkbox of task item `item` is written before, its first block other
// than definitions, and whether it is checked.
/// The checkbox of a task item whose task paragraph is `paragraph`, and the space or tab after it,
/// which is the one the paragraph's text starts with.
fn checkbox<S: AsRef<str>>(
    document: &Document<S>,
    paragraph: NodeId,
    checked: bool,
) -> (&'static str, char) {
    let separator = document
        .node(paragraph)
        .unwrap()
        .first_child()
        .and_then(|id| match &document.node(id).unwrap().kind {
            EventKind::Text(text) => text.resolve(document.source()).ok(),
            _ => None,
        })
        .filter(|value| value.starts_with('\t'))
        .map_or(' ', |_| '\t');
    (if checked { "[x]" } else { "[ ]" }, separator)
}

fn item_task<S: AsRef<str>>(document: &Document<S>, item: NodeId) -> Option<(NodeId, bool)> {
    let EventKind::Start(Tag::Item {
        checked: Some(checked),
        ..
    }) = document.node(item).expect("task item node exists").kind
    else {
        return None;
    };
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
    Some((paragraph, checked))
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

/// Whether a destination needs the `<…>` form: a raw destination cannot hold whitespace or controls, and one starting with `<` is read as the bracketed form, so `<a>` written raw reads back as `a`.
fn raw_destination_unsafe(destination: &str) -> bool {
    destination.starts_with('<') || contains_control_or_whitespace(destination)
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
