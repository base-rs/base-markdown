// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! One HTML writer for parser events and document traversal.
use crate::stream::{Event, EventKind, StreamError, Tag, TagKind};
use crate::util::{
    constant::{SAFE_PROTOCOL_HREF, SAFE_PROTOCOL_SRC},
    encode::{find_escape, write_escaped},
    sanitize_uri::{sanitize, write_sanitized},
};
use crate::{CompileOptions, LineEnding};
use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use core::{
    convert::TryFrom,
    fmt::{self, Write},
};

// Every u64, including the supported targets' usize counters, fits in 20 decimal digits.
fn decimal(mut value: u64, buffer: &mut [u8; 20]) -> &str {
    let mut start = buffer.len();
    loop {
        start -= 1;
        buffer[start] = b'0' + u8::try_from(value % 10).expect("a decimal digit fits in u8");
        value /= 10;
        if value == 0 {
            break;
        }
    }
    core::str::from_utf8(&buffer[start..]).expect("decimal digits are ASCII")
}

struct Frame {
    kind: TagKind,
    flag: bool,
    level: u8,
    end: usize,
}
struct HeadingCapture {
    output: String,
    text: String,
}
struct FootnoteCall {
    key: String,
    id: String,
    count: usize,
    definition: Option<usize>,
}

struct Capture {
    key: String,
    output: String,
    previous: Option<u8>,
}

/// Render semantic events directly into a caller-owned writer.
///
/// Parser output and [`crate::Document::events`] use exactly this renderer. The renderer retains nesting state and footnote bodies, never the full event stream.
pub struct EventHtmlRenderer<'a, W> {
    output: &'a mut W,
    source: &'a str,
    options: &'a CompileOptions,
    line_ending: LineEnding,
    last: Option<u8>,
    stack: Vec<Frame>,
    images: usize,
    image_titles: Vec<String>,
    captures: Vec<Capture>,
    notes: Vec<(String, String)>,
    calls: Vec<FootnoteCall>,
    started: bool,
    ended: bool,
    heading: Option<HeadingCapture>,
    anchors: crate::headings::AnchorGenerator,
}

impl<'a, W: Write> EventHtmlRenderer<'a, W> {
    /// Construct a renderer writing directly into `output`.
    pub fn new(source: &'a str, options: &'a CompileOptions, output: &'a mut W) -> Self {
        Self {
            output,
            source,
            options,
            line_ending: options.default_line_ending.clone(),
            last: None,
            stack: Vec::new(),
            images: 0,
            image_titles: Vec::new(),
            captures: Vec::new(),
            notes: Vec::new(),
            calls: Vec::new(),
            started: false,
            ended: false,
            heading: None,
            anchors: crate::headings::AnchorGenerator::default(),
        }
    }

    // Paired mixed-document benchmarks justify inlining these small writes.
    #[allow(clippy::inline_always)]
    #[inline(always)]
    fn write(&mut self, value: &str) -> Result<(), StreamError> {
        if let Some(heading) = self.heading.as_mut() {
            heading.output.push_str(value);
        } else if let Some(capture) = self.captures.last_mut() {
            capture.output.push_str(value);
        } else {
            self.output.write_str(value)?;
        }
        if let Some(&byte) = value.as_bytes().last() {
            self.last = Some(byte);
        }
        Ok(())
    }

    fn escaped(&mut self, value: &str, encode: bool) -> Result<(), StreamError> {
        let index = find_escape(value.as_bytes(), encode);
        // Ordinary spans need only one write; select a destination for the replacement loop only when an escape is present.
        if index == value.len() {
            return self.write(value);
        }
        if let Some(heading) = self.heading.as_mut() {
            write_escaped(&mut heading.output, &mut self.last, value, encode, index)
        } else if let Some(capture) = self.captures.last_mut() {
            write_escaped(&mut capture.output, &mut self.last, value, encode, index)
        } else {
            write_escaped(self.output, &mut self.last, value, encode, index)
        }
        .map_err(StreamError::Write)
    }

    fn newline(&mut self) -> Result<(), StreamError> {
        let value = self.line_ending.clone();
        self.write(value.as_str())
    }
    fn number(&mut self, value: u64) -> Result<(), StreamError> {
        self.write(decimal(value, &mut [0; 20]))
    }
    fn terminate(&mut self) -> Result<(), StreamError> {
        if !matches!(self.last, None | Some(b'\r' | b'\n')) {
            self.newline()?;
        }
        Ok(())
    }

    fn destination(&mut self, value: &str, image: bool) -> Result<(), StreamError> {
        let protocols =
            if self.options.allow_dangerous_protocol || (image && self.options.allow_any_img_src) {
                None
            } else if image {
                Some(SAFE_PROTOCOL_SRC.as_slice())
            } else {
                Some(SAFE_PROTOCOL_HREF.as_slice())
            };
        write_sanitized(value, protocols, |value| self.write(value))
    }

    fn start(&mut self, tag: &Tag<'_>, end: usize) -> Result<(), StreamError> {
        let kind = tag.kind();
        if kind == TagKind::Document {
            if self.started || !self.stack.is_empty() {
                return Err(StreamError::Unbalanced);
            }
            self.started = true;
        } else if self.stack.is_empty() {
            return Err(StreamError::Unbalanced);
        }
        if matches!(kind, TagKind::Heading | TagKind::FootnoteDefinition)
            && self
                .stack
                .iter()
                .any(|frame| !frame.kind.allows_descendant(kind))
        {
            return Err(StreamError::Unbalanced);
        }
        let mut frame = Frame {
            kind,
            flag: false,
            level: 0,
            end,
        };
        // Tags inside image alternative text contribute only their text content.
        let tags = self.images == 0;
        if !tags
            && matches!(
                kind,
                TagKind::Table
                    | TagKind::TableHead
                    | TagKind::TableBody
                    | TagKind::TableRow
                    | TagKind::TableCell
            )
        {
            self.stack.push(frame);
            return Ok(());
        }
        match tag {
            Tag::Document(line_ending) => {
                if let Some(value) = line_ending {
                    self.line_ending = value.clone();
                }
            }
            Tag::Paragraph { tight } => {
                frame.flag = *tight;
                if tags && !tight {
                    self.terminate()?;
                    self.write("<p>")?;
                }
            }
            Tag::Heading(level) => {
                frame.level = *level;
                self.terminate()?;
                if self.options.heading_ids {
                    self.heading = Some(HeadingCapture {
                        output: String::new(),
                        text: String::new(),
                    });
                } else {
                    self.write(
                        ["<h1>", "<h2>", "<h3>", "<h4>", "<h5>", "<h6>"][usize::from(*level - 1)],
                    )?;
                }
            }
            Tag::BlockQuote => {
                if tags {
                    self.terminate()?;
                    self.write("<blockquote>")?;
                }
            }
            Tag::List { start, .. } => {
                frame.flag = start.is_some();
                if tags {
                    self.terminate()?;
                    match start {
                        None => self.write("<ul>")?,
                        Some(1) => self.write("<ol>")?,
                        Some(start) => {
                            self.write("<ol start=\"")?;
                            self.number(u64::from(*start))?;
                            self.write("\">")?;
                        }
                    }
                }
            }
            Tag::Item { block_end } => {
                frame.flag = *block_end;
                if tags {
                    self.terminate()?;
                    self.write("<li>")?;
                }
            }
            Tag::Emphasis => {
                if tags {
                    self.write("<em>")?;
                }
            }
            Tag::Strong => {
                if tags {
                    self.write("<strong>")?;
                }
            }
            Tag::Delete => {
                if tags {
                    self.write("<del>")?;
                }
            }
            Tag::Link { destination, title } => {
                if tags {
                    self.write("<a href=\"")?;
                    self.destination(
                        destination
                            .resolve(self.source)
                            .expect("event values are source-valid"),
                        false,
                    )?;
                    if let Some(title) = title {
                        self.write("\" title=\"")?;
                        self.escaped(
                            title
                                .resolve(self.source)
                                .expect("event values are source-valid"),
                            true,
                        )?;
                    }
                    self.write("\">")?;
                }
            }
            Tag::Image { destination, title } => {
                frame.flag = tags;
                if tags {
                    self.write("<img src=\"")?;
                    self.destination(
                        destination
                            .resolve(self.source)
                            .expect("event values are source-valid"),
                        true,
                    )?;
                    self.write("\" alt=\"")?;
                }
                // The title belongs after alt. Keep it in the frame's separate capture only when present.
                if tags {
                    if let Some(title) = title {
                        frame.level = 1;
                        let mut output = String::new();
                        crate::util::encode::encode_into(
                            title
                                .resolve(self.source)
                                .expect("event values are source-valid"),
                            true,
                            &mut output,
                        );
                        self.image_titles.push(output);
                    }
                }
                self.images += 1;
            }
            Tag::Table => {
                self.terminate()?;
                self.write("<table>")?;
            }
            Tag::TableHead => {
                self.terminate()?;
                self.write("<thead>")?;
            }
            Tag::TableBody => self.write("<tbody>")?,
            Tag::TableRow => {
                self.terminate()?;
                self.write("<tr>")?;
            }
            Tag::TableCell { alignment, header } => {
                frame.flag = *header;
                self.terminate()?;
                self.write(match (header, alignment) {
                    (true, crate::AlignKind::Left) => "<th align=\"left\">",
                    (true, crate::AlignKind::Right) => "<th align=\"right\">",
                    (true, crate::AlignKind::Center) => "<th align=\"center\">",
                    (true, crate::AlignKind::None) => "<th>",
                    (false, crate::AlignKind::Left) => "<td align=\"left\">",
                    (false, crate::AlignKind::Right) => "<td align=\"right\">",
                    (false, crate::AlignKind::Center) => "<td align=\"center\">",
                    (false, crate::AlignKind::None) => "<td>",
                })?;
            }
            Tag::FootnoteDefinition(label) => {
                let key = label
                    .resolve(self.source)
                    .expect("event values are source-valid")
                    .to_string();
                self.captures.push(Capture {
                    key,
                    output: String::new(),
                    previous: self.last,
                });
                self.last = None;
            }
        }
        self.stack.push(frame);
        Ok(())
    }

    fn end(&mut self, kind: TagKind, end: usize) -> Result<(), StreamError> {
        let frame = self.stack.pop().ok_or(StreamError::Unbalanced)?;
        if frame.kind != kind {
            return Err(StreamError::Unbalanced);
        }
        if end < frame.end {
            return Err(StreamError::InvalidRange);
        }
        if kind == TagKind::Image {
            self.images -= 1;
            if frame.flag {
                if frame.level != 0 {
                    let title = self.image_titles.pop().unwrap();
                    self.write("\" title=\"")?;
                    self.write(&title)?;
                }
                self.write("\" />")?;
            }
            return Ok(());
        }
        if self.images != 0 {
            return Ok(());
        }
        match kind {
            TagKind::Document => {
                self.footnotes()?;
                self.ended = true;
            }
            TagKind::Paragraph => {
                if !frame.flag {
                    self.write("</p>")?;
                }
            }
            TagKind::Heading => {
                if let Some(heading) = self.heading.take() {
                    let text = crate::util::line_ending::trim_terminal_ending(&heading.text);
                    let anchor = self.anchors.anchor_for(text);
                    self.write(
                        [
                            "<h1 id=\"",
                            "<h2 id=\"",
                            "<h3 id=\"",
                            "<h4 id=\"",
                            "<h5 id=\"",
                            "<h6 id=\"",
                        ][usize::from(frame.level - 1)],
                    )?;
                    self.escaped(&anchor, true)?;
                    self.write("\">")?;
                    self.write(&heading.output)?;
                }
                self.write(
                    ["</h1>", "</h2>", "</h3>", "</h4>", "</h5>", "</h6>"]
                        [usize::from(frame.level - 1)],
                )?;
            }
            TagKind::BlockQuote => {
                self.terminate()?;
                self.write("</blockquote>")?;
            }
            TagKind::List => {
                self.newline()?;
                self.write(if frame.flag { "</ol>" } else { "</ul>" })?;
            }
            TagKind::Item => {
                if frame.flag {
                    self.terminate()?;
                }
                self.write("</li>")?;
            }
            TagKind::Emphasis => self.write("</em>")?,
            TagKind::Strong => self.write("</strong>")?,
            TagKind::Delete => self.write("</del>")?,
            TagKind::Link => self.write("</a>")?,
            TagKind::Table => {
                self.terminate()?;
                self.write("</table>")?;
            }
            TagKind::TableHead => {
                self.terminate()?;
                self.write("</thead>")?;
            }
            TagKind::TableBody => {
                self.terminate()?;
                self.write("</tbody>")?;
            }
            TagKind::TableRow => {
                self.terminate()?;
                self.write("</tr>")?;
            }
            TagKind::TableCell => self.write(if frame.flag { "</th>" } else { "</td>" })?,
            TagKind::FootnoteDefinition => {
                let capture = self.captures.pop().unwrap();
                self.last = capture.previous;
                self.notes.push((capture.key, capture.output));
            }
            TagKind::Image => {
                unreachable!("images were handled before suppressing tags in alt text")
            }
        }
        Ok(())
    }

    /// Pull an entire event source into the output writer.
    ///
    /// # Errors
    /// Propagates parse, source-range, malformed-stream, and output errors.
    pub fn render<'e>(
        &mut self,
        events: impl IntoIterator<Item = Result<Event<'e>, StreamError>>,
    ) -> Result<(), StreamError> {
        self.render_inner::<true>(events)
    }

    /// Internal parser rendering uses the producer's range and tag invariants.
    /// Keep arbitrary public event sources on the checked path above.
    pub(crate) fn render_markdown(
        &mut self,
        options: &crate::ParseOptions,
    ) -> Result<(), StreamError> {
        let events = crate::EventParser::new(self.source, options);
        self.render_inner::<false>(events)
    }

    fn render_inner<'e, const CHECK: bool>(
        &mut self,
        events: impl IntoIterator<Item = Result<Event<'e>, StreamError>>,
    ) -> Result<(), StreamError> {
        for event in events {
            self.event_ref::<CHECK>(&event?)?;
        }
        if !self.started || !self.ended || !self.stack.is_empty() {
            return Err(StreamError::Unbalanced);
        }
        Ok(())
    }

    /// Consume one event without retaining it.
    ///
    /// # Errors
    /// Propagates invalid source ranges, malformed streams, and writer errors.
    // Consume the public event while borrowing it through internal helpers to avoid copies.
    #[allow(clippy::needless_pass_by_value)]
    pub fn event(&mut self, event: Event<'_>) -> Result<(), StreamError> {
        self.event_ref::<true>(&event)
    }

    // Paired mixed-document benchmarks justify inlining this event boundary.
    #[allow(clippy::inline_always)]
    #[inline(always)]
    fn event_ref<const CHECK: bool>(&mut self, event: &Event<'_>) -> Result<(), StreamError> {
        if self.ended {
            return Err(StreamError::Unbalanced);
        }
        if CHECK {
            event.validate(Some(self.source))?;
        }
        if let Some(heading) = &mut self.heading {
            crate::headings::append_text::<true>(&event.kind, self.source, &mut heading.text);
        }
        match &event.kind {
            EventKind::Start(tag) => self.start(tag, event.span.end),
            EventKind::End(kind) => self.end(*kind, event.span.end),
            kind => {
                if self.stack.is_empty() {
                    return Err(StreamError::Unbalanced);
                }
                match kind {
                    EventKind::Text(value) | EventKind::SoftBreak(value) => self.escaped(
                        value
                            .resolve(self.source)
                            .expect("event values are source-valid"),
                        true,
                    ),
                    EventKind::Separator { value, conditional } => {
                        if *conditional {
                            self.terminate()
                        } else {
                            self.escaped(
                                value
                                    .resolve(self.source)
                                    .expect("event values are source-valid"),
                                true,
                            )
                        }
                    }
                    EventKind::HardBreak => {
                        if self.images == 0 {
                            self.write("<br />")
                        } else {
                            Ok(())
                        }
                    }
                    EventKind::Rule => {
                        if self.images == 0 {
                            self.terminate()?;
                            self.write("<hr />")
                        } else {
                            Ok(())
                        }
                    }
                    EventKind::Code { value, language } => {
                        if self.images == 0 {
                            if let Some(language) = language {
                                self.write("<code class=\"language-")?;
                                self.escaped(
                                    language
                                        .resolve(self.source)
                                        .expect("event values are source-valid"),
                                    true,
                                )?;
                                self.write("\">")?;
                            } else {
                                self.write("<code>")?;
                            }
                        }
                        self.escaped(
                            value
                                .resolve(self.source)
                                .expect("event values are source-valid"),
                            true,
                        )?;
                        if self.images == 0 {
                            self.write("</code>")?;
                        }
                        Ok(())
                    }
                    EventKind::Math(value) => {
                        if self.images == 0 {
                            self.write("<code class=\"language-math math-inline\">")?;
                        }
                        self.escaped(
                            value
                                .resolve(self.source)
                                .expect("event values are source-valid"),
                            true,
                        )?;
                        if self.images == 0 {
                            self.write("</code>")?;
                        }
                        Ok(())
                    }
                    EventKind::CodeBlock {
                        value,
                        language,
                        math,
                        unclosed,
                        ..
                    } => {
                        if self.images != 0 {
                            return self.escaped(
                                value
                                    .resolve(self.source)
                                    .expect("event values are source-valid"),
                                true,
                            );
                        }
                        self.terminate()?;
                        if *math {
                            self.write("<pre><code class=\"language-math math-display\">")?;
                        } else if let Some(language) = language {
                            self.write("<pre><code class=\"language-")?;
                            self.escaped(
                                language
                                    .resolve(self.source)
                                    .expect("event values are source-valid"),
                                true,
                            )?;
                            self.write("\">")?;
                        } else {
                            self.write("<pre><code>")?;
                        }
                        let value = value
                            .resolve(self.source)
                            .expect("event values are source-valid");
                        self.escaped(value, true)?;
                        if !value.is_empty() {
                            self.terminate()?;
                        }
                        self.write("</code></pre>")?;
                        if *unclosed {
                            self.terminate()?;
                        }
                        Ok(())
                    }
                    EventKind::Html { value, block } => {
                        if *block {
                            self.terminate()?;
                        }
                        let value = value
                            .resolve(self.source)
                            .expect("event values are source-valid");
                        if self.options.allow_dangerous_html && self.options.tagfilter {
                            crate::util::tagfilter::write_filtered(value, |part| {
                                self.escaped(part, false)
                            })
                        } else {
                            self.escaped(value, !self.options.allow_dangerous_html)
                        }
                    }
                    EventKind::TaskMarker(checked) => {
                        if self.images == 0 {
                            self.write(match (self.options.task_list_item_checkable, checked) {
                                (false, false) => "<input type=\"checkbox\" disabled=\"\" />",
                                (false, true) => {
                                    "<input type=\"checkbox\" disabled=\"\" checked=\"\" />"
                                }
                                (true, false) => "<input type=\"checkbox\" />",
                                (true, true) => "<input type=\"checkbox\" checked=\"\" />",
                            })?;
                        }
                        Ok(())
                    }
                    EventKind::Definition { .. } | EventKind::Opaque { .. } => Ok(()),
                    EventKind::FootnoteReference(value) => self.footnote_call(
                        value
                            .resolve(self.source)
                            .expect("event values are source-valid"),
                    ),
                    EventKind::Start(_) | EventKind::End(_) => {
                        unreachable!("containers were handled above")
                    }
                }
            }
        }
    }

    fn prefix(&mut self) -> Result<(), StreamError> {
        self.escaped(
            self.options
                .footnote_clobber_prefix
                .as_deref()
                .unwrap_or("user-content-"),
            true,
        )
    }

    fn footnote_call(&mut self, key: &str) -> Result<(), StreamError> {
        let index = self
            .calls
            .iter()
            .position(|call| call.key == key)
            .unwrap_or_else(|| {
                self.calls.push(FootnoteCall {
                    key: key.into(),
                    id: sanitize(&key.to_lowercase()),
                    count: 0,
                    definition: None,
                });
                self.calls.len() - 1
            });
        let call = &mut self.calls[index];
        call.count += 1;
        let count = call.count;
        if self.images != 0 {
            return Ok(());
        }
        // Temporarily move the cached ID out so writes can borrow the renderer. Restore it even when the caller's writer fails.
        let safe = core::mem::take(&mut call.id);
        let result = self.write_footnote_call(&safe, index + 1, count);
        self.calls[index].id = safe;
        result
    }

    fn write_footnote_call(
        &mut self,
        safe: &str,
        number: usize,
        count: usize,
    ) -> Result<(), StreamError> {
        self.write("<sup><a href=\"#")?;
        self.prefix()?;
        self.write("fn-")?;
        self.write(safe)?;
        self.write("\" id=\"")?;
        self.prefix()?;
        self.write("fnref-")?;
        self.write(safe)?;
        if count > 1 {
            self.write("-")?;
            self.number(count as u64)?;
        }
        self.write("\" data-footnote-ref=\"\" aria-describedby=\"footnote-label\">")?;
        self.number(number as u64)?;
        self.write("</a></sup>")
    }

    fn backreferences(&mut self, key: &str, count: usize) -> Result<(), StreamError> {
        for reference in 1..=count {
            if reference != 1 {
                self.write(" ")?;
            }
            let mut buffer = [0; 20];
            let number = (reference > 1).then(|| decimal(reference as u64, &mut buffer));
            self.write("<a href=\"#")?;
            self.prefix()?;
            self.write("fnref-")?;
            self.write(key)?;
            if let Some(number) = number {
                self.write("-")?;
                self.write(number)?;
            }
            self.write("\" data-footnote-backref=\"\" aria-label=\"")?;
            self.escaped(
                self.options
                    .footnote_back_label
                    .as_deref()
                    .unwrap_or("Back to content"),
                true,
            )?;
            self.write("\" class=\"data-footnote-backref\">↩")?;
            if let Some(number) = number {
                self.write("<sup>")?;
                self.write(number)?;
                self.write("</sup>")?;
            }
            self.write("</a>")?;
        }
        Ok(())
    }

    fn footnotes(&mut self) -> Result<(), StreamError> {
        if self.calls.is_empty() {
            return Ok(());
        }
        self.terminate()?;
        self.write("<section data-footnotes=\"\" class=\"footnotes\"><")?;
        let tag = self
            .options
            .footnote_label_tag_name
            .as_deref()
            .unwrap_or("h2");
        self.escaped(tag, true)?;
        self.write(" id=\"footnote-label\" ")?;
        self.write(
            self.options
                .footnote_label_attributes
                .as_deref()
                .unwrap_or("class=\"sr-only\""),
        )?;
        self.write(">")?;
        self.escaped(
            self.options
                .footnote_label
                .as_deref()
                .unwrap_or("Footnotes"),
            true,
        )?;
        self.write("</")?;
        self.escaped(tag, true)?;
        self.write(">")?;
        self.newline()?;
        self.write("<ol>")?;
        // Index called labels, then match definitions in completion order. Unused labels need no index storage, and the first matching body wins.
        let mut order = Vec::new();
        crate::definitions::build_index(&mut order, &self.calls, |call| call.key.as_str());
        let mut remaining = self.calls.len();
        for (index, (key, _)) in self.notes.iter().enumerate() {
            if let Some(call) =
                crate::definitions::find_index(&order, &self.calls, key, |call| call.key.as_str())
            {
                if self.calls[call].definition.is_none() {
                    self.calls[call].definition = Some(index);
                    remaining -= 1;
                    if remaining == 0 {
                        break;
                    }
                }
            }
        }
        for index in 0..self.calls.len() {
            let call = &self.calls[index];
            let note = call.definition.ok_or(StreamError::Unbalanced)?;
            let count = call.count;
            let safe = core::mem::take(&mut self.calls[index].id);
            let value = core::mem::take(&mut self.notes[note].1);
            self.newline()?;
            self.write("<li id=\"")?;
            self.prefix()?;
            self.write("fn-")?;
            self.write(&safe)?;
            self.write("\">")?;
            self.newline()?;
            let trimmed = value.trim_end_matches(['\r', '\n']);
            if let Some(before) = trimmed.strip_suffix("</p>") {
                self.write(before)?;
                self.write(" ")?;
                self.backreferences(&safe, count)?;
                self.write(&value[before.len()..])?;
            } else {
                self.write(&value)?;
                self.terminate()?;
                self.backreferences(&safe, count)?;
            }
            self.terminate()?;
            self.write("</li>")?;
        }
        self.newline()?;
        self.write("</ol>")?;
        self.newline()?;
        self.write("</section>")?;
        self.newline()
    }
}

/// Render events with the shared HTML writer.
///
/// # Errors
/// Returns parser, malformed-stream, invalid-range, or output errors.
pub fn render_events<'a>(
    source: &str,
    events: impl IntoIterator<Item = Result<Event<'a>, StreamError>>,
    options: &CompileOptions,
    output: &mut impl fmt::Write,
) -> Result<(), StreamError> {
    EventHtmlRenderer::new(source, options, output).render(events)
}

#[cfg(test)]
mod tests {
    use super::EventHtmlRenderer;
    use crate::stream::{Event, EventKind, Tag, Text};
    use crate::CompileOptions;
    use alloc::string::String;
    use core::fmt::{self, Write};

    #[test]
    fn decimal_formats_zero_boundaries_and_full_width_values() {
        let mut buffer = [0; 20];
        for value in [0, 1, 9, 10, 99, 100, u64::from(u32::MAX), u64::MAX] {
            assert_eq!(
                super::decimal(value, &mut buffer),
                alloc::format!("{value}")
            );
        }
        for exponent in 1..=19 {
            let power = 10_u64.pow(exponent);
            for value in [power - 1, power, power + 1] {
                assert_eq!(
                    super::decimal(value, &mut buffer),
                    alloc::format!("{value}")
                );
            }
        }
    }

    struct Writer {
        remaining: usize,
        output: String,
    }
    impl Write for Writer {
        fn write_str(&mut self, value: &str) -> fmt::Result {
            if self.remaining == 0 {
                return Err(fmt::Error);
            }
            self.remaining -= 1;
            self.output.push_str(value);
            Ok(())
        }
    }

    #[test]
    fn filtered_html_preserves_written_prefix_when_a_writer_fails() {
        let options = CompileOptions {
            allow_dangerous_html: true,
            tagfilter: true,
            ..CompileOptions::default()
        };
        for (remaining, expected) in [
            "",
            "prefix",
            "prefix&lt;",
            "prefix&lt;script>",
            "prefix&lt;script>�",
        ]
        .iter()
        .copied()
        .enumerate()
        {
            let mut writer = Writer {
                remaining,
                output: String::new(),
            };
            let mut renderer = EventHtmlRenderer::new("", &options, &mut writer);
            renderer
                .event(Event {
                    kind: EventKind::Start(Tag::Document(None)),
                    span: 0..0,
                })
                .unwrap();
            renderer
                .event(Event {
                    kind: EventKind::Html {
                        value: Text::Literal("prefix<script>\0tail"),
                        block: false,
                    },
                    span: 0..0,
                })
                .unwrap_err();
            assert_eq!(renderer.last, expected.as_bytes().last().copied());
            assert_eq!(writer.output, expected);
        }
    }

    #[test]
    fn footnote_backreferences_escape_options_and_propagate_each_write_failure() {
        let options = CompileOptions {
            footnote_clobber_prefix: Some("<&é".into()),
            footnote_back_label: Some("back \"<&".into()),
            ..CompileOptions::default()
        };
        let expected = "<a href=\"#&lt;&amp;éfnref-note\" data-footnote-backref=\"\" aria-label=\"back &quot;&lt;&amp;\" class=\"data-footnote-backref\">↩</a> <a href=\"#&lt;&amp;éfnref-note-2\" data-footnote-backref=\"\" aria-label=\"back &quot;&lt;&amp;\" class=\"data-footnote-backref\">↩<sup>2</sup></a>";
        let mut complete = false;
        for remaining in 0..64 {
            let mut writer = Writer {
                remaining,
                output: String::new(),
            };
            let mut renderer = EventHtmlRenderer::new("", &options, &mut writer);
            let result = renderer.backreferences("note", 2);
            let last = renderer.last;
            assert_eq!(last, writer.output.as_bytes().last().copied());
            assert!(expected.starts_with(&writer.output));
            if result.is_ok() {
                assert_eq!(writer.output, expected);
                complete = true;
            }
        }
        assert!(
            complete,
            "backreferences should finish within the write budget"
        );
    }
}
