// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

mod support;
use base_markdown::{to_markdown, Document, ParseOptions};
use pretty_assertions::assert_eq;
use support::fixture::to_markdown as to;
use support::fixture::{Break, Html, Node, Paragraph, Text};

#[test]
fn html() {
    assert_eq!(
        to(&Node::Html(Html {
            value: String::new(),
            position: None
        }))
        .unwrap(),
        "",
        "should support an empty html"
    );

    assert_eq!(
        to(&Node::Html(Html {
            value: String::from("a\nb"),
            position: None
        }))
        .unwrap(),
        "a\nb\n",
        "should support html"
    );

    assert_eq!(
        to(&Node::Paragraph(Paragraph {
            children: vec![
                Node::Text(Text {
                    value: "a\n".to_string(),
                    position: None
                }),
                Node::Html(Html {
                    value: "<div>".to_string(),
                    position: None
                })
            ],
            position: None
        }))
        .unwrap(),
        "a <div>\n",
        "should prevent html (text) from becoming html (flow) (1)"
    );

    assert_eq!(
        to(&Node::Paragraph(Paragraph {
            children: vec![
                Node::Text(Text {
                    value: "a\r".to_string(),
                    position: None
                }),
                Node::Html(Html {
                    value: "<div>".to_string(),
                    position: None
                })
            ],
            position: None
        }))
        .unwrap(),
        "a <div>\n",
        "should prevent html (text) from becoming html (flow) (2)"
    );

    assert_eq!(
        to(&Node::Paragraph(Paragraph {
            children: vec![
                Node::Text(Text {
                    value: "a\r\n".to_string(),
                    position: None
                }),
                Node::Html(Html {
                    value: "<div>".to_string(),
                    position: None
                })
            ],
            position: None
        }))
        .unwrap(),
        "a <div>\n",
        "should prevent html (text) from becoming html (flow) (3)"
    );

    assert_eq!(
        to(&Node::Paragraph(Paragraph {
            children: vec![
                Node::Html(Html {
                    value: "<x>".to_string(),
                    position: None
                }),
                Node::Text(Text {
                    value: "a".to_string(),
                    position: None
                })
            ],
            position: None
        }))
        .unwrap(),
        "<x>a\n",
        "should serialize html (text)"
    );

    assert_eq!(
        to(&Node::Paragraph(Paragraph {
            children: vec![
                Node::Text(Text {
                    value: "a\n".to_string(),
                    position: None
                }),
                Node::Html(Html {
                    value: "<span>".to_string(),
                    position: None
                })
            ],
            position: None
        }))
        .unwrap(),
        "a\n<span>\n",
        "should keep the line ending before html (text) that cannot interrupt a paragraph"
    );

    assert_eq!(
        to(&Node::Paragraph(Paragraph {
            children: vec![
                Node::Text(Text {
                    value: "a".to_string(),
                    position: None
                }),
                Node::Break(Break { position: None }),
                Node::Text(Text {
                    value: String::new(),
                    position: None
                }),
                Node::Html(Html {
                    value: "<!-- m -->".to_string(),
                    position: None
                })
            ],
            position: None
        }))
        .unwrap(),
        "a\\\n    <!-- m -->\n",
        "should indent html (text) after a hard break so the break is kept"
    );
}

#[test]
fn html_text_after_phrasing_round_trips_unchanged() {
    for source in [
        "a <!-- m -->\n",
        "- `c` <!-- m -->\n",
        "plain <b>x</b>\n",
        "a<span>b</span>\n",
        "a\n<span>\n",
        "a\n<span\nx>\n",
        "a\\\n</span>\n",
        "a\\\n    <!-- m -->\n",
        "*a*\\\n    <?x?> b\n",
        "> a\\\n>     <div>\n",
        "- a\\\n      <![CDATA[x]]>\n",
    ] {
        let document = Document::parse(source, &ParseOptions::default()).unwrap();
        assert_eq!(to_markdown(&document).unwrap(), source, "{source:?}");
    }
}

#[test]
fn trailing_space_hard_break_before_html_text_is_kept() {
    let document = Document::parse("a  \n</span>\n", &ParseOptions::default()).unwrap();
    assert_eq!(to_markdown(&document).unwrap(), "a\\\n</span>\n");
}
