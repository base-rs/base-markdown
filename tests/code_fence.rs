// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use core_markdown::stream::EventKind;
use core_markdown::{CodeFence, Constructs, Document, NodeId, NodeSyntax, ParseOptions};
use pretty_assertions::assert_eq;

fn code(source: &str, options: &ParseOptions) -> EventKind<'static> {
    let document = Document::parse(source, options).unwrap();
    document
        .nodes()
        .iter()
        .find(|node| matches!(node.kind, EventKind::CodeBlock { math: false, .. }))
        .expect("code block")
        .kind
        .clone()
}
fn fence(kind: &EventKind<'_>) -> Option<CodeFence> {
    match kind {
        EventKind::CodeBlock { fence, .. } => *fence,
        _ => None,
    }
}
fn document_fence<S: AsRef<str>>(document: &Document<S>, id: NodeId) -> Option<CodeFence> {
    fence(&document.node(id).unwrap().kind).or_else(|| match document.syntax(id) {
        Some(NodeSyntax::Frontmatter { fence, .. }) => Some(fence),
        _ => None,
    })
}

#[test]
fn opening_fences_match_markly_character_length_and_indent() {
    for (source, character, length, indent) in [
        ("```ruby\nputs 'wow'\n```", '`', 3, 0),
        ("  ~~~~ruby\n  puts 'wow'\n  ~~~~", '~', 4, 2),
        (" ```\ncode\n`````", '`', 3, 1),
        ("   ~~~~~\ncode\n~~~~~", '~', 5, 3),
        ("````rust", '`', 4, 0),
        ("~~~ \t", '~', 3, 0),
        ("```\r\ncode\r\n```", '`', 3, 0),
        ("~~~\rcode\r~~~", '~', 3, 0),
        ("before\n\n  ```\ncode\n```", '`', 3, 2),
        ("\u{feff}```\ncode\n```", '`', 3, 0),
    ] {
        let node = code(source, &ParseOptions::default());
        let expected = CodeFence {
            character,
            length,
            indent,
        };
        assert_eq!(fence(&node), Some(expected), "{source:?}");
        assert!(matches!(node, EventKind::CodeBlock {fence:Some(actual),..} if actual==expected));
    }
    let opening = "~".repeat(300);
    let source = format!("{opening}\ncode\n{opening}");
    assert_eq!(
        fence(&code(&source, &ParseOptions::default()))
            .unwrap()
            .length,
        300
    );
}

#[test]
fn indentation_excludes_container_prefixes_and_expands_tabs() {
    for (source, indent) in [
        ("> ```\n> code\n> ```", 0),
        (">  ```\n> code\n> ```", 1),
        (">   ```\n> code\n> ```", 2),
        ("> \t~~~\n> code\n> ~~~", 2),
        (">\t~~~\n> code\n> ~~~", 2),
        ("- ```\n  code\n  ```", 0),
        ("-   ```\n    code\n    ```", 0),
        ("- item\n\n    ```\n    code\n    ```", 2),
        ("> -   ~~~\n>     code\n>     ~~~", 0),
    ] {
        assert_eq!(
            fence(&code(source, &ParseOptions::default()))
                .unwrap()
                .indent,
            indent,
            "{source:?}"
        );
    }

    let options = ParseOptions {
        constructs: Constructs {
            code_indented: false,
            ..Constructs::default()
        },
        ..ParseOptions::default()
    };
    for (source, indent) in [
        ("\t```\n```", 4),
        (" \t~~~\n~~~", 4),
        ("\t\t```\n```", 8),
        ("    ~~~\n~~~", 4),
    ] {
        assert_eq!(
            fence(&code(source, &options)).unwrap().indent,
            indent,
            "{source:?}"
        );
    }
}

#[test]
fn unfenced_nodes_have_no_code_fence() {
    let node = code("    puts 'wow'", &ParseOptions::default());
    assert_eq!(fence(&node), None);
    let document = Document::parse("# Heading\n\n`code`", &ParseOptions::default()).unwrap();
    for id in document.descendants(document.root()) {
        assert_eq!(document_fence(&document, id), None);
    }
    let kind = EventKind::CodeBlock {
        value: "created without source".to_string().into(),
        language: None,
        fence: None,
        math: false,
        unclosed: false,
    };
    assert_eq!(fence(&kind), None);
}

#[test]
fn frontmatter_exposes_its_existing_opening_fence() {
    let options = ParseOptions {
        constructs: Constructs {
            frontmatter: true,
            ..Constructs::default()
        },
        ..ParseOptions::default()
    };
    for (source, character, length) in [
        ("---\nyaml: body\n---", '-', 3),
        ("+++\ntoml = 1\n+++", '+', 3),
        ("--- json\n{}\n---", '-', 3),
        ("+++ custom\nraw\n+++", '+', 3),
        ("````json\n{}\n`````", '`', 4),
        ("~~~~~json\n{}\n~~~~~", '~', 5),
    ] {
        let document = Document::parse(source, &options).unwrap();
        assert_eq!(
            document_fence(
                &document,
                document.children(document.root()).next().unwrap()
            ),
            Some(CodeFence {
                character,
                length,
                indent: 0
            })
        );
    }
}

#[test]
fn detached_and_edited_nodes_keep_fence_metadata() {
    let source = "  ~~~~ruby\n  puts 'wow'\n  ~~~~";
    let mut document = Document::parse(source, &ParseOptions::default()).unwrap();
    let children = document.extract_children(document.root()).unwrap();
    let node = children[0];
    let expected = Some(CodeFence {
        character: '~',
        length: 4,
        indent: 2,
    });
    assert_eq!(document_fence(&document, node), expected);
    let EventKind::CodeBlock { fence, .. } = document.node(node).unwrap().kind else {
        panic!("code block");
    };
    document
        .set_kind(
            node,
            EventKind::CodeBlock {
                value: "edited\n".to_string().into(),
                language: Some("python".to_string().into()),
                fence,
                math: false,
                unclosed: false,
            },
        )
        .unwrap();
    document.set_syntax(node, None).unwrap();
    assert_eq!(document_fence(&document, node), expected);
    assert_eq!(
        core_markdown::to_markdown_node(&document, node, &Default::default()).unwrap(),
        "```python\nedited\n```\n"
    );
}

#[cfg(feature = "serde")]
#[test]
fn serde_preserves_fence_metadata() {
    let document = Document::parse(
        "  ~~~~ruby\n  puts 'wow'\n  ~~~~".to_string(),
        &ParseOptions::default(),
    )
    .unwrap();
    let json = serde_json::to_string(&document).unwrap();
    let restored: Document = serde_json::from_str(&json).unwrap();
    let node = restored.children(restored.root()).next().unwrap();
    assert_eq!(
        document_fence(&restored, node),
        Some(CodeFence {
            character: '~',
            length: 4,
            indent: 2
        })
    );
}
