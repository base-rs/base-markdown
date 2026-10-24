// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use super::*;
use crate::event::Kind;
use crate::ParseOptions;
use alloc::vec;

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
    let mut options = ParseOptions::default();
    options.constructs.frontmatter = true;
    options.constructs.gfm_autolink_literal = true;
    options.constructs.gfm_footnote_definition = true;
    options.constructs.gfm_label_start_footnote = true;
    options.constructs.gfm_strikethrough = true;
    options.constructs.gfm_table = true;
    options.constructs.gfm_task_list_item = true;
    options.constructs.math_flow = true;
    options.constructs.math_text = true;

    let source = "---\ntitle: Example\n---\n\
Introductory text.\n\n\
# Heading\n\
Setext\n---\n\
Ordinary paragraph.\n\
Text with ordinary words.\n\
Another paragraph.\n\
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
        .any(|node| matches!(node, Node::Heading(_))));
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
    on_exit_list_item(&mut context);

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
    on_exit_list_item(&mut context);
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
    on_exit_list_item(&mut context);
}

#[test]
fn skips_checkbox_prefix_adjustment_for_unexpected_tail_nodes() {
    let events = event_pair(Name::ListItem, 0, 0);
    let mut context = context_with_tail(&events, b"", positioned_paragraph());

    on_exit_list_item(&mut context);

    assert_eq!(context.trees[0].0.children().unwrap().len(), 1);
}

#[test]
fn trims_crlf_and_cr_line_endings_from_literal_values() {
    assert_eq!(trim_eol("\r\nvalue\r\n".into(), true, true), "value");
    assert_eq!(trim_eol("\rvalue\r".into(), true, true), "value");
    assert_eq!(trim_eol("value\r".into(), true, true), "value");
}
