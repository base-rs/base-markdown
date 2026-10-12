// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

//! JS equivalent https://github.com/syntax-tree/mdast-util-to-markdown/blob/main/lib/util/format-heading-as-setext.js
use crate::mdast::{Heading, Node};
use alloc::string::{String, ToString};
use regex::Regex;

use crate::markdown::state::State;

pub fn format_heading_as_setext(heading: &Heading, state: &State) -> bool {
    let line_break = Regex::new(r"\r?\n|\r").unwrap();
    let mut literal_with_line_break = false;

    for child in &heading.children {
        if include_literal_with_line_break(child, &line_break) {
            literal_with_line_break = true;
            break;
        }
    }

    heading.depth < 3
        && !to_string(&heading.children).is_empty()
        && (state.options.setext || literal_with_line_break)
}

/// See: <https://github.com/syntax-tree/mdast-util-to-markdown/blob/main/lib/util/format-heading-as-setext.js>.
fn include_literal_with_line_break(node: &Node, regex: &Regex) -> bool {
    match node {
        Node::Break(_) => true,
        // Literals.
        Node::Code(x) => regex.is_match(&x.value),
        Node::Html(x) => regex.is_match(&x.value),
        Node::InlineCode(x) => regex.is_match(&x.value),
        Node::InlineMath(x) => regex.is_match(&x.value),
        Node::Math(x) => regex.is_match(&x.value),
        Node::MdxFlowExpression(x) => regex.is_match(&x.value),
        Node::MdxTextExpression(x) => regex.is_match(&x.value),
        Node::MdxjsEsm(x) => regex.is_match(&x.value),
        Node::Text(x) => regex.is_match(&x.value),
        Node::Frontmatter(x) => regex.is_match(&x.value),
        // Anything else.
        _ => {
            if let Some(children) = node.children() {
                for child in children {
                    if include_literal_with_line_break(child, regex) {
                        return true;
                    }
                }
            }

            false
        }
    }
}

/// Tiny version of `mdast-util-to-string`.
fn to_string(children: &[Node]) -> String {
    children.iter().map(ToString::to_string).collect()
}

#[cfg(test)]
mod tests {
    use super::format_heading_as_setext;
    use crate::{
        markdown::{state::State, Options},
        mdast::{
            Break, Code, Frontmatter, Heading, Html, Image, InlineCode, InlineMath, Math,
            MdxFlowExpression, MdxTextExpression, MdxjsEsm, Node, Text,
        },
    };
    use alloc::string::String;
    use alloc::vec;

    fn heading(depth: u8, child: Node) -> Heading {
        Heading {
            children: vec![child],
            position: None,
            depth,
        }
    }

    fn literal(value: &str) -> String {
        value.into()
    }

    #[test]
    fn selects_setext_for_options_and_literal_line_breaks() {
        let options = Options::default();
        let state = State::new(&options);

        let plain_heading = heading(
            1,
            Node::Text(Text {
                value: literal("plain"),
                position: None,
            }),
        );
        assert!(!format_heading_as_setext(&plain_heading, &state));

        for child in [
            Node::Code(Code {
                fence: None,
                value: literal("a\nb"),
                position: None,
                lang: None,
                meta: None,
            }),
            Node::Html(Html {
                value: literal("a\nb"),
                position: None,
            }),
            Node::InlineCode(InlineCode {
                value: literal("a\nb"),
                position: None,
                lang: None,
            }),
            Node::InlineMath(InlineMath {
                value: literal("a\nb"),
                position: None,
            }),
            Node::Math(Math {
                value: literal("a\nb"),
                position: None,
                meta: None,
            }),
            Node::MdxFlowExpression(MdxFlowExpression {
                value: literal("a\nb"),
                position: None,
                stops: vec![],
            }),
            Node::MdxTextExpression(MdxTextExpression {
                value: literal("a\nb"),
                position: None,
                stops: vec![],
            }),
            Node::MdxjsEsm(MdxjsEsm {
                value: literal("a\nb"),
                position: None,
                stops: vec![],
            }),
            Node::Text(Text {
                value: literal("a\nb"),
                position: None,
            }),
            Node::Frontmatter(Frontmatter {
                value: literal("a\nb"),
                info: "custom".into(),
                fence: '~',
                fence_length: 3,
                closing_fence_length: 3,
                position: None,
            }),
        ] {
            assert!(
                format_heading_as_setext(&heading(1, child.clone()), &state),
                "{:?}",
                child
            );
        }

        let break_with_text = Heading {
            children: vec![
                Node::Break(Break { position: None }),
                Node::Text(Text {
                    value: literal("after"),
                    position: None,
                }),
            ],
            position: None,
            depth: 1,
        };
        assert!(format_heading_as_setext(&break_with_text, &state));

        let nested = Node::Emphasis(crate::mdast::Emphasis {
            children: vec![Node::Text(Text {
                value: literal("a\nb"),
                position: None,
            })],
            position: None,
        });
        assert!(format_heading_as_setext(&heading(2, nested), &state));
        let nested_without_line_break = Node::Emphasis(crate::mdast::Emphasis {
            children: vec![Node::Text(Text {
                value: literal("plain"),
                position: None,
            })],
            position: None,
        });
        assert!(!format_heading_as_setext(
            &heading(2, nested_without_line_break),
            &state
        ));

        let image_without_children = Node::Image(Image {
            alt: literal("diagram"),
            position: None,
            title: None,
            url: "diagram.png".into(),
        });
        assert!(!format_heading_as_setext(
            &heading(2, image_without_children),
            &state
        ));

        let options = Options {
            setext: true,
            ..Options::default()
        };
        let state = State::new(&options);
        assert!(format_heading_as_setext(&plain_heading, &state));
        assert!(!format_heading_as_setext(
            &Heading {
                children: vec![],
                position: None,
                depth: 1,
            },
            &state
        ));
        assert!(!format_heading_as_setext(
            &heading(3, plain_heading.children[0].clone()),
            &state
        ));
    }
}
