// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

mod support;
use base_markdown::{to_markdown, CompileOptions, Document, MarkdownOptions, ParseOptions};
use pretty_assertions::assert_eq;
use support::fixture::to_markdown as to;
use support::fixture::{Break, Html, Node, Paragraph, Text};
use support::round_trip::{round_trip, Outcome};

#[test]
fn html() {
    assert_eq!(
        to(&Node::Html(Html {
            value: String::from("<a>\nb"),
            position: None
        }))
        .unwrap(),
        "<a>\nb\n",
        "should support html"
    );

    assert_eq!(
        to(&Node::Paragraph(Paragraph {
            children: vec![
                Node::Text(Text {
                    value: "a\n".to_string(),
                    position: None
                }),
                Node::Html(Html {
                    value: "<div>".to_string(),
                    position: None
                })
            ],
            position: None
        }))
        .unwrap(),
        "a\n    <div>\n",
        "should prevent html (text) from becoming html (flow) (1), indenting the line so it continues the paragraph"
    );

    assert_eq!(
        to(&Node::Paragraph(Paragraph {
            children: vec![
                Node::Text(Text {
                    value: "a\r".to_string(),
                    position: None
                }),
                Node::Html(Html {
                    value: "<div>".to_string(),
                    position: None
                })
            ],
            position: None
        }))
        .unwrap(),
        "a&#xD;<div>\n",
        "should prevent html (text) from becoming html (flow) (2), with a CR written as a reference, as output line endings are LF"
    );

    assert_eq!(
        to(&Node::Paragraph(Paragraph {
            children: vec![
                Node::Text(Text {
                    value: "a\r\n".to_string(),
                    position: None
                }),
                Node::Html(Html {
                    value: "<div>".to_string(),
                    position: None
                })
            ],
            position: None
        }))
        .unwrap(),
        "a&#xD;\n    <div>\n",
        "should prevent html (text) from becoming html (flow) (3)"
    );

    assert_eq!(
        to(&Node::Paragraph(Paragraph {
            children: vec![
                Node::Html(Html {
                    value: "<x>".to_string(),
                    position: None
                }),
                Node::Text(Text {
                    value: "a".to_string(),
                    position: None
                })
            ],
            position: None
        }))
        .unwrap(),
        "<x>a\n",
        "should serialize html (text)"
    );

    assert_eq!(
        to(&Node::Paragraph(Paragraph {
            children: vec![
                Node::Text(Text {
                    value: "a\n".to_string(),
                    position: None
                }),
                Node::Html(Html {
                    value: "<span>".to_string(),
                    position: None
                })
            ],
            position: None
        }))
        .unwrap(),
        "a\n<span>\n",
        "should keep the line ending before html (text) that cannot interrupt a paragraph"
    );

    assert_eq!(
        to(&Node::Paragraph(Paragraph {
            children: vec![
                Node::Text(Text {
                    value: "a".to_string(),
                    position: None
                }),
                Node::Break(Break { position: None }),
                Node::Text(Text {
                    value: String::new(),
                    position: None
                }),
                Node::Html(Html {
                    value: "<!-- m -->".to_string(),
                    position: None
                })
            ],
            position: None
        }))
        .unwrap(),
        "a\\\n    <!-- m -->\n",
        "should indent html (text) after a hard break so the break is kept"
    );
}

#[test]
fn html_text_after_phrasing_round_trips_unchanged() {
    for source in [
        "a <!-- m -->\n",
        "- `c` <!-- m -->\n",
        "plain <b>x</b>\n",
        "a<span>b</span>\n",
        "a\n<span>\n",
        "a\n<span\nx>\n",
        "a\\\n</span>\n",
        "a\\\n    <!-- m -->\n",
        "*a*\\\n    <?x?> b\n",
        "> a\\\n>     <div>\n",
        "- a\\\n      <![CDATA[x]]>\n",
    ] {
        let document = Document::parse(source, &ParseOptions::default()).unwrap();
        assert_eq!(to_markdown(&document).unwrap(), source, "{source:?}");
    }
}

#[test]
fn trailing_space_hard_break_before_html_text_is_kept() {
    let document = Document::parse("a  \n</span>\n", &ParseOptions::default()).unwrap();
    assert_eq!(to_markdown(&document).unwrap(), "a\\\n</span>\n");
}

/// A complete tag alone on the first line of a paragraph would start an HTML block, which it
/// cannot after other text, so the line ending after it is encoded (#146).
#[test]
fn complete_tag_alone_on_a_paragraph_first_line_stays_inline() {
    let html = |value: &str| {
        Node::Html(Html {
            value: value.to_string(),
            position: None,
        })
    };
    let text = |value: &str| {
        Node::Text(Text {
            value: value.to_string(),
            position: None,
        })
    };
    for (children, expected) in [
        (vec![html("<b>"), text("\nx")], "<b>&#xA;x\n"),
        (vec![html("<b>"), text(" \nx")], "<b>&#x20;\nx\n"),
        (vec![html("</b>"), text("\nx")], "</b>&#xA;x\n"),
        (
            vec![html("<b a=\"1\">"), text("\nx")],
            "<b a=\"1\">&#xA;x\n",
        ),
        // Other text on its line, or text before it, keeps it inline.
        (vec![html("<b>"), text("x\ny")], "<b>x\ny\n"),
        (vec![text("a\n"), html("<b>"), text("\nx")], "a\n<b>\nx\n"),
    ] {
        let markdown = to(&Node::Paragraph(Paragraph {
            children,
            position: None,
        }))
        .unwrap();
        assert_eq!(markdown, expected);
        let document = Document::parse(markdown.as_str(), &ParseOptions::default()).unwrap();
        let mut rendered = String::new();
        document
            .render_html(&Default::default(), &mut rendered)
            .unwrap();
        assert!(rendered.starts_with("<p>"), "{:?}", rendered);
        assert_eq!(to_markdown(&document).unwrap(), markdown);
    }
}

/// Parse `source`, write it, and check that the output is `written` and reads back as the same
/// tree and HTML (contract 3.3).
fn assert_written(source: &str, written: &str) {
    let document = Document::parse(source, &ParseOptions::default()).unwrap();
    assert_eq!(to_markdown(&document).unwrap(), written, "{:?}", source);
    match round_trip(document, &MarkdownOptions::default(), false) {
        Outcome::Pass => {}
        Outcome::Fail(failure) => panic!(
            "{} {:?}: {:?}\n{}\n{}",
            failure.kind, source, failure.output, failure.before, failure.after
        ),
        Outcome::Rejected | Outcome::Unwritable(_) => panic!("rejected {:?}", source),
    }
}

#[test]
fn html_block_indentation_keeps_its_columns() {
    let html = |source: &str| {
        let mut output = String::new();
        let options = CompileOptions {
            allow_dangerous_html: true,
            ..CompileOptions::default()
        };
        Document::parse(source, &ParseOptions::default())
            .unwrap()
            .render_html(&options, &mut output)
            .unwrap();
        output
    };
    // What is left of a tab a container takes part of is the block's, as cmark keeps it (#263,
    // #274). markdown-rs 1.0.0 drops it after an item's content column.
    assert_eq!(
        html("> \t<div>\n"),
        "<blockquote>\n\t<div>\n</blockquote>\n"
    );
    assert_eq!(
        html("> [a]: /u\n> \t<div>\n"),
        "<blockquote>\n\t<div>\n</blockquote>\n"
    );
    assert_eq!(
        html("> a\n>\n> \t<div>\n"),
        "<blockquote>\n<p>a</p>\n\t<div>\n</blockquote>\n"
    );
    assert_eq!(
        html("- <pre>\n\tx</pre>\n"),
        "<ul>\n<li>\n<pre>\n  x</pre>\n</li>\n</ul>\n"
    );
    for (source, written) in [
        ("> \t<div>\n", "> \t<div>\n"),
        ("- <pre>\n\tx</pre>\n", "- <pre>\n    x</pre>\n"),
        // The item keeps the column at which the tab is no wider than the block's indentation
        // allows (#266).
        ("-    a\n\n       \t<div>\n", "-    a\n\n       \t<div>\n"),
        ("1.   a\n\n      \t<div>\n", "1.   a\n\n      \t<div>\n"),
        ("- a\n\n  \t<div>\n", "- a\n\n  \t<div>\n"),
        ("   >   \t<pre>\n", "   >   \t<pre>\n"),
        // A list before an indented HTML block is written wider than its indentation (#262).
        ("  - i\n  <div>\n", "-  i\n\n  <div>\n"),
        ("-\tc\n  <!A\n", "-  c\n\n  <!A\n"),
        ("1.   a\n   <div>\n", "1.  a\n\n   <div>\n"),
        ("-   <!-- x -->\n  <textarea>\n", "-  <!-- x -->\n\n  <textarea>\n"),
        // An unclosed block of kinds 1–5 ending an item is joined to the next by a line ending,
        // and the list shows that it is loose by another join (#273).
        ("1. <!--x\n2. a\n\n   b\n", "1. <!--x\n2. a\n\n   b\n"),
        // A footnote definition's label takes the whitespace after it.
        ("[^1]:\n     <v>", "[^1]:\n     <v>\n"),
        (
            "-\t-\t  <a:b c=\"d\">\n       \t<![CDATA[\n1.    \t</style>\n  <a:b>\n",
            "-  - ```\n     <a:b c=\"d\">\n     ```\n      \t<![CDATA[\n1. ```\n    </style>\n   ```\n\n\\<a:b>\n",
        ),
    ] {
        assert_written(source, written);
    }
}

/// Parses `source` under `options`, and checks that it round trips when written with `markdown`
/// and renders `html`.
fn assert_round_trips(
    source: &str,
    options: &ParseOptions,
    markdown: &MarkdownOptions,
    html: &str,
) {
    let document = Document::parse(source, options).unwrap();
    let compile = CompileOptions {
        allow_dangerous_html: true,
        ..CompileOptions::default()
    };
    let mut output = String::new();
    document.render_html(&compile, &mut output).unwrap();
    assert_eq!(output, html, "{:?}", source);
    match round_trip(document, markdown, false) {
        Outcome::Pass => {}
        Outcome::Fail(failure) => panic!(
            "{} {:?}: {:?}\n{}\n{}",
            failure.kind, source, failure.output, failure.before, failure.after
        ),
        Outcome::Rejected | Outcome::Unwritable(_) => panic!("rejected {:?}", source),
    }
}

#[test]
fn html_blocks_continue_no_block_before_them() {
    let unindented = ParseOptions {
        constructs: base_markdown::Constructs {
            code_indented: false,
            ..base_markdown::Constructs::default()
        },
        ..ParseOptions::default()
    };
    // A footnote definition takes a line indented 4 columns or more after a blank line, so the
    // item keeps the block indented less, whatever its preferred padding.
    for indent in [
        base_markdown::IndentOptions::One,
        base_markdown::IndentOptions::Tab,
        base_markdown::IndentOptions::Mixed,
    ] {
        let markdown = MarkdownOptions {
            list_item_indent: indent,
            ..MarkdownOptions::default()
        };
        assert_round_trips(
            "- [^ab]: a\n\n  \t<div>\n",
            &unindented,
            &markdown,
            "<ul>\n<li>\n\t<div>\n</li>\n</ul>\n",
        );
        assert_round_trips(
            " 10. [^a]: a\n\n     \t<div>\n",
            &unindented,
            &markdown,
            "<ol start=\"10\">\n<li>\n\t<div>\n</li>\n</ol>\n",
        );
    }
    // An empty item takes no line after the blank line after it.
    for (source, html) in [
        (
            "-\n\n      <div>\n",
            "<ul>\n<li></li>\n</ul>\n      <div>\n",
        ),
        (
            "- a\n-\n\n      <div>\n",
            "<ul>\n<li>a</li>\n<li></li>\n</ul>\n      <div>\n",
        ),
        (
            "2)\n\n        <pre>x</pre>\n",
            "<ol start=\"2\">\n<li></li>\n</ol>\n        <pre>x</pre>\n",
        ),
    ] {
        assert_round_trips(source, &unindented, &MarkdownOptions::default(), html);
    }
    // An HTML block that keeps its blank lines takes a line indented at all after them, so the
    // block quote or list after it has no spaces before its `>` or marker.
    let blank_lines = ParseOptions {
        html_block_blank_lines: true,
        ..ParseOptions::default()
    };
    for (source, html) in [
        (
            "  > <div>\n  >\n  > > \t<p>\n",
            "<blockquote>\n<div>\n\n<blockquote>\n\t<p>\n</blockquote>\n</blockquote>",
        ),
        (
            "-  <div>\n\n   -\n     \t<p>\n",
            "<ul>\n<li>\n<div>\n\n<ul>\n<li>\n\t<p>\n</li>\n</ul>\n</li>\n</ul>",
        ),
        (
            "- <div>\n    x\n\n   - a\n",
            "<ul>\n<li>\n<div>\n  x\n\n<ul>\n<li>a</li>\n</ul>\n</li>\n</ul>\n",
        ),
    ] {
        assert_round_trips(source, &blank_lines, &MarkdownOptions::default(), html);
    }
    // At the root, a block after a footnote definition indented 4 columns has no other column.
    let mut document =
        Document::parse("[^ab]: x\n\n<p>\n\n    <a href=\"x\">\n", &unindented).unwrap();
    let block = document.children(document.root()).nth(1).unwrap();
    document.detach(block).unwrap();
    assert_eq!(
        to_markdown(&document).unwrap_err().rule_id,
        "unwritable-node"
    );
}

#[test]
fn html_blocks_with_no_column_are_unwritable() {
    let options = ParseOptions::default();
    // A tab after a space is 4 columns wide at the root.
    let mut document = Document::parse("<div>", &options).unwrap();
    let block = document.children(document.root()).next().unwrap();
    document
        .set_kind(
            block,
            base_markdown::stream::EventKind::Html {
                value: base_markdown::stream::Text::Literal(" \t<div>"),
                block: true,
            },
        )
        .unwrap();
    assert_eq!(
        to_markdown(&document).unwrap_err().rule_id,
        "unwritable-node"
    );
    // An item is at most 8 columns wide with a bullet, so a block indented 8 columns after its
    // list, which only options without indented code read, would continue it.
    let unindented = ParseOptions {
        constructs: base_markdown::Constructs {
            code_indented: false,
            ..base_markdown::Constructs::default()
        },
        ..ParseOptions::default()
    };
    let mut document = Document::parse("- x\n\nb\n\n        <div>\n", &unindented).unwrap();
    let paragraph = document
        .children(document.root())
        .find(|&id| {
            matches!(
                document.node(id).unwrap().kind,
                base_markdown::stream::EventKind::Start(
                    base_markdown::stream::Tag::Paragraph { .. }
                )
            )
        })
        .unwrap();
    document.detach(paragraph).unwrap();
    assert_eq!(
        to_markdown(&document).unwrap_err().rule_id,
        "unwritable-node"
    );
    // A loose list that only a blank line after an unclosed block would show loose.
    let mut document = Document::parse("- <pre>\n- a\n", &options).unwrap();
    let list = document.children(document.root()).next().unwrap();
    document
        .set_kind(
            list,
            base_markdown::stream::EventKind::Start(base_markdown::stream::Tag::List {
                start: None,
                tight: false,
            }),
        )
        .unwrap();
    document.normalize();
    assert_eq!(
        to_markdown(&document).unwrap_err().rule_id,
        "unwritable-node"
    );
}

/// Sets the kind of the HTML block at `path`, by child index from the root, to one of `value`.
fn set_html(document: &mut Document<String>, path: &[usize], value: &'static str) {
    let mut id = document.root();
    for &index in path {
        id = document.children(id).nth(index).unwrap();
    }
    document
        .set_kind(
            id,
            base_markdown::stream::EventKind::Html {
                value: base_markdown::stream::Text::Literal(value),
                block: true,
            },
        )
        .unwrap();
}

#[test]
fn html_block_containers_that_cannot_be_written_are_unwritable() {
    let options = ParseOptions::default();
    // An item that starts with a blank line cannot interrupt the paragraph before its list.
    let mut document = Document::parse(String::from("- a\n  - <pre>\n"), &options).unwrap();
    set_html(&mut document, &[0, 0, 1, 0, 0], " <pre>");
    assert_eq!(
        to_markdown(&document).unwrap_err().rule_id,
        "unwritable-node"
    );
    // The block in the item fits only the columns that a line indented 3 columns, which the
    // item must be wider than, takes.
    let mut document = Document::parse(String::from("- <div>\n\n<div>\n"), &options).unwrap();
    set_html(&mut document, &[0, 0, 0], "\t <div>");
    set_html(&mut document, &[2], "   <div>");
    assert_eq!(
        to_markdown(&document).unwrap_err().rule_id,
        "unwritable-node"
    );
    // Code that a tight item keeps after a paragraph needs a fence.
    let unfenced = ParseOptions {
        constructs: base_markdown::Constructs {
            code_fenced: false,
            ..base_markdown::Constructs::default()
        },
        ..ParseOptions::default()
    };
    let mut document = Document::parse(String::from("- a\n"), &unfenced).unwrap();
    let list = document.children(document.root()).next().unwrap();
    let item = document.children(list).next().unwrap();
    document
        .append_child(
            item,
            base_markdown::stream::EventKind::CodeBlock {
                value: base_markdown::stream::Text::Literal("code\n"),
                info: None,
                fence: None,
                unclosed: false,
            },
        )
        .unwrap();
    assert_eq!(
        to_markdown(&document).unwrap_err().rule_id,
        "unwritable-node"
    );
}

/// Each indentation, kind of HTML block, open or closed, and container, with a list before or
/// after, round trips, with indented code or without (#275).
#[test]
fn html_blocks_round_trip_in_every_container() {
    let indents = ["", " ", "  ", "   ", " \t", "  \t", "\t", " \t "];
    let blocks = [
        "<pre>",
        "<pre></pre>",
        "<!-- x",
        "<!-- x -->",
        "<?x",
        "<?x?>",
        "<!X",
        "<!X>",
        "<![CDATA[",
        "<![CDATA[]]>",
        "<div>",
        "<a href=\"x\">",
    ];
    // Where each container starts the block, first in it or after a paragraph, and the prefix of
    // the line after it.
    let mut containers = Vec::new();
    for pre in 0..4 {
        for pad in 1..5 {
            let marker = format!("{}-{}", " ".repeat(pre), " ".repeat(pad));
            let rest = " ".repeat(pre + 1 + pad);
            containers.push((marker.clone(), rest.clone()));
            containers.push((format!("{}a\n\n{}", marker, rest), rest));
        }
    }
    for spaces in 0..4 {
        let marker = format!("{}> ", " ".repeat(spaces));
        containers.push((marker.clone(), marker.clone()));
        containers.push((format!("{}a\n>\n{}", marker, marker), marker));
    }
    containers.push(("[^1]:\n    ".into(), "    ".into()));
    containers.push(("[^1]: a\n\n    ".into(), "    ".into()));
    // No list, or a tight or loose one before or after.
    let lists = [
        ("", ""),
        ("- a\n", ""),
        ("1. a\n\n2. b\n", ""),
        ("", "- a\n"),
        ("", "1. a\n\n2. b\n"),
    ];
    let unindented = ParseOptions {
        constructs: base_markdown::Constructs {
            code_indented: false,
            ..base_markdown::Constructs::default()
        },
        ..ParseOptions::default()
    };
    for options in [ParseOptions::default(), unindented] {
        for indent in indents {
            for block in blocks {
                for (start, rest) in &containers {
                    for (before, after) in lists {
                        let source = format!(
                            "{}{}{}{}\n{}c\n\n{}",
                            before, start, indent, block, rest, after
                        );
                        let document = Document::parse(source.as_str(), &options).unwrap();
                        match round_trip(document, &MarkdownOptions::default(), false) {
                            Outcome::Pass => {}
                            Outcome::Unwritable(_) | Outcome::Rejected => {
                                panic!("rejected {:?}", source)
                            }
                            Outcome::Fail(failure) => panic!(
                                "{} {:?}: {:?}\n{}\n{}",
                                failure.kind, source, failure.output, failure.before, failure.after
                            ),
                        }
                    }
                }
            }
        }
    }
}
