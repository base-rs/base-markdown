// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

use core_markdown::markdown::{
    to_markdown as to, to_markdown_with_options as to_md_with_opts, Options,
};
use core_markdown::mdast::{Node, Strong, Text};
use pretty_assertions::assert_eq;

#[test]
fn strong() {
    assert_eq!(
        to(&Node::Strong(Strong {
            children: Vec::new(),
            position: None
        }))
        .unwrap(),
        "****\n",
        "should support an empty strong"
    );

    assert_eq!(
        to(&Node::Strong(Strong {
            children: vec![Node::Text(Text {
                value: String::from("a"),
                position: None,
            })],
            position: None
        }))
        .unwrap(),
        "**a**\n",
        "should support a strong w/ children"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::Strong(Strong {
                children: vec![Node::Text(Text {
                    value: String::from("a"),
                    position: None,
                })],
                position: None
            }),
            &Options {
                strong: '_',
                ..Default::default()
            }
        )
        .unwrap(),
        "__a__\n",
        "should support a strong w/ underscores when `emphasis: \"_\"`"
    );
}
