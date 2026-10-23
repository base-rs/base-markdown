// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use core_markdown::{
    mdast::Node, to_html_with_options, to_mdast, unist::Position, Options, ParseOptions,
};
use pretty_assertions::assert_eq;

#[test]
fn paragraphs_preserve_container_chunks_and_source_positions() {
    let source = "> a\tb\r\n> *c*\r\n\r\ntext\r\nnext";
    assert_eq!(
        to_html_with_options(source, &Options::default()).unwrap(),
        "<blockquote>\n<p>a\tb\n<em>c</em></p>\n</blockquote>\n<p>text\nnext</p>"
            .replace('\n', "\r\n")
    );
    let tree = to_mdast(source, &ParseOptions::default()).unwrap();
    let children = tree.children().unwrap();
    let paragraph = &children[0].children().unwrap()[0];
    assert!(matches!(paragraph, Node::Paragraph(_)));
    assert_eq!(
        paragraph.position(),
        Some(&Position::new(1, 3, 2, 2, 6, 12))
    );
    let inline = paragraph.children().unwrap();
    assert_eq!(inline[0].position(), Some(&Position::new(1, 3, 2, 2, 1, 7)));
    assert_eq!(
        inline[1].position(),
        Some(&Position::new(2, 3, 9, 2, 6, 12))
    );
    assert_eq!(
        children[1].position(),
        Some(&Position::new(4, 1, 16, 5, 5, 26))
    );
}

#[test]
fn paragraphs_and_definitions_share_forward_references() {
    let source = "first [ref]\ncontinued\n\n   [ref]:\n     /url\n     'title'\n\nafter [ref]\n\n[other]: /other\nlast [other]";
    assert_eq!(
        to_html_with_options(source, &Options::default()).unwrap(),
        "<p>first <a href=\"/url\" title=\"title\">ref</a>\ncontinued</p>\n<p>after <a href=\"/url\" title=\"title\">ref</a></p>\n<p>last <a href=\"/other\">other</a></p>"
    );
    let mut options = Options::default();
    options.parse.constructs.definition = false;
    assert_eq!(
        to_html_with_options("[ref]: /url\nnext\n\ntext [ref]", &options).unwrap(),
        "<p>[ref]: /url\nnext</p>\n<p>text [ref]</p>"
    );
    assert_eq!(
        to_html_with_options("- [x] first\n  continued\n\n  second\n\n- [ ] next", &Options::gfm()).unwrap(),
        "<ul>\n<li>\n<p><input type=\"checkbox\" disabled=\"\" checked=\"\" /> first\ncontinued</p>\n<p>second</p>\n</li>\n<li>\n<p><input type=\"checkbox\" disabled=\"\" /> next</p>\n</li>\n</ul>"
    );
}
