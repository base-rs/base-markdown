// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use base_markdown::stream::{EventKind, Tag};
use base_markdown::{
    message, to_html_with_options, CompileOptions, Document, Options, ParseOptions,
};
use pretty_assertions::assert_eq;

fn render(
    source: &str,
    parse_options: &ParseOptions,
    compile_options: CompileOptions,
) -> Result<String, message::Message> {
    let document = Document::parse(source, parse_options).unwrap();
    let mut output = String::new();
    document.render_html(&compile_options, &mut output).unwrap();
    Ok(output)
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
        "<p><a href=\"/guide\" title=\"Guide\">hello</a></p>\n"
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
fn renders_tables_and_task_lists() -> Result<(), message::Message> {
    assert_eq!(
        render(
            "- [x] done\n\n| A | B |\n| :- | -: |\n| one | two |",
            &ParseOptions::gfm(),
            CompileOptions::default(),
        )?,
        "<ul>\n<li><input type=\"checkbox\" disabled=\"\" checked=\"\" /> done</li>\n</ul>\n<table>\n<thead>\n<tr>\n<th align=\"left\">A</th>\n<th align=\"right\">B</th>\n</tr>\n</thead>\n<tbody>\n<tr>\n<td align=\"left\">one</td>\n<td align=\"right\">two</td>\n</tr>\n</tbody>\n</table>"
    );

    Ok(())
}

#[test]
fn renders_footnotes() -> Result<(), message::Message> {
    assert_eq!(
        render(
            "Call.[^note]\n\n[^note]: details",
            &ParseOptions::gfm(),
            CompileOptions::default(),
        )?,
        "<p>Call.<sup><a href=\"#user-content-fn-note\" id=\"user-content-fnref-note\" data-footnote-ref=\"\" aria-describedby=\"footnote-label\">1</a></sup></p>\n<section data-footnotes=\"\" class=\"footnotes\"><h2 id=\"footnote-label\" class=\"sr-only\">Footnotes</h2>\n<ol>\n<li id=\"user-content-fn-note\">\n<p>details <a href=\"#user-content-fnref-note\" data-footnote-backref=\"\" aria-label=\"Back to content\" class=\"data-footnote-backref\">↩</a></p>\n</li>\n</ol>\n</section>\n"
    );

    Ok(())
}

#[test]
fn renders_constructed_nodes_and_subtrees() {
    let mut document = Document::parse(String::new(), &ParseOptions::default()).unwrap();
    let root = document.root();
    let heading = document
        .append_child(root, EventKind::Start(Tag::Heading(2)))
        .unwrap();
    document
        .append_child(heading, EventKind::Text("A heading".to_string().into()))
        .unwrap();
    let quote = document
        .append_child(root, EventKind::Start(Tag::BlockQuote))
        .unwrap();
    let paragraph = document
        .append_child(quote, EventKind::Start(Tag::Paragraph { tight: false }))
        .unwrap();
    let text = document
        .append_child(paragraph, EventKind::Text("quoted".to_string().into()))
        .unwrap();
    let image = document
        .append_child(
            root,
            EventKind::Start(Tag::Image {
                destination: "javascript:bad()".to_string().into(),
                title: None,
            }),
        )
        .unwrap();
    document
        .append_child(image, EventKind::Text("image".to_string().into()))
        .unwrap();
    let options = CompileOptions {
        heading_ids: true,
        ..Default::default()
    };
    for (node, expected) in [
        (heading, "<h2 id=\"a-heading\">A heading</h2>"),
        (quote, "<blockquote>\n<p>quoted</p>\n</blockquote>"),
        (paragraph, "<p>quoted</p>"),
        (text, "quoted"),
        (image, "<img src=\"\" alt=\"image\" />"),
    ] {
        let mut output = String::new();
        document.render_node(node, &options, &mut output).unwrap();
        assert_eq!(output, expected);
    }
}

#[test]
fn covers_parser_html_options_and_generated_heading_ids() {
    let mut parse_options = ParseOptions::gfm();
    parse_options.inline_code_info = true;
    parse_options.constructs.math_text = true;
    parse_options.constructs.math_flow = true;
    let options = Options {
        parse: parse_options,
        compile: CompileOptions {
            heading_ids: true,
            task_list_item_checkable: false,
            ..CompileOptions::default()
        },
    };
    let html = to_html_with_options(
        "# Same\n\n# Same\n\nUse ruby:`puts 1` and $x$.\n\n- [x] done\n\n$$\nx^2\n$$",
        &options,
    );

    assert!(html.contains("<h1 id=\"same\">Same</h1>"));
    assert!(html.contains("<h1 id=\"same-2\">Same</h1>"));
    assert!(html.contains("<code class=\"language-ruby\">puts 1</code>"));
    assert!(html.contains("<code class=\"language-math math-inline\">x</code>"));
    assert!(html.contains("disabled=\"\""), "{:?}", html);
    assert!(html.contains("<code class=\"language-math math-display\">x^2"));
}

#[test]
fn renders_unchecked_items_and_header_only_tables() {
    let source = "- ordinary item\n\n| header |\n| --- |";
    let document = Document::parse(source, &ParseOptions::gfm()).unwrap();
    let mut output = String::new();
    document
        .render_html(&CompileOptions::default(), &mut output)
        .unwrap();
    assert!(output.contains("<li>ordinary item</li>"));
    assert!(output.contains("<thead>"));
    assert!(!output.contains("<tbody>"));
}

#[test]
fn suppresses_inline_markup_inside_image_alt_text() {
    assert_eq!(
        to_html_with_options("![`code`](image)", &Options::default()),
        "<p><img src=\"image\" alt=\"code\" /></p>"
    );
}
