// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use core_markdown::{
    mdast::{
        AlignKind, Blockquote, Break, Code, Definition, Delete, Emphasis, FootnoteDefinition,
        FootnoteReference, Frontmatter, Heading, Html, Image, ImageReference, InlineCode,
        InlineMath, Link, LinkReference, List, ListItem, Math, Node, Paragraph, ReferenceKind,
        Root, Strong, Table, TableCell, TableRow, Text, ThematicBreak,
    },
    message,
    renderer::Renderer,
    to_html_with_options, to_mdast, CompileOptions, HtmlRenderer, Options, ParseOptions,
};
use pretty_assertions::assert_eq;

fn text(value: &str) -> Node {
    Node::Text(Text {
        value: value.into(),
        position: None,
    })
}

fn paragraph(children: Vec<Node>) -> Node {
    Node::Paragraph(Paragraph {
        children,
        position: None,
    })
}

fn render(
    source: &str,
    parse_options: &ParseOptions,
    compile_options: CompileOptions,
) -> Result<String, message::Message> {
    let tree = to_mdast(source, parse_options)?;
    let mut renderer = HtmlRenderer::with_options(compile_options);
    Ok(tree.render_with(&mut renderer))
}

#[test]
fn renders_ast_nodes_and_heading_ids() -> Result<(), message::Message> {
    assert_eq!(
        render(
            "# Intro\n\nHello, *world* & `code`.",
            &ParseOptions::default(),
            CompileOptions {
                heading_ids: true,
                ..CompileOptions::default()
            },
        )?,
        "<h1 id=\"intro\">Intro</h1>\n<p>Hello, <em>world</em> &amp; <code>code</code>.</p>"
    );

    assert_eq!(
        render(
            "```rust\nlet value = \"<tag>\";\n```",
            &ParseOptions::default(),
            CompileOptions::default(),
        )?,
        "<pre><code class=\"language-rust\">let value = &quot;&lt;tag&gt;&quot;;\n</code></pre>"
    );

    Ok(())
}

#[test]
fn applies_html_and_url_safety_options() -> Result<(), message::Message> {
    let source = "<b>safe text</b>\n\n[link](javascript:alert(1))";

    assert_eq!(
        render(source, &ParseOptions::default(), CompileOptions::default())?,
        "<p>&lt;b&gt;safe text&lt;/b&gt;</p>\n<p><a href=\"\">link</a></p>"
    );

    assert_eq!(
        render(
            source,
            &ParseOptions::default(),
            CompileOptions {
                allow_dangerous_html: true,
                allow_dangerous_protocol: true,
                ..CompileOptions::default()
            },
        )?,
        "<p><b>safe text</b></p>\n<p><a href=\"javascript:alert(1)\">link</a></p>"
    );

    Ok(())
}

#[test]
fn renders_references_and_rich_inline_code() -> Result<(), message::Message> {
    assert_eq!(
        render(
            "[hello][id]\n\n[id]: /guide \"Guide\"",
            &ParseOptions::default(),
            CompileOptions::default(),
        )?,
        "<p><a href=\"/guide\" title=\"Guide\">hello</a></p>"
    );

    assert_eq!(
        render(
            "Use ruby:`puts 1`.",
            &ParseOptions {
                inline_code_info: true,
                ..ParseOptions::default()
            },
            CompileOptions::default(),
        )?,
        "<p>Use <code class=\"language-ruby\">puts 1</code>.</p>"
    );

    Ok(())
}

#[test]
fn renders_gfm_tables_and_task_lists() -> Result<(), message::Message> {
    assert_eq!(
        render(
            "- [x] done\n\n| A | B |\n| :- | -: |\n| one | two |",
            &ParseOptions::gfm(),
            CompileOptions::default(),
        )?,
        "<ul>\n<li><input type=\"checkbox\" checked=\"\" disabled=\"\" /> done</li>\n</ul>\n<table>\n<thead>\n<tr><th align=\"left\">A</th><th align=\"right\">B</th></tr>\n</thead>\n<tbody>\n<tr><td align=\"left\">one</td><td align=\"right\">two</td></tr>\n</tbody>\n</table>"
    );

    Ok(())
}

#[test]
fn renders_gfm_footnotes() -> Result<(), message::Message> {
    assert_eq!(
        render(
            "Call.[^note]\n\n[^note]: details",
            &ParseOptions::gfm(),
            CompileOptions::default(),
        )?,
        "<p>Call.<sup><a href=\"#user-content-fn-note\" id=\"user-content-fnref-note\" data-footnote-ref=\"\" aria-describedby=\"footnote-label\">1</a></sup></p>\n<section data-footnotes=\"\" class=\"footnotes\"><h2 id=\"footnote-label\" class=\"sr-only\">Footnotes</h2>\n<ol>\n<li id=\"user-content-fn-note\">\n<p>details <a href=\"#user-content-fnref-note\" data-footnote-backref=\"\" aria-label=\"Back to content\" class=\"data-footnote-backref\">↩</a></p></li>\n</ol>\n</section>"
    );

    Ok(())
}

#[test]
fn renders_ast_node_variants_and_custom_renderer_methods() {
    let tree = Node::Root(Root {
        position: None,
        children: vec![
            Node::Fragment(core_markdown::mdast::Fragment {
                children: vec![paragraph(vec![text("fragment")])],
            }),
            Node::Heading(Heading {
                children: vec![text("A heading")],
                position: None,
                depth: 2,
            }),
            Node::Blockquote(Blockquote {
                children: vec![paragraph(vec![text("quoted")])],
                position: None,
            }),
            Node::Blockquote(Blockquote {
                children: vec![],
                position: None,
            }),
            Node::List(List {
                children: vec![
                    Node::ListItem(ListItem {
                        children: vec![paragraph(vec![text("checked")])],
                        position: None,
                        spread: false,
                        checked: Some(true),
                    }),
                    Node::ListItem(ListItem {
                        children: vec![paragraph(vec![text("unchecked")])],
                        position: None,
                        spread: false,
                        checked: Some(false),
                    }),
                ],
                position: None,
                ordered: true,
                start: Some(3),
                spread: true,
            }),
            Node::List(List {
                children: vec![],
                position: None,
                ordered: true,
                start: Some(1),
                spread: false,
            }),
            Node::Code(Code {
                fence: None,
                value: "code".into(),
                position: None,
                lang: Some("rust".into()),
                meta: None,
            }),
            Node::Html(Html {
                value: "<!-- raw html -->".into(),
                position: None,
            }),
            Node::Math(Math {
                value: "x < y".into(),
                position: None,
                meta: None,
            }),
            Node::ThematicBreak(ThematicBreak { position: None }),
            paragraph(vec![
                Node::Break(Break { position: None }),
                Node::InlineCode(InlineCode {
                    value: "inline <code>".into(),
                    position: None,
                    lang: Some("ruby".into()),
                }),
                Node::InlineMath(InlineMath {
                    value: "x < y".into(),
                    position: None,
                }),
                Node::Emphasis(Emphasis {
                    children: vec![text("emphasis")],
                    position: None,
                }),
                Node::Strong(Strong {
                    children: vec![text("strong")],
                    position: None,
                }),
                Node::Delete(Delete {
                    children: vec![text("deleted")],
                    position: None,
                }),
                Node::Link(Link {
                    children: vec![text("linked")],
                    position: None,
                    url: "javascript:alert(1)".into(),
                    title: Some("link title".into()),
                }),
                Node::Image(Image {
                    position: None,
                    alt: "image & alt".into(),
                    url: "javascript:alert(1)".into(),
                    title: Some("image title".into()),
                }),
            ]),
            Node::Definition(Definition {
                position: None,
                url: "/guide".into(),
                title: Some("Reference title".into()),
                identifier: "guide".into(),
                label: None,
            }),
            paragraph(vec![
                Node::LinkReference(LinkReference {
                    children: vec![text("reference link")],
                    position: None,
                    reference_kind: ReferenceKind::Full,
                    identifier: "guide".into(),
                    label: None,
                }),
                Node::LinkReference(LinkReference {
                    children: vec![text("unresolved link")],
                    position: None,
                    reference_kind: ReferenceKind::Shortcut,
                    identifier: "missing".into(),
                    label: None,
                }),
                Node::ImageReference(ImageReference {
                    position: None,
                    alt: "reference image".into(),
                    reference_kind: ReferenceKind::Full,
                    identifier: "guide".into(),
                    label: None,
                }),
                Node::ImageReference(ImageReference {
                    position: None,
                    alt: "unresolved image".into(),
                    reference_kind: ReferenceKind::Shortcut,
                    identifier: "missing".into(),
                    label: None,
                }),
                Node::FootnoteReference(FootnoteReference {
                    position: None,
                    identifier: "note".into(),
                    label: None,
                }),
                Node::FootnoteReference(FootnoteReference {
                    position: None,
                    identifier: "missing-note".into(),
                    label: None,
                }),
                Node::FootnoteReference(FootnoteReference {
                    position: None,
                    identifier: "other-note".into(),
                    label: None,
                }),
            ]),
            Node::Table(Table {
                children: vec![
                    Node::TableRow(TableRow {
                        children: vec![
                            Node::TableCell(TableCell {
                                children: vec![text("left")],
                                position: None,
                            }),
                            Node::TableCell(TableCell {
                                children: vec![text("center")],
                                position: None,
                            }),
                            Node::TableCell(TableCell {
                                children: vec![text("right")],
                                position: None,
                            }),
                            Node::TableCell(TableCell {
                                children: vec![text("none")],
                                position: None,
                            }),
                        ],
                        position: None,
                    }),
                    Node::TableRow(TableRow {
                        children: vec![
                            Node::TableCell(TableCell {
                                children: vec![text("body")],
                                position: None,
                            }),
                            text("raw cell node"),
                        ],
                        position: None,
                    }),
                    Node::TableRow(TableRow {
                        children: vec![Node::TableCell(TableCell {
                            children: vec![text("second body row")],
                            position: None,
                        })],
                        position: None,
                    }),
                    paragraph(vec![text("ignored table child")]),
                ],
                position: None,
                align: vec![
                    AlignKind::Left,
                    AlignKind::Center,
                    AlignKind::Right,
                    AlignKind::None,
                ],
            }),
            Node::TableRow(TableRow {
                children: vec![text("standalone row")],
                position: None,
            }),
            Node::TableCell(TableCell {
                children: vec![text("standalone cell")],
                position: None,
            }),
            Node::Table(Table {
                children: vec![
                    paragraph(vec![text("not a table row")]),
                    Node::TableRow(TableRow {
                        children: vec![text("body without header")],
                        position: None,
                    }),
                ],
                position: None,
                align: vec![],
            }),
            Node::Frontmatter(Frontmatter {
                info: String::new(),
                fence: '-',
                fence_length: 3,
                closing_fence_length: 3,
                value: "title: Example".into(),
                position: None,
            }),
            Node::Frontmatter(Frontmatter {
                info: String::new(),
                fence: '+',
                fence_length: 3,
                closing_fence_length: 3,
                value: "title = 'Example'".into(),
                position: None,
            }),
            Node::FootnoteReference(FootnoteReference {
                position: None,
                identifier: "note".into(),
                label: None,
            }),
            Node::FootnoteDefinition(FootnoteDefinition {
                children: vec![paragraph(vec![text("footnote text")])],
                position: None,
                identifier: "note".into(),
                label: None,
            }),
            Node::FootnoteDefinition(FootnoteDefinition {
                children: vec![Node::Html(Html {
                    value: "raw footnote".into(),
                    position: None,
                })],
                position: None,
                identifier: "other-note".into(),
                label: None,
            }),
        ],
    });

    let options = CompileOptions {
        allow_any_img_src: true,
        allow_dangerous_html: true,
        gfm_tagfilter: true,
        gfm_task_list_item_checkable: true,
        heading_ids: true,
        gfm_footnote_back_label: Some("Back & forth".into()),
        gfm_footnote_clobber_prefix: Some("custom-".into()),
        gfm_footnote_label: Some("Notes & details".into()),
        gfm_footnote_label_attributes: Some("class=\"notes\"".into()),
        gfm_footnote_label_tag_name: Some("h3".into()),
        ..CompileOptions::default()
    };
    let mut renderer = HtmlRenderer::with_options(options);
    assert!(renderer.options().heading_ids);

    let output = tree.render_with(&mut renderer);
    for expected in [
        "<h2 id=\"a-heading\">A heading</h2>",
        "<blockquote>\n<p>quoted</p>\n</blockquote>",
        "<ol start=\"3\">",
        "<input type=\"checkbox\" checked=\"\" />",
        "<img src=\"javascript:alert(1)\" alt=\"image &amp; alt\" title=\"image title\" />",
        "<img src=\"/guide\" alt=\"reference image\" title=\"Reference title\" />",
        "<th align=\"center\">center</th>",
        "<td align=\"center\">raw cell node</td>",
        "second body row",
        "<table>\n<tbody>",
        "<h3 id=\"footnote-label\" class=\"notes\">Notes &amp; details</h3>",
        "aria-label=\"Back &amp; forth\"",
    ] {
        assert!(
            output.contains(expected),
            "missing {:?} in {:?}",
            expected,
            output
        );
    }

    let mut renderer = HtmlRenderer::new();
    let paragraph = paragraph(vec![text("child")]);
    assert_eq!(renderer.render_node(&text("direct")), "direct");
    assert_eq!(renderer.render_children(&paragraph), "child");
    assert_eq!(
        renderer.render_children(&Node::Root(Root {
            position: None,
            children: vec![paragraph.clone()],
        })),
        "<p>child</p>"
    );
    assert_eq!(
        renderer.render_children(&Node::ThematicBreak(ThematicBreak { position: None })),
        ""
    );

    let image = Node::Image(Image {
        position: None,
        alt: "image".into(),
        url: "javascript:bad()".into(),
        title: None,
    });
    let mut safe_renderer = HtmlRenderer::default();
    assert_eq!(
        safe_renderer.render_node(&image),
        "<img src=\"\" alt=\"image\" />"
    );
}

#[test]
fn covers_parser_html_options_and_generated_heading_ids() -> Result<(), message::Message> {
    let mut parse_options = ParseOptions::gfm();
    parse_options.inline_code_info = true;
    parse_options.constructs.math_text = true;
    parse_options.constructs.math_flow = true;
    let options = Options {
        parse: parse_options,
        compile: CompileOptions {
            heading_ids: true,
            gfm_task_list_item_checkable: false,
            ..CompileOptions::default()
        },
    };
    let html = to_html_with_options(
        "# Same\n\n# Same\n\nUse ruby:`puts 1` and $x$.\n\n- [x] done\n\n$$\nx^2\n$$",
        &options,
    )?;

    assert!(html.contains("<h1 id=\"same\">Same</h1>"));
    assert!(html.contains("<h1 id=\"same-2\">Same</h1>"));
    assert!(html.contains("<code class=\"language-ruby\">puts 1</code>"));
    assert!(html.contains("<code class=\"language-math math-inline\">x</code>"));
    assert!(html.contains("disabled=\"\""), "{:?}", html);
    assert!(html.contains("<code class=\"language-math math-display\">x^2"));

    Ok(())
}

#[test]
fn renders_unchecked_items_and_header_only_tables() {
    let tree = Node::Root(Root {
        position: None,
        children: vec![
            Node::ListItem(ListItem {
                children: vec![paragraph(vec![text("ordinary item")])],
                position: None,
                spread: false,
                checked: None,
            }),
            Node::Table(Table {
                children: vec![Node::TableRow(TableRow {
                    children: vec![Node::TableCell(TableCell {
                        children: vec![text("header")],
                        position: None,
                    })],
                    position: None,
                })],
                position: None,
                align: vec![AlignKind::None],
            }),
        ],
    });
    let mut renderer = HtmlRenderer::new();
    let output = tree.render_with(&mut renderer);

    assert!(output.contains("<li><p>ordinary item</p></li>"));
    assert!(output.contains("<thead>"));
    assert!(!output.contains("<tbody>"));
}

#[test]
fn suppresses_inline_markup_inside_image_alt_text() -> Result<(), message::Message> {
    assert_eq!(
        to_html_with_options("![`code`](image)", &Options::default())?,
        "<p><img src=\"image\" alt=\"code\" /></p>"
    );

    Ok(())
}
