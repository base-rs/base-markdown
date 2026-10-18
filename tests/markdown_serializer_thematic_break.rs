// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

use core_markdown::markdown::{
    to_markdown as to, to_markdown_with_options as to_md_with_opts, Options,
};
use core_markdown::mdast::{Node, ThematicBreak};
use pretty_assertions::assert_eq;

#[test]
fn thematic_break() {
    assert_eq!(
        to(&Node::ThematicBreak(ThematicBreak { position: None })).unwrap(),
        "***\n",
        "should support a thematic break"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::ThematicBreak(ThematicBreak { position: None }),
            &Options {
                rule: '-',
                ..Default::default()
            }
        )
        .unwrap(),
        "---\n",
        "should support a thematic break w/ dashes when `rule: \"-\"`"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::ThematicBreak(ThematicBreak { position: None }),
            &Options {
                rule: '_',
                ..Default::default()
            }
        )
        .unwrap(),
        "___\n",
        "should support a thematic break w/ underscores when `rule: \"_\"`"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::ThematicBreak(ThematicBreak { position: None }),
            &Options {
                rule_repetition: 5,
                ..Default::default()
            }
        )
        .unwrap(),
        "*****\n",
        "should support a thematic break w/ more repetitions w/ `rule_repetition`"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::ThematicBreak(ThematicBreak { position: None }),
            &Options {
                rule_spaces: true,
                ..Default::default()
            }
        )
        .unwrap(),
        "* * *\n",
        "should support a thematic break w/ spaces w/ `rule_spaces`"
    );
}
