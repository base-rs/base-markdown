// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! One HTML writer for parser events and document traversal.
use crate::stream::{Event, EventKind, StreamError, Tag, TagKind};
use crate::util::{
    constant::{SAFE_PROTOCOL_HREF, SAFE_PROTOCOL_SRC},
    sanitize_uri::{sanitize, write_sanitized},
};
use crate::{CompileOptions, LineEnding};
use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};
use core::fmt::{self, Write};

struct Frame {
    kind: TagKind,
    flag: bool,
    level: u8,
}
struct HeadingCapture {
    output: String,
    text: String,
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
    calls: Vec<(String, usize)>,
    started: bool,
    ended: bool,
    heading: Option<HeadingCapture>,
    anchors: crate::mdast::AnchorGenerator,
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
            anchors: crate::mdast::AnchorGenerator::default(),
        }
    }

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
        let mut start = 0;
        for (index, byte) in value.bytes().enumerate() {
            let replacement = match byte {
                0 => Some("�"),
                b'&' if encode => Some("&amp;"),
                b'"' if encode => Some("&quot;"),
                b'<' if encode => Some("&lt;"),
                b'>' if encode => Some("&gt;"),
                _ => None,
            };
            if let Some(replacement) = replacement {
                self.write(&value[start..index])?;
                self.write(replacement)?;
                start = index + 1;
            }
        }
        self.write(&value[start..])
    }

    fn newline(&mut self) -> Result<(), StreamError> {
        let value = self.line_ending.clone();
        self.write(value.as_str())
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
        write_sanitized(value, protocols, true, |value| self.write(value))
    }

    fn start(&mut self, tag: Tag<'_>) -> Result<(), StreamError> {
        let kind = tag.kind();
        if kind == TagKind::Document {
            if self.started || !self.stack.is_empty() {
                return Err(StreamError::Unbalanced);
            }
            self.started = true;
        } else if self.stack.is_empty() {
            return Err(StreamError::Unbalanced);
        }
        let mut frame = Frame {
            kind,
            flag: false,
            level: 0,
        };
        // Tags inside image alternative text contribute only their text content.
        let tags = self.images == 0;
        match tag {
            Tag::Document(line_ending) => {
                if let Some(value) = line_ending {
                    self.line_ending = value;
                }
            }
            Tag::Paragraph { tight } => {
                frame.flag = tight;
                if tags && !tight {
                    self.terminate()?;
                    self.write("<p>")?;
                }
            }
            Tag::Heading(level) => {
                if !(1..=6).contains(&level) {
                    return Err(StreamError::Unbalanced);
                }
                frame.level = level;
                if tags {
                    self.terminate()?;
                    if self.options.heading_ids {
                        self.heading = Some(HeadingCapture {
                            output: String::new(),
                            text: String::new(),
                        });
                    } else {
                        self.write("<h")?;
                        self.write(&level.to_string())?;
                        self.write(">")?;
                    }
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
                    self.write(if start.is_some() { "<ol" } else { "<ul" })?;
                    if let Some(start) = start {
                        if start != 1 {
                            self.write(" start=\"")?;
                            self.write(&start.to_string())?;
                            self.write("\"")?;
                        }
                    }
                    self.write(">")?;
                }
            }
            Tag::Item { block_end } => {
                frame.flag = block_end;
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
                    self.destination(destination.resolve(self.source)?, false)?;
                    self.write("\"")?;
                    if let Some(title) = title {
                        self.write(" title=\"")?;
                        self.escaped(title.resolve(self.source)?, true)?;
                        self.write("\"")?;
                    }
                    self.write(">")?;
                }
            }
            Tag::Image { destination, title } => {
                frame.flag = tags;
                if tags {
                    self.write("<img src=\"")?;
                    self.destination(destination.resolve(self.source)?, true)?;
                    self.write("\" alt=\"")?;
                }
                // The title belongs after alt. Keep it in the frame's separate capture only when present.
                if tags {
                    if let Some(title) = title {
                        frame.level = 1;
                        let mut output = String::new();
                        crate::util::encode::encode_into(
                            title.resolve(self.source)?,
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
                frame.flag = header;
                self.terminate()?;
                self.write(if header { "<th" } else { "<td" })?;
                match alignment {
                    crate::mdast::AlignKind::Left => self.write(" align=\"left\"")?,
                    crate::mdast::AlignKind::Right => self.write(" align=\"right\"")?,
                    crate::mdast::AlignKind::Center => self.write(" align=\"center\"")?,
                    crate::mdast::AlignKind::None => {}
                }
                self.write(">")?;
            }
            Tag::FootnoteDefinition(label) => {
                let key = label.resolve(self.source)?.to_string();
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

    fn end(&mut self, kind: TagKind) -> Result<(), StreamError> {
        let frame = self.stack.pop().ok_or(StreamError::Unbalanced)?;
        if frame.kind != kind {
            return Err(StreamError::Unbalanced);
        }
        if kind == TagKind::Image {
            self.images -= 1;
            if frame.flag {
                self.write("\"")?;
                if frame.level != 0 {
                    let title = self.image_titles.pop().unwrap();
                    self.write(" title=\"")?;
                    self.write(&title)?;
                    self.write("\"")?;
                }
                self.write(" />")?;
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
                    let text = heading
                        .text
                        .strip_suffix("\r\n")
                        .or_else(|| heading.text.strip_suffix('\n'))
                        .or_else(|| heading.text.strip_suffix('\r'))
                        .unwrap_or(&heading.text);
                    let anchor = self.anchors.anchor_for(text);
                    self.write("<h")?;
                    self.write(&frame.level.to_string())?;
                    self.write(" id=\"")?;
                    self.escaped(&anchor, true)?;
                    self.write("\">")?;
                    self.write(&heading.output)?;
                }
                self.write("</h")?;
                self.write(&frame.level.to_string())?;
                self.write(">")?;
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
        for event in events {
            self.event(event?)?;
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
    pub fn event(&mut self, event: Event<'_>) -> Result<(), StreamError> {
        if self.ended {
            return Err(StreamError::Unbalanced);
        }
        event.validate(Some(self.source))?;
        if let Some(heading) = &mut self.heading {
            match &event.kind {
                EventKind::Text(value)
                | EventKind::SoftBreak(value)
                | EventKind::Math(value)
                | EventKind::Code { value, .. }
                | EventKind::Html { value, .. } => {
                    heading.text.push_str(value.resolve(self.source)?);
                }
                EventKind::HardBreak => heading.text.push('\n'),
                _ => {}
            }
        }
        match event.kind {
            EventKind::Start(tag) => self.start(tag),
            EventKind::End(kind) => self.end(kind),
            kind => {
                if self.stack.is_empty() {
                    return Err(StreamError::Unbalanced);
                }
                match kind {
                    EventKind::Text(value) | EventKind::SoftBreak(value) => {
                        self.escaped(value.resolve(self.source)?, true)
                    }
                    EventKind::Separator { value, conditional } => {
                        if conditional {
                            self.terminate()
                        } else {
                            self.escaped(value.resolve(self.source)?, true)
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
                        self.terminate()?;
                        self.write("<hr />")
                    }
                    EventKind::Code { value, language } => {
                        if self.images == 0 {
                            self.write("<code")?;
                            if let Some(language) = language {
                                self.write(" class=\"language-")?;
                                self.escaped(language.resolve(self.source)?, true)?;
                                self.write("\"")?;
                            }
                            self.write(">")?;
                        }
                        self.escaped(value.resolve(self.source)?, true)?;
                        if self.images == 0 {
                            self.write("</code>")?;
                        }
                        Ok(())
                    }
                    EventKind::Math(value) => {
                        if self.images == 0 {
                            self.write("<code class=\"language-math math-inline\">")?;
                        }
                        self.escaped(value.resolve(self.source)?, true)?;
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
                        self.terminate()?;
                        self.write("<pre><code")?;
                        if math {
                            self.write(" class=\"language-math math-display\"")?;
                        } else if let Some(language) = language {
                            self.write(" class=\"language-")?;
                            self.escaped(language.resolve(self.source)?, true)?;
                            self.write("\"")?;
                        }
                        self.write(">")?;
                        let value = value.resolve(self.source)?;
                        self.escaped(value, true)?;
                        if !value.is_empty() {
                            self.terminate()?;
                        }
                        self.write("</code></pre>")?;
                        if unclosed {
                            self.terminate()?;
                        }
                        Ok(())
                    }
                    EventKind::Html { value, block } => {
                        if block {
                            self.terminate()?;
                        }
                        let value = value.resolve(self.source)?;
                        if self.options.allow_dangerous_html && self.options.gfm_tagfilter {
                            self.escaped(&crate::util::gfm_tagfilter::gfm_tagfilter(value), false)
                        } else {
                            self.escaped(value, !self.options.allow_dangerous_html)
                        }
                    }
                    EventKind::TaskMarker(checked) => {
                        if self.images == 0 {
                            self.write("<input type=\"checkbox\" ")?;
                            if !self.options.gfm_task_list_item_checkable {
                                self.write("disabled=\"\" ")?;
                            }
                            if checked {
                                self.write("checked=\"\" ")?;
                            }
                            self.write("/>")?;
                        }
                        Ok(())
                    }
                    EventKind::Definition { .. } | EventKind::Opaque { .. } => Ok(()),
                    EventKind::FootnoteReference(value) => {
                        self.footnote_call(value.resolve(self.source)?)
                    }
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
                .gfm_footnote_clobber_prefix
                .as_deref()
                .unwrap_or("user-content-"),
            true,
        )
    }

    fn footnote_call(&mut self, key: &str) -> Result<(), StreamError> {
        let index = self
            .calls
            .iter()
            .position(|(id, _)| id == key)
            .unwrap_or_else(|| {
                self.calls.push((key.into(), 0));
                self.calls.len() - 1
            });
        self.calls[index].1 += 1;
        let count = self.calls[index].1;
        if self.images != 0 {
            return Ok(());
        }
        let safe = sanitize(&key.to_lowercase());
        self.write("<sup><a href=\"#")?;
        self.prefix()?;
        self.write("fn-")?;
        self.write(&safe)?;
        self.write("\" id=\"")?;
        self.prefix()?;
        self.write("fnref-")?;
        self.write(&safe)?;
        if count > 1 {
            self.write("-")?;
            self.write(&count.to_string())?;
        }
        self.write("\" data-footnote-ref=\"\" aria-describedby=\"footnote-label\">")?;
        self.write(&(index + 1).to_string())?;
        self.write("</a></sup>")
    }

    fn footnotes(&mut self) -> Result<(), StreamError> {
        if self.calls.is_empty() {
            return Ok(());
        }
        self.terminate()?;
        self.write("<section data-footnotes=\"\" class=\"footnotes\"><")?;
        let tag = self
            .options
            .gfm_footnote_label_tag_name
            .as_deref()
            .unwrap_or("h2");
        self.escaped(tag, true)?;
        self.write(" id=\"footnote-label\" ")?;
        self.write(
            self.options
                .gfm_footnote_label_attributes
                .as_deref()
                .unwrap_or("class=\"sr-only\""),
        )?;
        self.write(">")?;
        self.escaped(
            self.options
                .gfm_footnote_label
                .as_deref()
                .unwrap_or("Footnotes"),
            true,
        )?;
        self.write("</")?;
        self.escaped(tag, true)?;
        self.write(">")?;
        self.newline()?;
        self.write("<ol>")?;
        for index in 0..self.calls.len() {
            let (key, count) = &self.calls[index];
            let safe = sanitize(&key.to_lowercase());
            let note = self
                .notes
                .iter()
                .position(|(id, _)| id == key)
                .ok_or(StreamError::Unbalanced)?;
            let count = *count;
            let value = core::mem::take(&mut self.notes[note].1);
            let mut references = String::new();
            for reference in 1..=count {
                if reference != 1 {
                    references.push(' ');
                }
                let prefix = crate::util::encode::encode(
                    self.options
                        .gfm_footnote_clobber_prefix
                        .as_deref()
                        .unwrap_or("user-content-"),
                    true,
                );
                let back = crate::util::encode::encode(
                    self.options
                        .gfm_footnote_back_label
                        .as_deref()
                        .unwrap_or("Back to content"),
                    true,
                );
                let suffix = if reference == 1 {
                    String::new()
                } else {
                    format!("-{reference}")
                };
                let number = if reference == 1 {
                    String::new()
                } else {
                    format!("<sup>{reference}</sup>")
                };
                let _ = write!(references, "<a href=\"#{prefix}fnref-{safe}{suffix}\" data-footnote-backref=\"\" aria-label=\"{back}\" class=\"data-footnote-backref\">↩{number}</a>");
            }
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
                self.write(&references)?;
                self.write(&value[before.len()..])?;
            } else {
                self.write(&value)?;
                self.terminate()?;
                self.write(&references)?;
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
