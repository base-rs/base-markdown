// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use super::*;
use crate::event::Kind;
use crate::{MdxExpressionKind, MdxSignal, ParseOptions};
use alloc::{string::String, vec};

fn point(index: usize) -> crate::event::Point {
    crate::event::Point {
        line: 1,
        column: index + 1,
        index,
        vs: 0,
    }
}

fn event_pair(name: Name, start: usize, end: usize) -> [Event; 2] {
    [
        Event {
            kind: Kind::Enter,
            name: name.clone(),
            point: point(start),
            link: None,
        },
        Event {
            kind: Kind::Exit,
            name,
            point: point(end),
            link: None,
        },
    ]
}

fn positioned_paragraph() -> Node {
    Node::Paragraph(Paragraph {
        children: vec![],
        position: Some(Position {
            start: Point::new(1, 1, 0),
            end: Point::new(1, 1, 0),
        }),
    })
}

fn positioned_text() -> Node {
    Node::Text(Text {
        value: String::new(),
        position: Some(Position {
            start: Point::new(1, 1, 0),
            end: Point::new(1, 1, 0),
        }),
    })
}

fn with_position(mut node: Node) -> Node {
    node.position_set(Some(Position::new(1, 1, 0, 1, 2, 1)));
    node
}

fn context_with_tail<'a>(events: &'a [Event], bytes: &'a [u8], tail: Node) -> CompileContext<'a> {
    let mut context = CompileContext::new(events, bytes, false);
    let (tree, stack, event_stack) = context.trees.first_mut().unwrap();
    tree.children_mut().unwrap().push(tail);
    stack.push(0);
    event_stack.push(0);
    context.index = events.len().saturating_sub(1);
    context
}

#[test]
fn compiles_extension_document() {
    let mut options = ParseOptions::mdx();
    options.constructs.frontmatter = true;
    options.constructs.gfm_autolink_literal = true;
    options.constructs.gfm_footnote_definition = true;
    options.constructs.gfm_label_start_footnote = true;
    options.constructs.gfm_strikethrough = true;
    options.constructs.gfm_table = true;
    options.constructs.gfm_task_list_item = true;
    options.constructs.math_flow = true;
    options.constructs.math_text = true;
    options.mdx_esm_parse = Some(Box::new(|_| MdxSignal::Ok));
    options.mdx_expression_parse = Some(Box::new(|_, _: &MdxExpressionKind| MdxSignal::Ok));

    let source = "---\ntitle: Example\n---\n\
import Component from 'component'\n\n\
# Heading\n\
Setext\n---\n\
<Component.Button disabled title=\"title\" count={1} {...props} />\n\
Text <Component.Item /> with {value}.\n\
{flowExpression}\n\
~~deleted~~ and $math$\n\
```rust meta\ncode\n```\n\
| left | center | right |\n\
| :--- | :----: | ----: |\n\
| `a\\|b` | ~~c~~ | d |\n\
| `plain` | ordinary | value |\n\
- [x] checked\n\
- [ ] unchecked\n\
[^note]: footnote\n\
Reference [link][label] and ![image][image].\n\
Unresolved footnote fallback [^missing](url).\n\
[label]: https://example.com \"title\"\n\
[image]: /image.png \"image title\"\n\
[^note]\n";
    let root = crate::to_mdast(source, &options).expect("compiles extension document");

    let Node::Root(root) = root else {
        panic!("compilation returns a root node");
    };

    assert!(root
        .children
        .iter()
        .any(|node| matches!(node, Node::Frontmatter(_))));
    assert!(root
        .children
        .iter()
        .any(|node| matches!(node, Node::MdxjsEsm(_))));
    assert!(root
        .children
        .iter()
        .any(|node| matches!(node, Node::Heading(_))));
    assert!(root
        .children
        .iter()
        .any(|node| matches!(node, Node::MdxJsxFlowElement(_))));
    assert!(root
        .children
        .iter()
        .any(|node| matches!(node, Node::Table(_))));
    assert!(root
        .children
        .iter()
        .any(|node| matches!(node, Node::List(_))));
    assert!(root
        .children
        .iter()
        .any(|node| matches!(node, Node::FootnoteDefinition(_))));
}

#[test]
fn compiles_html_autolinks_and_reference_forms() {
    let mut options = ParseOptions::gfm();
    options.constructs.frontmatter = true;
    let root = crate::to_mdast(
        "<https://example.com> <person@example.com>\n\
www.example.com mailto:person@example.com xmpp:user@example.com\n\
<!-- comment -->\n\
<?instruction?>\n\
<!doctype html>\n\
<![CDATA[data]]>\n\
<div>block</div>\n\
[label]: /full \"title\"\n\n\
a [full][label] b [label] c.\n\
![image][label] ![shortcut image]\n\
hard break  \nnext\\\n",
        &options,
    )
    .expect("compiles HTML, links, and references");

    let Node::Root(root) = root else {
        panic!("compilation returns a root node");
    };

    assert!(root
        .children
        .iter()
        .any(|node| matches!(node, Node::Html(_))));
    assert!(root
        .children
        .iter()
        .any(|node| matches!(node, Node::Paragraph(_))));
}

#[test]
fn recognizes_uppercase_gfm_autolink_prefixes() {
    let root = crate::to_mdast(
        "HTTPS://example.com and WWW.example.org",
        &ParseOptions::gfm(),
    )
    .expect("recognizes case-insensitive GFM autolinks");

    let mut urls = vec![];
    root.walk(|node| {
        if let Node::Link(link) = node {
            urls.push(link.url.as_str());
        }
    });

    assert_eq!(urls, ["HTTPS://example.com", "http://WWW.example.org"]);
}

#[test]
fn preserves_empty_link_labels() {
    let root = crate::to_mdast("[](/resource)", &ParseOptions::default())
        .expect("parses a resource with an empty label");

    let mut links = vec![];
    root.walk(|node| {
        if let Node::Link(link) = node {
            links.push(link);
        }
    });

    assert_eq!(links.len(), 1);
    assert_eq!(links[0].children.as_slice(), &[]);
    assert_eq!(links[0].url, "/resource");
}

#[test]
fn parses_frontmatter_at_end_of_input() {
    let options = ParseOptions {
        constructs: crate::Constructs {
            frontmatter: true,
            ..crate::Constructs::default()
        },
        ..ParseOptions::default()
    };

    crate::to_mdast("---\ntitle: Example\n---", &options)
        .expect("parses a closing frontmatter fence at EOF");
}

#[test]
fn parses_unicode_jsx_name_continuations() {
    crate::to_mdast(
        "<Ångström 属性=\"value\" /><ui:组件 data:值=\"value\" /><Component.名称 />",
        &ParseOptions::mdx(),
    )
    .expect("parses Unicode JSX element, member, and attribute names");
}

#[test]
fn parses_exported_mdx_esm() {
    let options = ParseOptions {
        mdx_esm_parse: Some(Box::new(|_| MdxSignal::Ok)),
        ..ParseOptions::mdx()
    };

    crate::to_mdast("export const value = 1", &options).expect("parses MDX export");
    crate::to_mdast("import value from 'module'", &options).expect("parses MDX import");
}

#[test]
fn reports_parse_errors_from_the_public_entry_point() {
    let options = ParseOptions {
        mdx_expression_parse: Some(Box::new(|_, _| {
            MdxSignal::Error(
                "invalid expression".into(),
                0,
                Box::new("test".into()),
                Box::new("invalid-expression".into()),
            )
        })),
        ..ParseOptions::mdx()
    };

    assert!(crate::to_mdast("{value}", &options).is_err());
    assert!(crate::to_mdast("> {value}", &options).is_err());
}

#[test]
fn updates_text_positions_when_removing_inline_code_info() {
    let events = [
        Event {
            kind: Kind::Enter,
            name: Name::CodeText,
            point: point(12),
            link: None,
        },
        Event {
            kind: Kind::Exit,
            name: Name::CodeText,
            point: point(18),
            link: None,
        },
    ];
    let text = Node::Text(Text {
        value: "before rust:".into(),
        position: Some(Position::new(1, 1, 0, 1, 13, 12)),
    });
    let paragraph = Node::Paragraph(Paragraph {
        children: vec![text],
        position: None,
    });
    let mut context = context_with_tail(&events, b"before rust:`code`", paragraph);
    context.inline_code_info = true;
    context.index = 0;

    on_enter_code_text(&mut context);
    context.resume();

    let Node::Paragraph(paragraph) = context.tail_penultimate_mut() else {
        panic!("the code span remains inside its paragraph");
    };
    let Node::Text(text) = &paragraph.children[0] else {
        panic!("the prefix remains text");
    };
    assert_eq!(text.value, "before ");
    assert_eq!(text.position.as_ref().unwrap().end, Point::new(1, 8, 7));
}

#[test]
fn removes_inline_code_info_when_the_text_has_no_position() {
    let events = [
        Event {
            kind: Kind::Enter,
            name: Name::CodeText,
            point: point(12),
            link: None,
        },
        Event {
            kind: Kind::Exit,
            name: Name::CodeText,
            point: point(18),
            link: None,
        },
    ];
    let paragraph = Node::Paragraph(Paragraph {
        children: vec![Node::Text(Text {
            value: "before rust:".into(),
            position: None,
        })],
        position: None,
    });
    let mut context = context_with_tail(&events, b"before rust:`code`", paragraph);
    context.inline_code_info = true;
    context.index = 0;

    on_enter_code_text(&mut context);
    context.resume();

    let Node::Paragraph(paragraph) = context.tail_penultimate_mut() else {
        panic!("the code span remains inside its paragraph");
    };
    let Node::Text(text) = &paragraph.children[0] else {
        panic!("the prefix remains text");
    };
    assert_eq!(text.value, "before ");
    assert!(text.position.is_none());
}

#[test]
fn propagates_raw_text_exit_errors_across_jsx_parent_boundaries() {
    let events = [
        Event {
            kind: Kind::Enter,
            name: Name::MdxJsxFlowTag,
            point: point(0),
            link: None,
        },
        Event {
            kind: Kind::Exit,
            name: Name::CodeText,
            point: point(1),
            link: None,
        },
    ];
    let inline_code = Node::InlineCode(InlineCode {
        value: String::new(),
        position: None,
        lang: None,
    });
    let mut context = context_with_tail(&events, b"x", with_position(inline_code));
    context.index = 1;
    context.buffer();
    let parent = JsxTag {
        name: Some("parent".into()),
        attributes: vec![],
        close: false,
        self_closing: false,
        start: Point::new(1, 1, 0),
        end: Point::new(1, 1, 0),
    };
    context.jsx_tag = Some(parent.clone());
    context.jsx_tag_stack.push(parent);

    let error = exit(&mut context).expect_err("the JSX boundary rejects the mismatched exit");
    assert_eq!(*error.rule_id, "end-tag-mismatch");
}

#[test]
fn reports_jsx_tag_mismatch_errors() {
    let error = crate::to_mdast("<a><b></a>", &ParseOptions::mdx())
        .expect_err("a closing tag cannot skip a nested open tag");
    assert_eq!(*error.rule_id, "end-tag-mismatch");

    let attribute_events = event_pair(Name::MdxJsxTagAttributeExpression, 0, 0);
    let mut context = context_with_tail(&attribute_events, b"", positioned_paragraph());
    context.jsx_tag = Some(JsxTag {
        name: Some("tag".into()),
        attributes: vec![],
        close: true,
        self_closing: false,
        start: Point::new(1, 1, 0),
        end: Point::new(1, 1, 0),
    });
    assert!(enter(&mut context).is_err());
}

#[test]
fn propagates_exit_errors_across_jsx_parent_boundaries() {
    let list_item = Node::ListItem(ListItem {
        children: vec![],
        position: None,
        spread: false,
        checked: None,
    });
    let code = Node::Code(Code {
        fence: None,
        value: String::new(),
        position: None,
        lang: None,
        meta: None,
    });
    let math = Node::Math(Math {
        value: String::new(),
        position: None,
        meta: None,
    });
    let inline_code = Node::InlineCode(InlineCode {
        value: String::new(),
        position: None,
        lang: None,
    });
    let inline_math = Node::InlineMath(InlineMath {
        value: String::new(),
        position: None,
    });
    let html = Node::Html(Html {
        value: String::new(),
        position: None,
    });
    let yaml = Node::Frontmatter(Frontmatter {
        info: String::new(),
        fence: '-',
        fence_length: 3,
        closing_fence_length: 3,
        value: String::new(),
        position: None,
    });

    let cases = vec![
        (Name::Data, positioned_text(), false, false),
        (Name::AutolinkProtocol, positioned_text(), false, false),
        (Name::AutolinkEmail, positioned_text(), false, false),
        (Name::CodeFenced, with_position(code.clone()), true, false),
        (Name::CodeIndented, with_position(code), true, false),
        (Name::CodeText, with_position(inline_code), true, false),
        (Name::MathFlow, with_position(math), true, false),
        (Name::MathText, with_position(inline_math), true, false),
        (Name::Frontmatter, with_position(yaml), true, false),
        (Name::GfmAutolinkLiteralWww, positioned_text(), false, false),
        (Name::GfmFootnoteCall, positioned_paragraph(), false, true),
        (Name::Image, positioned_paragraph(), false, true),
        (Name::Link, positioned_paragraph(), false, true),
        (Name::GfmTable, positioned_paragraph(), false, false),
        (Name::HardBreakEscape, positioned_paragraph(), false, false),
        (Name::HeadingSetext, positioned_paragraph(), false, false),
        (Name::HtmlFlow, with_position(html.clone()), true, false),
        (Name::HtmlText, with_position(html), true, false),
        (Name::ListItem, with_position(list_item), false, false),
        (Name::MdxEsm, positioned_paragraph(), true, false),
        (Name::MdxFlowExpression, positioned_paragraph(), true, false),
        (Name::MdxTextExpression, positioned_paragraph(), true, false),
    ];

    for (name, tail, buffer, media_reference) in cases {
        let events = [
            Event {
                kind: Kind::Enter,
                name: Name::MdxJsxFlowTag,
                point: point(0),
                link: None,
            },
            Event {
                kind: Kind::Enter,
                name: name.clone(),
                point: point(0),
                link: None,
            },
            Event {
                kind: Kind::Exit,
                name: name.clone(),
                point: point(1),
                link: None,
            },
        ];
        let mut context = context_with_tail(&events, b"x", tail);
        let parent = JsxTag {
            name: Some("parent".into()),
            attributes: vec![],
            close: false,
            self_closing: false,
            start: Point::new(1, 1, 0),
            end: Point::new(1, 1, 0),
        };
        context.jsx_tag = Some(parent.clone());
        context.jsx_tag_stack.push(parent);

        if buffer {
            context.buffer();
        }
        if media_reference {
            context.media_reference_stack.push(Reference::new());
        }

        let error = exit(&mut context).expect_err("the JSX parent event is mismatched");
        assert_eq!(*error.rule_id, "end-tag-mismatch", "{name:?}");
    }

    let events = [
        Event {
            kind: Kind::Enter,
            name: Name::MdxJsxFlowTag,
            point: point(0),
            link: None,
        },
        Event {
            kind: Kind::Enter,
            name: Name::LineEnding,
            point: point(0),
            link: None,
        },
        Event {
            kind: Kind::Enter,
            name: Name::MdxJsxTextTag,
            point: point(0),
            link: None,
        },
        Event {
            kind: Kind::Exit,
            name: Name::LineEnding,
            point: point(1),
            link: None,
        },
    ];
    let mut context = context_with_tail(&events, b"x", positioned_paragraph());
    let parent = JsxTag {
        name: Some("parent".into()),
        attributes: vec![],
        close: false,
        self_closing: false,
        start: Point::new(1, 1, 0),
        end: Point::new(1, 1, 0),
    };
    context.jsx_tag = Some(parent.clone());
    context.jsx_tag_stack.push(parent);
    let error = exit(&mut context).expect_err("the line ending crosses a JSX boundary");
    assert_eq!(*error.rule_id, "end-tag-mismatch");
}

#[test]
fn preserves_preceding_content_when_inline_code_info_has_no_text_prefix_node() {
    let bytes = b"rust:`code`";
    let events = event_pair(Name::CodeText, 5, 9);
    let paragraph = Node::Paragraph(Paragraph {
        children: vec![Node::Text(Text {
            value: "unrelated".into(),
            position: Some(Position::new(1, 1, 0, 1, 10, 9)),
        })],
        position: Some(Position::new(1, 1, 0, 1, 10, 9)),
    });
    let mut context = context_with_tail(&events, bytes, paragraph);
    context.inline_code_info = true;
    context.index = 0;

    on_enter_code_text(&mut context);

    let root_children = context.trees[0].0.children().unwrap();
    let paragraph_children = root_children[0].children().unwrap();
    assert!(matches!(
        paragraph_children.as_slice(),
        [
            Node::Text(Text { value, .. }),
            Node::InlineCode(InlineCode { lang: None, .. })
        ] if value == "unrelated"
    ));
}

#[test]
fn handles_inline_code_info_without_a_preceding_text_node() {
    let bytes = b"rust:`code`";
    let events = event_pair(Name::CodeText, 5, 9);
    let mut context = context_with_tail(&events, bytes, positioned_paragraph());
    context.inline_code_info = true;
    context.index = 0;

    on_enter_code_text(&mut context);

    let root_children = context.trees[0].0.children().unwrap();
    let paragraph_children = root_children[0].children().unwrap();
    assert!(matches!(
        paragraph_children.as_slice(),
        [Node::InlineCode(InlineCode { lang: None, .. })]
    ));
}

#[test]
fn handles_checked_items_without_a_text_prefix() {
    let events = event_pair(Name::ListItem, 0, 7);
    let text = Node::Text(Text {
        value: "content".into(),
        position: Some(Position::new(1, 1, 0, 1, 8, 7)),
    });
    let item = Node::ListItem(ListItem {
        children: vec![Node::Paragraph(Paragraph {
            children: vec![text],
            position: Some(Position::new(1, 1, 0, 1, 8, 7)),
        })],
        position: Some(Position::new(1, 1, 0, 1, 8, 7)),
        spread: false,
        checked: Some(true),
    });
    let mut context = context_with_tail(&events, b"content", item);
    on_exit_list_item(&mut context).expect("exit checked list item");

    let root_children = context.trees[0].0.children().unwrap();
    let item_children = root_children[0].children().unwrap();
    let paragraph_children = item_children[0].children().unwrap();
    assert!(matches!(
        paragraph_children.as_slice(),
        [Node::Text(Text { value, .. })] if value == "content"
    ));
}

#[test]
fn skips_checkbox_prefix_adjustment_for_nontext_paragraph_content() {
    let events = event_pair(Name::ListItem, 0, 5);
    let item = Node::ListItem(ListItem {
        children: vec![Node::Paragraph(Paragraph {
            children: vec![Node::InlineCode(InlineCode {
                value: "code".into(),
                position: Some(Position::new(1, 1, 0, 1, 5, 4)),
                lang: None,
            })],
            position: Some(Position::new(1, 1, 0, 1, 5, 4)),
        })],
        position: Some(Position::new(1, 1, 0, 1, 5, 4)),
        spread: false,
        checked: Some(true),
    });
    let mut context = context_with_tail(&events, b"code!", item);
    on_exit_list_item(&mut context).expect("exit checked list item");
}

#[test]
fn skips_checkbox_prefix_adjustment_for_nonparagraph_task_content() {
    let events = event_pair(Name::ListItem, 0, 1);
    let item = Node::ListItem(ListItem {
        children: vec![Node::Code(Code {
            fence: None,
            value: "code".into(),
            position: Some(Position::new(1, 1, 0, 1, 2, 1)),
            lang: None,
            meta: None,
        })],
        position: Some(Position::new(1, 1, 0, 1, 2, 1)),
        spread: false,
        checked: Some(true),
    });
    let mut context = context_with_tail(&events, b"x", item);
    on_exit_list_item(&mut context).expect("exit checked list item");
}

#[test]
fn skips_checkbox_prefix_adjustment_for_unexpected_tail_nodes() {
    let events = event_pair(Name::ListItem, 0, 0);
    let mut context = context_with_tail(&events, b"", positioned_paragraph());

    on_exit_list_item(&mut context).expect("exit list item event");

    assert_eq!(context.trees[0].0.children().unwrap().len(), 1);
}

#[test]
fn trims_crlf_and_cr_line_endings_from_literal_values() {
    assert_eq!(trim_eol("\r\nvalue\r\n".into(), true, true), "value");
    assert_eq!(trim_eol("\rvalue\r".into(), true, true), "value");
    assert_eq!(trim_eol("value\r".into(), true, true), "value");
}
