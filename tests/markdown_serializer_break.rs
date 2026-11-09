// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

mod support;
use base_markdown::markdown::Options;
use base_markdown::stream::{EventKind, StreamError, Tag};
use base_markdown::Document;
use pretty_assertions::assert_eq;
use support::fixture::{to_markdown as to, to_markdown_with_options as to_md_with_opts};
use support::fixture::{Break, Node};

#[test]
fn r#break() {
    assert_eq!(
        to(&Node::Break(Break { position: None })).unwrap(),
        "\\\n",
        "should support a break"
    );

    // A heading of level 3 to 6 is ATX, which holds no break, so none can be built.
    let mut document = Document::parse(String::new(), &Default::default()).unwrap();
    let heading = document
        .append_child(document.root(), EventKind::Start(Tag::Heading(3)))
        .unwrap();
    assert!(
        matches!(
            document.append_child(heading, EventKind::HardBreak),
            Err(StreamError::InvalidNesting)
        ),
        "should reject breaks in heading (atx)"
    );

    assert_eq!(
        to_md_with_opts(
            &Document::parse("a  \nb\n=\n", &Default::default()).unwrap(),
            &Options::default()
        )
        .unwrap(),
        "a\\\nb\n=\n",
        "should serialize breaks in heading (setext)"
    );
}
