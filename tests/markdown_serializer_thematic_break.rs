// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

mod support;
use base_markdown::markdown::Options;
use base_markdown::{to_markdown_with_options, Document, ParseOptions};
use pretty_assertions::assert_eq;
use support::fixture::{to_markdown as to, to_markdown_with_options as to_md_with_opts};
use support::fixture::{Node, ThematicBreak};

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

#[test]
fn thematic_break_dash_falls_back_to_star() {
    let options = Options {
        rule: '-',
        ..Default::default()
    };
    for (source, expected, message) in [
        (
            "- a\n  ***\n",
            "- a\n  ***\n",
            "should write a rule after a paragraph line w/ `*`, not as a setext underline",
        ),
        (
            "1. a\n   b\n   ***\n   ***\n",
            "1. a\n   b\n   ***\n   ---\n",
            "should write only the rule after the paragraph line w/ `*`",
        ),
        (
            "- [a]: b\n  ***\n",
            "- [a]: b\n  ***\n",
            "should write a rule after a definition w/ `*`",
        ),
        (
            "- a\n\n  ***\n",
            "- a\n\n  ---\n",
            "should write a rule after a blank line w/ `-`",
        ),
        (
            "- # a\n  ***\n",
            "- # a\n  ---\n",
            "should write a rule after a heading w/ `-`",
        ),
        (
            "***\n\na\n\n***\n",
            "***\n\na\n\n---\n",
            "should write a first rule that would open frontmatter w/ `*`",
        ),
        (
            "***\n\n```\n---\n```\n",
            "***\n\n```\n---\n```\n",
            "should write a first rule closed as frontmatter by a line of code w/ `*`",
        ),
        (
            "***\n\na\n",
            "---\n\na\n",
            "should write a first rule that nothing closes w/ `-`",
        ),
        (
            "a\n\n***\n\n***\n",
            "a\n\n---\n\n---\n",
            "should write rules after the start of the document w/ `-`",
        ),
    ] {
        let mut document = Document::parse(source, &ParseOptions::default()).unwrap();
        document.normalize();
        assert_eq!(
            to_markdown_with_options(&document, &options).unwrap(),
            expected,
            "{}",
            message
        );
    }
}
