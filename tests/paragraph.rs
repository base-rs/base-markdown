// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use core_markdown::{to_html_with_options, Document, Options, ParseOptions};
use pretty_assertions::assert_eq;

#[test]
fn paragraphs_preserve_container_chunks_and_source_positions() {
    let source = "> a\tb\r\n> *c*\r\n\r\ntext\r\nnext";
    assert_eq!(
        to_html_with_options(source, &Options::default()),
        "<blockquote>\n<p>a\tb\n<em>c</em></p>\n</blockquote>\n<p>text\nnext</p>"
            .replace('\n', "\r\n")
    );
    let tree = Document::parse(source, &ParseOptions::default()).unwrap();
    let children: Vec<_> = tree
        .children(tree.root())
        .filter(|id| {
            !matches!(
                tree.node(*id).unwrap().kind,
                core_markdown::stream::EventKind::Separator { .. }
            )
        })
        .collect();
    let paragraph = tree.children(children[0]).next().unwrap();
    assert!(matches!(
        tree.node(paragraph).unwrap().kind,
        core_markdown::stream::EventKind::Start(core_markdown::stream::Tag::Paragraph { .. })
    ));
    assert_eq!(tree.node(paragraph).unwrap().span, 2..14);
    let emphasis = tree
        .children(paragraph)
        .find(|id| {
            matches!(
                tree.node(*id).unwrap().kind,
                core_markdown::stream::EventKind::Start(core_markdown::stream::Tag::Emphasis)
            )
        })
        .unwrap();
    assert_eq!(tree.node(emphasis).unwrap().span, 9..12);
    assert_eq!(tree.node(children[1]).unwrap().span, 16..26);
    assert_eq!(tree.text_content(paragraph), "a\tb\r\nc");
}

#[test]
fn paragraphs_and_definitions_share_forward_references() {
    let source = "first [ref]\ncontinued\n\n   [ref]:\n     /url\n     'title'\n\nafter [ref]\n\n[other]: /other\nlast [other]";
    assert_eq!(
        to_html_with_options(source, &Options::default()),
        "<p>first <a href=\"/url\" title=\"title\">ref</a>\ncontinued</p>\n<p>after <a href=\"/url\" title=\"title\">ref</a></p>\n<p>last <a href=\"/other\">other</a></p>"
    );
    let mut options = Options::default();
    options.parse.constructs.definition = false;
    assert_eq!(
        to_html_with_options("[ref]: /url\nnext\n\ntext [ref]", &options),
        "<p>[ref]: /url\nnext</p>\n<p>text [ref]</p>"
    );
    assert_eq!(
        to_html_with_options("- [x] first\n  continued\n\n  second\n\n- [ ] next", &Options::gfm()),
        "<ul>\n<li>\n<p><input type=\"checkbox\" disabled=\"\" checked=\"\" /> first\ncontinued</p>\n<p>second</p>\n</li>\n<li>\n<p><input type=\"checkbox\" disabled=\"\" /> next</p>\n</li>\n</ul>"
    );
}
