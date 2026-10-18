// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use core_markdown::{
    mdast::{CodeFence, Node},
    to_mdast, Constructs, ParseOptions,
};
use pretty_assertions::assert_eq;

fn code(source: &str, options: &ParseOptions) -> Node {
    let document = to_mdast(source, options).unwrap();
    let mut result = None;
    document.walk(|node| {
        if matches!(node, Node::Code(_)) {
            result = Some(node.clone());
        }
    });
    result.expect("a code block")
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
        assert_eq!(node.code_fence(), Some(expected), "{source:?}");
        let Node::Code(node) = node else {
            unreachable!()
        };
        assert_eq!(node.fence, Some(expected));
    }
    let opening = "~".repeat(300);
    let source = format!("{opening}\ncode\n{opening}");
    assert_eq!(
        code(&source, &ParseOptions::default())
            .code_fence()
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
            code(source, &ParseOptions::default())
                .code_fence()
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
            code(source, &options).code_fence().unwrap().indent,
            indent,
            "{source:?}"
        );
    }
}

#[test]
fn unfenced_nodes_have_no_code_fence() {
    let node = code("    puts 'wow'", &ParseOptions::default());
    assert_eq!(node.code_fence(), None);
    let document = to_mdast("# Heading\n\n`code`", &ParseOptions::default()).unwrap();
    document.walk(|node| assert_eq!(node.code_fence(), None));
    let node = Node::Code(core_markdown::mdast::Code {
        value: "created without source".into(),
        position: None,
        lang: None,
        meta: None,
        fence: None,
    });
    assert_eq!(node.code_fence(), None);
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
        let document = to_mdast(source, &options).unwrap();
        assert_eq!(
            document.children().unwrap()[0].code_fence(),
            Some(CodeFence {
                character,
                length,
                indent: 0
            })
        );
    }
}

#[test]
fn cloned_and_detached_nodes_keep_metadata_without_positions() {
    let source = "  ~~~~ruby\n  puts 'wow'\n  ~~~~";
    let mut document = to_mdast(source, &ParseOptions::default()).unwrap();
    let mut detached = document.extract_children().unwrap();
    detached.walk_mut(|node| node.position_set(None));
    let mut node = detached.children().unwrap()[0].clone();
    let expected = Some(CodeFence {
        character: '~',
        length: 4,
        indent: 2,
    });
    assert_eq!(node.code_fence(), expected);
    let Node::Code(ref mut code) = node else {
        unreachable!()
    };
    code.value = "edited".into();
    code.lang = Some("python".into());
    assert_eq!(node.code_fence(), expected);
    assert_eq!(node.to_markdown(), "```python\nedited\n```\n");
}

#[cfg(feature = "serde")]
#[test]
fn serde_preserves_metadata_and_accepts_older_code_nodes() {
    let node = code("  ~~~~ruby\n  puts 'wow'\n  ~~~~", &ParseOptions::default());
    let json = serde_json::to_value(&node).unwrap();
    assert_eq!(
        json["fence"],
        serde_json::json!({"character":"~", "length":4, "indent":2})
    );
    assert_eq!(serde_json::from_value::<Node>(json).unwrap(), node);
    let old = serde_json::from_str::<Node>(r#"{"type":"code","value":"legacy"}"#).unwrap();
    assert_eq!(old.code_fence(), None);
    assert!(serde_json::to_value(&old).unwrap().get("fence").is_none());
}
