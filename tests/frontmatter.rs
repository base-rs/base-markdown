// Released under the MIT License.
// Copyright, 2022, by Bernhard Berger.
// Copyright, 2022-2025, by Titus Wormer.
// Copyright, 2024, by Nokome Bentley.
// Copyright, 2026, by Samuel Williams.

use core_markdown::{
    mdast::{Frontmatter, Node, Root},
    message, to_html, to_html_with_options, to_mdast,
    unist::Position,
    Constructs, Options, ParseOptions,
};
use pretty_assertions::assert_eq;

#[test]
fn frontmatter() -> Result<(), message::Message> {
    let frontmatter = Options {
        parse: ParseOptions {
            constructs: Constructs {
                frontmatter: true,
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    };

    assert_eq!(
        to_html("---\ntitle: Jupyter\n---"),
        "<hr />\n<h2>title: Jupyter</h2>",
        "should not support frontmatter by default"
    );

    assert_eq!(
        to_html_with_options("---\ntitle: Jupyter\n---", &frontmatter)?,
        "",
        "should support frontmatter (yaml)"
    );

    assert_eq!(
        to_html_with_options("+++\ntitle = \"Jupyter\"\n+++", &frontmatter)?,
        "",
        "should support frontmatter (toml)"
    );

    assert_eq!(
        to_html_with_options("---\n---", &frontmatter)?,
        "",
        "should support empty frontmatter"
    );

    assert_eq!(
        to_html_with_options("--\n---", &frontmatter)?,
        "<h2>--</h2>",
        "should not support 2 markers in an opening fence"
    );

    assert_eq!(
        to_html_with_options("----\n---", &frontmatter)?,
        "<hr />\n<hr />",
        "should not support 4 markers in an opening fence"
    );

    assert_eq!(
        to_html_with_options("---\n--", &frontmatter)?,
        "<hr />\n<p>--</p>",
        "should not support 2 markers in a closing fence"
    );

    assert_eq!(
        to_html_with_options("---\n--\n", &frontmatter)?,
        "<hr />\n<p>--</p>\n",
        "should not panic if newline after 2 marker closing fence"
    );

    assert_eq!(
        to_html_with_options("---\n----", &frontmatter)?,
        "<hr />\n<hr />",
        "should not support 4 markers in a closing fence"
    );

    assert_eq!(
        to_html_with_options("---\n---\n## Neptune", &frontmatter)?,
        "<h2>Neptune</h2>",
        "should support content after frontmatter"
    );

    assert_eq!(
        to_html_with_options("--- \t\n---", &frontmatter)?,
        "",
        "should support spaces and tabs after opening fence"
    );

    assert_eq!(
        to_html_with_options("---\n---\t ", &frontmatter)?,
        "",
        "should support spaces and tabs after closing fence"
    );

    assert_eq!(
        to_html_with_options("---\n---\na\nb", &frontmatter)?,
        "<p>a\nb</p>",
        "should support line endings after frontmatter"
    );

    assert_eq!(
        to_html_with_options("--- a\n---", &frontmatter)?,
        "",
        "should support a format hint after the opening fence"
    );

    assert_eq!(
        to_html_with_options("---\n--- b", &frontmatter)?,
        "<hr />\n<p>--- b</p>",
        "should not support content after closing fence"
    );

    assert_eq!(
        to_html_with_options("## Neptune\n---\n---", &frontmatter)?,
        "<h2>Neptune</h2>\n<hr />\n<hr />",
        "should not support frontmatter after content"
    );

    assert_eq!(
        to_html_with_options("> ---\n> ---\n> ## Neptune", &frontmatter)?,
        "<blockquote>\n<hr />\n<hr />\n<h2>Neptune</h2>\n</blockquote>",
        "should not support frontmatter in a container"
    );

    assert_eq!(
        to_html_with_options("---", &frontmatter)?,
        "<hr />",
        "should not support just an opening fence"
    );

    assert_eq!(
        to_html_with_options("---\ntitle: Neptune", &frontmatter)?,
        "<hr />\n<p>title: Neptune</p>",
        "should not support a missing closing fence"
    );

    assert_eq!(
        to_html_with_options("---\na\n\nb\n \t\nc\n---", &frontmatter)?,
        "",
        "should support blank lines in frontmatter"
    );

    assert_eq!(
        to_mdast("---\na: b\n---", &frontmatter.parse)?,
        Node::Root(Root {
            children: vec![Node::Frontmatter(Frontmatter {
                info: String::new(),
                fence: '-',
                fence_length: 3,
                closing_fence_length: 3,
                value: "a: b\n".into(),
                position: Some(Position::new(1, 1, 0, 3, 4, 12))
            })],
            position: Some(Position::new(1, 1, 0, 3, 4, 12))
        }),
        "should support yaml as `Frontmatter` in mdast"
    );

    assert_eq!(
        to_mdast("+++\ntitle = \"Jupyter\"\n+++", &frontmatter.parse)?,
        Node::Root(Root {
            children: vec![Node::Frontmatter(Frontmatter {
                info: String::new(),
                fence: '+',
                fence_length: 3,
                closing_fence_length: 3,
                value: "title = \"Jupyter\"\n".into(),
                position: Some(Position::new(1, 1, 0, 3, 4, 25))
            })],
            position: Some(Position::new(1, 1, 0, 3, 4, 25))
        }),
        "should support toml as `Frontmatter` in mdast"
    );

    Ok(())
}

fn parse_options() -> ParseOptions {
    ParseOptions {
        constructs: Constructs {
            frontmatter: true,
            ..Constructs::default()
        },
        ..ParseOptions::default()
    }
}

fn parse_frontmatter(source: &str) -> Frontmatter {
    let document = to_mdast(source, &parse_options()).unwrap();
    match &document.children().unwrap()[0] {
        Node::Frontmatter(node) => node.clone(),
        node => panic!("expected frontmatter for {:?}, got {:?}", source, node),
    }
}

#[test]
fn delimiter_defaults_keep_the_authored_info_and_body() {
    for (source, language, info, body) in [
        ("---\nname: Neptune\n---", "yaml", "", "name: Neptune\n"),
        (
            "+++\nname = 'Neptune'\n+++",
            "toml",
            "",
            "name = 'Neptune'\n",
        ),
        ("--- \t\n---", "yaml", "", ""),
        ("+++\n+++", "toml", "", ""),
        ("---\r\na: b\r\n---", "yaml", "", "a: b\r\n"),
        ("+++\r\na = 1\r\n+++", "toml", "", "a = 1\r\n"),
        ("+++\ra = 1\r+++", "toml", "", "a = 1\r"),
        ("---\n\na: b\n\n---", "yaml", "", "\na: b\n\n"),
        ("--- json title=x\n{}\n---", "json", "json title=x", "{}\n"),
        ("+++ yaml\na: b\n+++", "yaml", "yaml", "a: b\n"),
    ] {
        let node = parse_frontmatter(source);
        assert_eq!(node.language(), Some(language), "{source:?}");
        assert_eq!(node.info, info);
        assert_eq!(node.value, body);
        let ast = Node::Frontmatter(node);
        assert_eq!(ast.code_language(), Some(language));
        assert_eq!(ast.code_info().as_deref(), Some(info));

        let output = ast.to_markdown();
        let reparsed = parse_frontmatter(&output);
        assert_eq!(reparsed.language(), Some(language));
        assert_eq!(reparsed.info, info);
        assert_eq!(reparsed.value, body);
        let fence = reparsed.fence.to_string().repeat(3);
        let opening = if info.is_empty() {
            fence
        } else {
            format!("{fence} {info}")
        };
        assert_eq!(output.lines().next(), Some(opening.as_str()));
    }
}

#[test]
fn tagged_frontmatter_keeps_opaque_info_and_literal_body() {
    for (source, fence, opening, closing, info, body) in [
        (
            "```json\n{\"name\": \"Neptune\"}\n```",
            '`',
            3,
            3,
            "json",
            "{\"name\": \"Neptune\"}\n",
        ),
        (
            "~~~~toml title=planet\nname = 'Neptune'\n~~~~~",
            '~',
            4,
            5,
            "toml title=planet",
            "name = 'Neptune'\n",
        ),
        (
            "---yaml\nname: Neptune\n---",
            '-',
            3,
            3,
            "yaml",
            "name: Neptune\n",
        ),
        ("--- json\n{}\n---", '-', 3, 3, "json", "{}\n"),
        (
            "+++ custom\nraw &amp; \\* text\n+++",
            '+',
            3,
            3,
            "custom",
            "raw &amp; \\* text\n",
        ),
        (
            "``` \tjson\\+ &amp; title=planet \t\n\na\n\n \t\n```",
            '`',
            3,
            3,
            "json\\+ &amp; title=planet",
            "\na\n\n \t\n",
        ),
        ("~~~\u{a0}\nraw\n~~~", '~', 3, 3, "\u{a0}", "raw\n"),
        ("\u{feff}```json\n{}\n```", '`', 3, 3, "json", "{}\n"),
        ("~~~\u{c}\nraw\n~~~", '~', 3, 3, "\u{c}", "raw\n"),
        (
            "~~~ ~format metadata\nraw\n~~~",
            '~',
            3,
            3,
            "~format metadata",
            "raw\n",
        ),
        ("~~~json\n~~~", '~', 3, 3, "json", ""),
        ("```json\r\n{}\r\n```", '`', 3, 3, "json", "{}\r\n"),
        ("~~~json\r{}\r~~~", '~', 3, 3, "json", "{}\r"),
        (
            "~~~some`format\nraw\n   ~~~~ \t",
            '~',
            3,
            4,
            "some`format",
            "raw\n",
        ),
        ("```json\n{}\n  ```", '`', 3, 3, "json", "{}\n"),
    ] {
        let node = parse_frontmatter(source);
        assert_eq!(node.info, info, "{source:?}");
        assert_eq!(node.value, body, "{source:?}");
        assert_eq!(
            (node.fence, node.fence_length, node.closing_fence_length),
            (fence, opening, closing)
        );
        assert_eq!(node.language(), info.split_ascii_whitespace().next());
        let document = to_mdast(source, &parse_options()).unwrap();
        let output = document.to_markdown();
        let reparsed = parse_frontmatter(&output);
        assert_eq!(reparsed.info, info);
        assert_eq!(reparsed.value, body);
        assert_eq!(
            (
                reparsed.fence,
                reparsed.fence_length,
                reparsed.closing_fence_length
            ),
            (fence, opening, closing)
        );
    }
}

#[test]
fn only_closed_tagged_fences_at_document_start_are_frontmatter() {
    for source in [
        "```\nbody\n```",
        "~~~ \t\nbody\n~~~",
        "```json",
        "~~~json\nbody",
        "```json\nbody\n~~~",
        "````json\nbody\n```",
        "```json`bad\nbody\n```",
        "``json\nbody\n``",
        "~~json\nbody\n~~",
        " ```json\nbody\n ```",
        "\n```json\nbody\n```",
        "text\n\n~~~json\nbody\n~~~",
        "> ```json\n> body\n> ```",
        "- ~~~json\n  body\n  ~~~",
        "~~~json\nbody\n    ~~~",
        "```json\n{}\n\t```",
        "---json",
        "+++json\nbody",
        "----json\nbody\n----",
    ] {
        let document = to_mdast(source, &parse_options()).unwrap();
        assert!(
            !document
                .children()
                .unwrap()
                .iter()
                .any(|node| matches!(node, Node::Frontmatter(_))),
            "{:?}",
            source
        );
    }

    for source in [
        "```json\n{}\n```",
        "~~~toml\na = 1\n~~~",
        "---json\n{}\n---",
    ] {
        let document = to_mdast(source, &ParseOptions::default()).unwrap();
        assert!(!document
            .children()
            .unwrap()
            .iter()
            .any(|node| matches!(node, Node::Frontmatter(_))));
    }
}

#[test]
fn nonclosing_fence_lines_remain_in_the_body() {
    let source = "````json\n```\n~~~~\n```` suffix\n    ````\n`````";
    assert_eq!(
        parse_frontmatter(source).value,
        "```\n~~~~\n```` suffix\n    ````\n"
    );
    assert_eq!(
        parse_frontmatter("---custom\n----\n--- suffix\n---").value,
        "----\n--- suffix\n"
    );
    assert_eq!(
        parse_frontmatter("~~~json\nbody\n~~~\n\n```rust\ncode\n```").value,
        "body\n"
    );
}

#[test]
fn both_html_paths_omit_frontmatter_and_keep_later_code() {
    let options = Options {
        parse: parse_options(),
        ..Options::default()
    };
    let source = "--- json\n{}\n---\n\n# Neptune\n\n```rust\nlet x = 1;\n```";
    let expected = "<h1>Neptune</h1>\n<pre><code class=\"language-rust\">let x = 1;\n</code></pre>";
    assert_eq!(to_html_with_options(source, &options).unwrap(), expected);
    let document = to_mdast(source, &options.parse).unwrap();
    assert_eq!(
        document.render_with(&mut core_markdown::HtmlRenderer::new()),
        expected
    );
    assert_eq!(document.children().unwrap().len(), 3);
    assert!(matches!(&document.children().unwrap()[2], Node::Code(_)));
}

#[test]
fn frontmatter_ast_helpers_and_source_positions() {
    let mut node = Node::Frontmatter(parse_frontmatter("```json title=x\n{}\n```"));
    assert_eq!(node.code_language(), Some("json"));
    assert_eq!(node.code_info().as_deref(), Some("json title=x"));
    assert_eq!(node.text_content(), "{}\n");
    assert_eq!(node.to_string(), "{}\n");
    assert!(format!("{node:?}").starts_with("Frontmatter {"));
    assert!(node.children().is_none());
    assert!(node.children_mut().is_none());
    assert_eq!(node.position(), Some(&Position::new(1, 1, 0, 3, 4, 22)));
    node.position_mut().unwrap().start.line = 2;
    assert_eq!(node.position().unwrap().start.line, 2);
    node.position_set(None);
    assert_eq!(node.position(), None);
    let mut visited = 0;
    node.walk(&mut |_| visited += 1);
    assert_eq!(visited, 1);
}

#[test]
fn serialization_protects_edited_frontmatter_bodies() {
    for (source, body, expected) in [
        (
            "```json\n{}\n```",
            "a\n```\nb\n",
            "````json\na\n```\nb\n````\n",
        ),
        ("~~~custom\n~~~", "~~~~\n", "~~~~~custom\n~~~~\n~~~~~\n"),
        ("--- json\n---", "--- \t\n", "~~~json\n--- \t\n~~~\n"),
        ("+++ json\n+++", "+++\rbody\r", "~~~json\n+++\rbody\r~~~\n"),
        ("--- json\n---", "value", "--- json\nvalue\n---\n"),
        ("~~~json\n~~~~~", "", "~~~json\n~~~~~\n"),
        ("---\n---", "---\n", "~~~yaml\n---\n~~~\n"),
        ("+++\n+++", "+++\n", "~~~toml\n+++\n~~~\n"),
        ("---\n---", "value", "---\nvalue\n---\n"),
        ("+++\n+++", "value", "+++\nvalue\n+++\n"),
        ("---\u{c}\n---", "---\n", "~~~yaml\n---\n~~~\n"),
    ] {
        let mut frontmatter = parse_frontmatter(source);
        let language = frontmatter.language().map(str::to_owned);
        frontmatter.value = body.into();
        let output = Node::Frontmatter(frontmatter).to_markdown();
        assert_eq!(output, expected);
        assert_eq!(parse_frontmatter(&output).language(), language.as_deref());
        assert_eq!(
            parse_frontmatter(&output).value,
            if body.is_empty() || body.ends_with(['\r', '\n']) {
                body.into()
            } else {
                format!("{body}\n")
            }
        );
    }
}

#[test]
fn serialization_rejects_invalid_edited_frontmatter_metadata() {
    for (fence, info) in [
        ('!', "json"),
        ('~', ""),
        ('~', " \t"),
        ('~', "json\nother"),
        ('~', "json\rother"),
        ('`', "json`other"),
    ] {
        let mut frontmatter = parse_frontmatter("~~~json\n~~~");
        frontmatter.fence = fence;
        frontmatter.info = info.into();
        let result = core_markdown::to_markdown(&Node::Frontmatter(frontmatter));
        assert_eq!(*result.unwrap_err().rule_id, "invalid-frontmatter");
    }
}

#[cfg(feature = "serde")]
#[test]
fn frontmatter_serde_roundtrip_preserves_body_and_fences() {
    let mut node = parse_frontmatter("~~~~custom title=x\nraw\n~~~~~");
    let json = serde_json::to_value(&node).unwrap();
    assert_eq!(json["info"], "custom title=x");
    assert_eq!(json["value"], "raw\n");
    assert_eq!(json["fenceLength"], 4);
    assert_eq!(json["closingFenceLength"], 5);
    assert_eq!(
        serde_json::from_value::<core_markdown::mdast::Frontmatter>(json).unwrap(),
        node
    );
    let ast = Node::Frontmatter(node.clone());
    assert_eq!(
        serde_json::from_str::<Node>(&serde_json::to_string(&ast).unwrap()).unwrap(),
        ast
    );
    node.position = None;
    assert!(serde_json::to_value(&node)
        .unwrap()
        .get("position")
        .is_none());
}

#[test]
fn frontmatter_does_not_require_the_code_fenced_construct() {
    let mut options = parse_options();
    options.constructs.code_fenced = false;
    let document = to_mdast("```json\n{}\n```", &options).unwrap();
    assert!(matches!(
        &document.children().unwrap()[0],
        Node::Frontmatter(_)
    ));
}
