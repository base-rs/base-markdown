// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! HTML rendering for mdast trees.
use super::Renderer;
use crate::{
    mdast::{AlignKind, AttributeContent, AttributeValue, Heading, Node},
    util::{encode::encode, sanitize_uri::sanitize_with_protocols},
    CompileOptions,
};
use alloc::{
    collections::BTreeMap,
    fmt::Write as _,
    format,
    string::{String, ToString},
    vec::Vec,
};

/// Render an mdast tree or fragment as HTML.
///
/// This renders AST nodes directly. It is separate from [`crate::to_html`],
/// which renders Markdown source through the parser's event compiler. Raw HTML
/// is escaped and JSX is omitted by default; URLs use the same safe protocol
/// allowlists as the built-in HTML compiler. Pass `allow_dangerous_html` to
/// render raw HTML and static JSX. MDX expressions and ESM are omitted because
/// evaluating JavaScript is outside the renderer's scope.
///
/// # Example
///
/// ```
/// use core_markdown::{mdast::Node, to_mdast, HtmlRenderer, ParseOptions};
///
/// let mut tree = to_mdast("# Hello *world*!", &ParseOptions::default()).unwrap();
/// if let Node::Heading(heading) = &mut tree.children_mut().unwrap()[0] {
///     heading.depth = 2;
/// }
///
/// let mut renderer = HtmlRenderer::new();
/// let html = tree.render_with(&mut renderer);
/// assert_eq!(html, "<h2>Hello <em>world</em>!</h2>");
/// ```
#[derive(Clone, Debug, Default)]
pub struct HtmlRenderer {
    options: CompileOptions,
    heading_anchors: Vec<String>,
    heading_index: usize,
    references: BTreeMap<String, (String, Option<String>)>,
    footnote_definitions: BTreeMap<String, Vec<Node>>,
    footnote_numbers: BTreeMap<String, usize>,
    footnote_calls: BTreeMap<String, usize>,
    footnote_order: Vec<String>,
    tight_list: bool,
}

impl HtmlRenderer {
    /// Create a safe HTML renderer with default options.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create an HTML renderer with compilation options.
    #[must_use]
    pub fn with_options(options: CompileOptions) -> Self {
        Self {
            options,
            ..Self::default()
        }
    }

    /// Return the options used by this renderer.
    #[must_use]
    pub fn options(&self) -> &CompileOptions {
        &self.options
    }

    fn prepare(&mut self, root: &Node) {
        self.heading_anchors.clear();
        self.heading_index = 0;
        self.references.clear();
        self.footnote_definitions.clear();
        self.footnote_numbers.clear();
        self.footnote_calls.clear();
        self.footnote_order.clear();
        self.tight_list = false;

        if self.options.heading_ids {
            self.heading_anchors = crate::mdast::Headings::extract(root)
                .iter()
                .map(|heading| heading.anchor.clone())
                .collect();
        }

        let mut references = BTreeMap::new();
        let mut footnote_definitions = BTreeMap::new();
        root.walk(|node| match node {
            Node::Definition(definition) => {
                references.insert(
                    definition.identifier.clone(),
                    (definition.url.clone(), definition.title.clone()),
                );
            }
            Node::FootnoteDefinition(definition) => {
                footnote_definitions
                    .insert(definition.identifier.clone(), definition.children.clone());
            }
            _ => {}
        });
        self.references = references;
        self.footnote_definitions = footnote_definitions;
    }

    fn render_node_inner(&mut self, node: &Node) -> String {
        match node {
            Node::Root(root) => self.render_root(&root.children),
            Node::Fragment(fragment) => self.render_root(&fragment.children),
            Node::Paragraph(paragraph) => {
                let content = self.render_inline_children(&paragraph.children);
                if content.is_empty() {
                    String::new()
                } else if self.tight_list {
                    content
                } else {
                    format!("<p>{content}</p>")
                }
            }
            Node::Heading(heading) => self.render_heading(heading),
            Node::Blockquote(quote) => {
                let children = self.render_flow_children(&quote.children);
                let line_ending = self.line_ending();
                format!(
                    "<blockquote>{}</blockquote>",
                    with_surrounding_line_endings(&children, &line_ending)
                )
            }
            Node::List(list) => self.render_list(list),
            Node::ListItem(item) => self.render_list_item(item),
            Node::Code(code) => {
                let class = code
                    .lang
                    .as_ref()
                    .map(|lang| format!(" class=\"language-{}\"", encode(lang, true)))
                    .unwrap_or_default();
                format!(
                    "<pre><code{}>{}</code></pre>",
                    class,
                    self.render_code_value(&code.value)
                )
            }
            Node::Math(math) => format!(
                "<pre><code class=\"language-math math-display\">{}</code></pre>",
                self.render_code_value(&math.value)
            ),
            Node::ThematicBreak(_) => "<hr />".into(),
            Node::Html(html) => self.render_html(&html.value),
            Node::Break(_) => format!("<br />{}", self.line_ending()),
            Node::InlineCode(code) => {
                let class = code
                    .lang
                    .as_ref()
                    .map(|lang| format!(" class=\"language-{}\"", encode(lang, true)))
                    .unwrap_or_default();
                format!("<code{}>{}</code>", class, encode(&code.value, true))
            }
            Node::InlineMath(math) => format!(
                "<code class=\"language-math math-inline\">{}</code>",
                encode(&math.value, true)
            ),
            Node::Emphasis(emphasis) => format!(
                "<em>{}</em>",
                self.render_inline_children(&emphasis.children)
            ),
            Node::Strong(strong) => format!(
                "<strong>{}</strong>",
                self.render_inline_children(&strong.children)
            ),
            Node::Delete(delete) => format!(
                "<del>{}</del>",
                self.render_inline_children(&delete.children)
            ),
            Node::Text(text) => encode(&text.value, true),
            Node::Link(link) => {
                let href = self.link_url(&link.url);
                let title = render_title(link.title.as_deref());
                format!(
                    "<a href=\"{}\"{}>{}</a>",
                    href,
                    title,
                    self.render_inline_children(&link.children)
                )
            }
            Node::Image(image) => {
                let src = self.image_url(&image.url);
                let title = render_title(image.title.as_deref());
                format!(
                    "<img src=\"{}\" alt=\"{}\"{} />",
                    src,
                    encode(&image.alt, true),
                    title
                )
            }
            Node::LinkReference(reference) => self.render_link_reference(reference),
            Node::ImageReference(reference) => self.render_image_reference(reference),
            Node::Table(table) => self.render_table(table),
            Node::TableRow(row) => self.render_table_row(&row.children, false, &[]),
            Node::TableCell(cell) => {
                format!("<td>{}</td>", self.render_inline_children(&cell.children))
            }
            Node::FootnoteReference(reference) => self.render_footnote_reference(reference),
            Node::FootnoteDefinition(_)
            | Node::Definition(_)
            | Node::MdxTextExpression(_)
            | Node::MdxFlowExpression(_)
            | Node::MdxjsEsm(_)
            | Node::Frontmatter(_) => String::new(),
            Node::MdxJsxFlowElement(element) => self.render_jsx(
                element.name.as_deref(),
                &element.attributes,
                &element.children,
                true,
            ),
            Node::MdxJsxTextElement(element) => self.render_jsx(
                element.name.as_deref(),
                &element.attributes,
                &element.children,
                false,
            ),
        }
    }

    fn render_root(&mut self, children: &[Node]) -> String {
        let mut output = self.render_flow_children(children);
        let footnotes = self.render_footnote_section();
        let line_ending = self.line_ending();

        if !footnotes.is_empty() {
            if !output.is_empty() {
                output.push_str(&line_ending);
            }
            output.push_str(&footnotes);
        }

        output
    }

    fn render_heading(&mut self, heading: &Heading) -> String {
        let anchor = self.heading_anchors.get(self.heading_index).cloned();
        self.heading_index += 1;
        let id = anchor
            .map(|value| format!(" id=\"{}\"", encode(&value, true)))
            .unwrap_or_default();
        let content = self.render_inline_children(&heading.children);
        format!("<h{0}{1}>{2}</h{0}>", heading.depth, id, content)
    }

    fn render_list(&mut self, list: &crate::mdast::List) -> String {
        let previous_tight = self.tight_list;
        self.tight_list = !list.spread;

        let mut items = Vec::new();
        for child in &list.children {
            let rendered = self.render_node_inner(child);
            if !rendered.is_empty() {
                items.push(rendered);
            }
        }

        self.tight_list = previous_tight;
        let line_ending = self.line_ending();
        let content = items.join(&line_ending);

        if list.ordered {
            let start = list.start.unwrap_or(1);
            let start_attribute = if start == 1 {
                String::new()
            } else {
                format!(" start=\"{start}\"")
            };
            format!("<ol{start_attribute}>{line_ending}{content}{line_ending}</ol>")
        } else {
            format!("<ul>{line_ending}{content}{line_ending}</ul>")
        }
    }

    fn render_list_item(&mut self, item: &crate::mdast::ListItem) -> String {
        let mut checkbox = String::new();
        if let Some(checked) = item.checked {
            checkbox.push_str("<input type=\"checkbox\" ");
            if checked {
                checkbox.push_str("checked=\"\" ");
            }
            if !self.options.gfm_task_list_item_checkable {
                checkbox.push_str("disabled=\"\" ");
            }
            checkbox.push_str("/> ");
        }

        let content = self.render_flow_children(&item.children);
        format!("<li>{checkbox}{content}</li>")
    }

    fn render_table(&mut self, table: &crate::mdast::Table) -> String {
        let mut rows = table.children.iter();
        let mut output = String::from("<table>");
        let line_ending = self.line_ending();

        if let Some(Node::TableRow(row)) = rows.next() {
            output.push_str(&line_ending);
            output.push_str("<thead>");
            output.push_str(&line_ending);
            output.push_str(&self.render_table_row(&row.children, true, &table.align));
            output.push_str(&line_ending);
            output.push_str("</thead>");
        }

        let body = rows
            .filter_map(|node| match node {
                Node::TableRow(row) => Some(row),
                _ => None,
            })
            .collect::<Vec<_>>();

        if !body.is_empty() {
            output.push_str(&line_ending);
            output.push_str("<tbody>");
            output.push_str(&line_ending);
            for (index, row) in body.iter().enumerate() {
                if index > 0 {
                    output.push_str(&line_ending);
                }
                output.push_str(&self.render_table_row(&row.children, false, &table.align));
            }
            output.push_str(&line_ending);
            output.push_str("</tbody>");
        }

        output.push_str(&line_ending);
        output.push_str("</table>");
        output
    }

    fn render_table_row(
        &mut self,
        cells: &[Node],
        header: bool,
        alignments: &[AlignKind],
    ) -> String {
        let mut output = String::from("<tr>");
        for (index, cell) in cells.iter().enumerate() {
            let tag = if header { "th" } else { "td" };
            let alignment = match alignments.get(index) {
                Some(AlignKind::Left) => " align=\"left\"",
                Some(AlignKind::Right) => " align=\"right\"",
                Some(AlignKind::Center) => " align=\"center\"",
                Some(AlignKind::None) | None => "",
            };
            output.push('<');
            output.push_str(tag);
            output.push_str(alignment);
            output.push('>');
            match cell {
                Node::TableCell(cell) => {
                    output.push_str(&self.render_inline_children(&cell.children));
                }
                _ => output.push_str(&self.render_node_inner(cell)),
            }
            output.push_str("</");
            output.push_str(tag);
            output.push('>');
        }
        output.push_str("</tr>");
        output
    }

    fn render_link_reference(&mut self, reference: &crate::mdast::LinkReference) -> String {
        if let Some((url, title)) = self.references.get(&reference.identifier).cloned() {
            let href = self.link_url(&url);
            format!(
                "<a href=\"{}\"{}>{}</a>",
                href,
                render_title(title.as_deref()),
                self.render_inline_children(&reference.children)
            )
        } else {
            self.render_inline_children(&reference.children)
        }
    }

    fn render_image_reference(&mut self, reference: &crate::mdast::ImageReference) -> String {
        if let Some((url, title)) = self.references.get(&reference.identifier).cloned() {
            format!(
                "<img src=\"{}\" alt=\"{}\"{} />",
                self.image_url(&url),
                encode(&reference.alt, true),
                render_title(title.as_deref())
            )
        } else {
            encode(&reference.alt, true)
        }
    }

    fn render_footnote_reference(&mut self, reference: &crate::mdast::FootnoteReference) -> String {
        if !self
            .footnote_definitions
            .contains_key(&reference.identifier)
        {
            return String::new();
        }

        let number = if let Some(number) = self.footnote_numbers.get(&reference.identifier) {
            *number
        } else {
            let number = self.footnote_order.len() + 1;
            self.footnote_order.push(reference.identifier.clone());
            self.footnote_numbers
                .insert(reference.identifier.clone(), number);
            number
        };

        let call = self
            .footnote_calls
            .entry(reference.identifier.clone())
            .or_insert(0);
        *call += 1;

        let prefix = self
            .options
            .gfm_footnote_clobber_prefix
            .as_deref()
            .unwrap_or("user-content-");
        let id = crate::util::sanitize_uri::sanitize(&reference.identifier.to_lowercase());
        let suffix = if *call == 1 {
            String::new()
        } else {
            format!("-{call}")
        };

        format!(
            "<sup><a href=\"#{}fn-{}\" id=\"{}fnref-{}{}\" data-footnote-ref=\"\" aria-describedby=\"footnote-label\">{}</a></sup>",
            encode(prefix, true),
            id,
            encode(prefix, true),
            id,
            suffix,
            number
        )
    }

    fn render_footnote_section(&mut self) -> String {
        if self.footnote_order.is_empty() {
            return String::new();
        }

        let label_tag = self
            .options
            .gfm_footnote_label_tag_name
            .as_deref()
            .unwrap_or("h2");
        let label_attributes = self
            .options
            .gfm_footnote_label_attributes
            .as_deref()
            .unwrap_or("class=\"sr-only\"");
        let label = self
            .options
            .gfm_footnote_label
            .as_deref()
            .unwrap_or("Footnotes");
        let line_ending = self.line_ending();
        let mut output = format!(
            "<section data-footnotes=\"\" class=\"footnotes\"><{0} id=\"footnote-label\" {1}>{2}</{0}>",
            encode(label_tag, true),
            label_attributes,
            encode(label, true),
        );
        output.push_str(&line_ending);
        output.push_str("<ol>");
        output.push_str(&line_ending);
        let mut has_item = false;

        let mut index = 0;
        while index < self.footnote_order.len() {
            let identifier = self.footnote_order[index].clone();
            index += 1;
            let Some(children) = self.footnote_definitions.get(&identifier).cloned() else {
                continue;
            };

            let prefix = self
                .options
                .gfm_footnote_clobber_prefix
                .clone()
                .unwrap_or_else(|| "user-content-".into());
            let id = crate::util::sanitize_uri::sanitize(&identifier.to_lowercase());
            let mut content = self.render_flow_children(&children);
            let count = self.footnote_calls.get(&identifier).copied().unwrap_or(1);
            let back_label = self
                .options
                .gfm_footnote_back_label
                .as_deref()
                .unwrap_or("Back to content");
            let mut backrefs = String::new();

            for call in 1..=count {
                if call > 1 {
                    backrefs.push(' ');
                }
                let suffix = if call == 1 {
                    String::new()
                } else {
                    format!("-{call}")
                };
                let _ = write!(
                    backrefs,
                    "<a href=\"#{}fnref-{}{}\" data-footnote-backref=\"\" aria-label=\"{}\" class=\"data-footnote-backref\">↩",
                    encode(&prefix, true),
                    id,
                    suffix,
                    encode(back_label, true)
                );
                if call > 1 {
                    let _ = write!(backrefs, "<sup>{call}</sup>");
                }
                backrefs.push_str("</a>");
            }

            if let Some(index) = content.rfind("</p>") {
                content.insert_str(index, &format!(" {backrefs}"));
            } else {
                content.push(' ');
                content.push_str(&backrefs);
            }

            if has_item {
                output.push_str(&line_ending);
            }
            let _ = write!(output, "<li id=\"{}fn-{}\">", encode(&prefix, true), id);
            output.push_str(&line_ending);
            output.push_str(&content);
            output.push_str("</li>");
            has_item = true;
        }

        output.push_str(&line_ending);
        output.push_str("</ol>");
        output.push_str(&line_ending);
        output.push_str("</section>");
        output
    }

    fn render_jsx(
        &mut self,
        name: Option<&str>,
        attributes: &[AttributeContent],
        children: &[Node],
        flow: bool,
    ) -> String {
        // JSX is an HTML-like construct. Keep it disabled under the same
        // trust option as raw HTML.
        if !self.options.allow_dangerous_html {
            return String::new();
        }

        let content = if flow {
            self.render_flow_children(children)
        } else {
            self.render_inline_children(children)
        };

        let Some(name) = name else {
            return content;
        };

        let mut opening = format!("<{}", encode(name, true));
        for attribute in attributes {
            let attribute = match attribute {
                AttributeContent::Property(attribute) => attribute,
                AttributeContent::Expression(_) => continue,
            };
            match &attribute.value {
                Some(AttributeValue::Literal(value)) => {
                    opening.push(' ');
                    opening.push_str(&encode(&attribute.name, true));
                    opening.push_str("=\"");
                    let value = if attribute.name.eq_ignore_ascii_case("href") {
                        self.link_url(value)
                    } else if attribute.name.eq_ignore_ascii_case("src") {
                        self.image_url(value)
                    } else {
                        encode(value, true)
                    };
                    opening.push_str(&value);
                    opening.push('"');
                }
                Some(AttributeValue::Expression(_)) => {}
                None => {
                    opening.push(' ');
                    opening.push_str(&encode(&attribute.name, true));
                }
            }
        }

        let output = if children.is_empty() {
            format!("{opening}/>")
        } else {
            format!("{}>{}</{}>", opening, content, encode(name, true))
        };

        self.render_html(&output)
    }

    fn render_html(&self, value: &str) -> String {
        if !self.options.allow_dangerous_html {
            return encode(value, true);
        }
        if self.options.gfm_tagfilter {
            crate::util::gfm_tagfilter::gfm_tagfilter(value)
        } else {
            value.to_string()
        }
    }

    fn render_code_value(&self, value: &str) -> String {
        let mut output = encode(value, true);
        if !value.is_empty() && !value.ends_with('\n') && !value.ends_with('\r') {
            output.push_str(&self.line_ending());
        }
        output
    }

    fn line_ending(&self) -> String {
        self.options.default_line_ending.as_str().to_string()
    }

    fn link_url(&self, value: &str) -> String {
        if self.options.allow_dangerous_protocol {
            crate::util::sanitize_uri::sanitize(value)
        } else {
            sanitize_with_protocols(value, &["http", "https", "irc", "ircs", "mailto", "xmpp"])
        }
    }

    fn image_url(&self, value: &str) -> String {
        if self.options.allow_any_img_src || self.options.allow_dangerous_protocol {
            crate::util::sanitize_uri::sanitize(value)
        } else {
            sanitize_with_protocols(value, &["http", "https"])
        }
    }

    fn render_inline_children(&mut self, children: &[Node]) -> String {
        let mut output = String::new();
        for child in children {
            output.push_str(&self.render_node_inner(child));
        }
        output
    }

    fn render_flow_children(&mut self, children: &[Node]) -> String {
        let mut output = String::new();
        let mut count = 0;
        let line_ending = self.line_ending();
        for child in children {
            let rendered = self.render_node_inner(child);
            if rendered.is_empty() {
                continue;
            }
            if count > 0 {
                output.push_str(&line_ending);
            }
            output.push_str(&rendered);
            count += 1;
        }
        output
    }
}

impl Renderer for HtmlRenderer {
    fn render(&mut self, node: &Node) -> String {
        self.prepare(node);
        self.render_node_inner(node)
    }

    fn render_node(&mut self, node: &Node) -> String {
        self.render_node_inner(node)
    }

    fn render_children(&mut self, node: &Node) -> String {
        match node {
            Node::Root(_)
            | Node::Fragment(_)
            | Node::Blockquote(_)
            | Node::List(_)
            | Node::ListItem(_)
            | Node::Table(_)
            | Node::FootnoteDefinition(_)
            | Node::MdxJsxFlowElement(_) => node
                .children()
                .map(|children| self.render_flow_children(children))
                .unwrap_or_default(),
            _ => node
                .children()
                .map(|children| self.render_inline_children(children))
                .unwrap_or_default(),
        }
    }
}

fn render_title(title: Option<&str>) -> String {
    match title {
        Some(title) => format!(" title=\"{}\"", encode(title, true)),
        None => String::new(),
    }
}

fn with_surrounding_line_endings(value: &str, line_ending: &str) -> String {
    if value.is_empty() {
        String::new()
    } else {
        format!("{line_ending}{value}{line_ending}")
    }
}

#[cfg(test)]
mod tests {
    use super::HtmlRenderer;
    use crate::mdast::{Node, Paragraph, Text};
    use alloc::vec;

    #[test]
    fn skips_footnote_entries_without_definitions() {
        let mut renderer = HtmlRenderer::new();
        renderer.footnote_order.push("missing".into());

        let output = renderer.render_footnote_section();

        assert!(output.contains("<ol>"));
        assert!(!output.contains("<li>"));
    }

    #[test]
    fn renders_footnotes_without_preceding_root_content() {
        let mut renderer = HtmlRenderer::new();
        renderer.footnote_order.push("note".into());
        renderer.footnote_definitions.insert(
            "note".into(),
            vec![Node::Paragraph(Paragraph {
                children: vec![Node::Text(Text {
                    value: "footnote".into(),
                    position: None,
                })],
                position: None,
            })],
        );

        let output = renderer.render_root(&[]);

        assert!(output.starts_with("<section data-footnotes"));
        assert!(!output.starts_with('\n'));
    }

    #[test]
    fn omits_empty_nodes_from_lists_and_only_terminates_code_when_needed() {
        let mut renderer = HtmlRenderer::new();
        let list = crate::mdast::List {
            children: vec![Node::MdxFlowExpression(crate::mdast::MdxFlowExpression {
                value: "value".into(),
                position: None,
                stops: vec![],
            })],
            position: None,
            ordered: false,
            start: None,
            spread: false,
        };

        assert_eq!(renderer.render_list(&list), "<ul>\n\n</ul>");
        assert_eq!(renderer.render_code_value(""), "");
        assert_eq!(renderer.render_code_value("code\n"), "code\n");
    }
}
