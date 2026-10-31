// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use base_markdown::stream::{EventKind, Tag, Text};
use base_markdown::{Document, LinkForm, NodeId, NodeSyntax, ParseOptions};
use pretty_assertions::assert_eq;

fn parse(source: &str) -> Document<String> {
    Document::parse(source.to_owned(), &ParseOptions::gfm()).unwrap()
}

fn links(document: &Document<String>) -> Vec<NodeId> {
    document
        .descendants(document.root())
        .filter(|&id| {
            matches!(
                document.node(id).unwrap().kind,
                EventKind::Start(Tag::Link { .. })
            )
        })
        .collect()
}

fn paragraphs(document: &Document<String>) -> Vec<NodeId> {
    document
        .children(document.root())
        .filter(|&id| {
            matches!(
                document.node(id).unwrap().kind,
                EventKind::Start(Tag::Paragraph { .. })
            )
        })
        .collect()
}

fn markdown(document: &Document<String>) -> String {
    base_markdown::to_markdown(document).unwrap()
}

/// Serializing an edited tree gives `expected`, which serializes back to itself.
fn assert_serializes(document: &Document<String>, expected: &str) {
    let markdown = markdown(document);
    assert_eq!(markdown, expected);
    assert_eq!(self::markdown(&parse(&markdown)), markdown);
}

fn explicit(document: &mut Document<String>, link: NodeId) {
    document
        .set_syntax(
            link,
            Some(NodeSyntax::Link {
                form: LinkForm::Explicit,
            }),
        )
        .unwrap();
}

#[test]
fn moved_literal_autolinks_keep_their_form() {
    let mut document = parse("www.a.b www.c.d\n");
    let paragraph = paragraphs(&document)[0];
    let first = document.children(paragraph).next().unwrap();
    let second = links(&document)[1];
    document.attach(paragraph, second, Some(first)).unwrap();
    for link in links(&document) {
        assert_eq!(
            document.syntax(link),
            Some(NodeSyntax::Link {
                form: LinkForm::Bare
            })
        );
    }
    // The stored form is trusted, so the moved links are written as their text.
    assert_eq!(markdown(&document), "www.c.dwww.a.b&#x20;\n");
    // An editor that wants them kept apart chooses the explicit form.
    for link in links(&document) {
        explicit(&mut document, link);
    }
    assert_serializes(
        &document,
        "[www.c.d](http://www.c.d)[www.a.b](http://www.a.b)&#x20;\n",
    );
}

#[test]
fn literal_autolinks_moved_between_paragraphs_keep_their_form() {
    for (source, from, to, expected) in [
        ("www.a.b x\n\nc d\n", 0, 1, "&#x20;x\n\nc dwww.a.b\n"),
        ("c d\n\nwww.a.b x\n", 1, 0, "c dwww.a.b\n\n&#x20;x\n"),
    ] {
        let mut document = parse(source);
        let paragraphs = paragraphs(&document);
        let link = links(&document)[0];
        assert_eq!(
            document.node(link).unwrap().parent(),
            Some(paragraphs[from])
        );
        document.attach(paragraphs[to], link, None).unwrap();
        assert_eq!(markdown(&document), expected);
    }
}

#[test]
fn literal_autolinks_after_edited_neighbours_keep_their_form() {
    let mut document = parse("\\*a\\*www.a.b\n");
    let paragraph = paragraphs(&document)[0];
    let link = links(&document)[0];
    let texts: Vec<_> = document
        .children(paragraph)
        .filter(|&child| child != link)
        .collect();
    for text in texts {
        document.detach(text).unwrap();
    }
    let emphasis = document
        .append_child(paragraph, EventKind::Start(Tag::Emphasis))
        .unwrap();
    document
        .append_child(emphasis, EventKind::Text(Text::from("a".to_owned())))
        .unwrap();
    document.attach(paragraph, emphasis, Some(link)).unwrap();
    assert_serializes(&document, "*a*www.a.b\n");
}

#[test]
fn copied_links_keep_their_form() {
    let replacement = parse("www.a.b <https://c.d> [e](f)\n");
    let mut document = parse("# x\n\ny\n");
    let root = document.root();
    assert!(document
        .replace_heading_section(root, 0, &replacement, false, false)
        .unwrap());
    let forms: Vec<_> = links(&document)
        .into_iter()
        .map(|link| document.syntax(link))
        .collect();
    assert_eq!(
        forms,
        [LinkForm::Bare, LinkForm::Angle, LinkForm::Explicit]
            .map(|form| Some(NodeSyntax::Link { form }))
    );
    assert_serializes(&document, "# x\n\nwww.a.b <https://c.d> [e](f)\n");
}

#[test]
fn constructed_links_are_explicit() {
    for (destination, text, expected) in [
        ("", None, "www.a.b\n\n[]()\n"),
        ("", Some(""), "www.a.b\n\n[]()\n"),
        ("http://", None, "www.a.b\n\n[](http://)\n"),
        ("http://", Some(""), "www.a.b\n\n[](http://)\n"),
        (
            "http://www.c.d",
            Some("www.c.d"),
            "www.a.b\n\n[www.c.d](http://www.c.d)\n",
        ),
        (
            "https://c.d",
            Some("https://c.d"),
            "www.a.b\n\n[https://c.d](https://c.d)\n",
        ),
    ] {
        let mut document = parse("www.a.b\n");
        let root = document.root();
        let paragraph = document
            .append_child(root, EventKind::Start(Tag::Paragraph { tight: false }))
            .unwrap();
        let link = document
            .append_child(
                paragraph,
                EventKind::Start(Tag::Link {
                    destination: Text::from(destination.to_owned()),
                    title: None,
                }),
            )
            .unwrap();
        if let Some(text) = text {
            document
                .append_child(link, EventKind::Text(Text::from(text.to_owned())))
                .unwrap();
        }
        assert!(document.syntax(link).is_none());
        assert_serializes(&document, expected);
    }
}

#[test]
fn bare_and_angle_forms_fall_back_when_they_cannot_hold_the_link() {
    let mut document = parse("www.a.b\n");
    let link = links(&document)[0];
    let child = document.children(link).next().unwrap();
    // A title, or text that is not a single text node, needs the explicit form.
    document
        .set_kind(
            link,
            EventKind::Start(Tag::Link {
                destination: Text::from("http://www.a.b".to_owned()),
                title: Some(Text::from("t".to_owned())),
            }),
        )
        .unwrap();
    for form in [LinkForm::Bare, LinkForm::Angle] {
        document
            .set_syntax(link, Some(NodeSyntax::Link { form }))
            .unwrap();
        assert_serializes(&document, "[www.a.b](http://www.a.b \"t\")\n");
    }
    document.detach(child).unwrap();
    document
        .set_kind(
            link,
            EventKind::Start(Tag::Link {
                destination: Text::from("http://www.a.b".to_owned()),
                title: None,
            }),
        )
        .unwrap();
    for form in [LinkForm::Bare, LinkForm::Angle] {
        document
            .set_syntax(link, Some(NodeSyntax::Link { form }))
            .unwrap();
        assert_serializes(&document, "[](http://www.a.b)\n");
    }
    // Text that is the destination can be written in either form.
    document
        .append_child(
            link,
            EventKind::Text(Text::from("http://www.a.b".to_owned())),
        )
        .unwrap();
    for (form, expected) in [
        (LinkForm::Bare, "http://www.a.b\n"),
        (LinkForm::Angle, "<http://www.a.b>\n"),
        (LinkForm::Explicit, "[http://www.a.b](http://www.a.b)\n"),
    ] {
        document
            .set_syntax(link, Some(NodeSyntax::Link { form }))
            .unwrap();
        assert_serializes(&document, expected);
    }
}

#[test]
fn bare_and_angle_forms_need_a_single_text_child() {
    for source in ["[*a*](http://a.b)\n", "[a*b*](http://a.b)\n"] {
        let mut document = parse(source);
        let link = links(&document)[0];
        for form in [LinkForm::Bare, LinkForm::Angle] {
            document
                .set_syntax(link, Some(NodeSyntax::Link { form }))
                .unwrap();
            assert_serializes(&document, source);
        }
    }
}

#[test]
fn angle_brackets_before_bare_links_are_escaped_when_they_would_close_an_autolink() {
    // Parsed as `<`, a bare link and `>`; the text of the link is then replaced.
    for (text, expected) in [
        // `<1@a.b>` is an email autolink, so the `<` keeps its escape.
        ("1@a.b", "\\<1@a.b>\n"),
        // `<1>` and `<1@a_b.c>` are not autolinks, so the `<` is written as it is.
        ("1", "<1>\n"),
        ("1@a_b.c", "<1@a_b.c>\n"),
        // A `<` before a letter is already escaped, since it could start HTML.
        ("www.a.b", "\\<www.a.b>\n"),
    ] {
        let mut document = parse("\\<1@a.b>\n");
        let link = links(&document)[0];
        let child = document.children(link).next().unwrap();
        document
            .set_kind(child, EventKind::Text(Text::from(text.to_owned())))
            .unwrap();
        assert_eq!(markdown(&document), expected, "{text:?}");
    }
}

#[test]
fn bare_links_moved_into_table_cells_escape_pipes() {
    let mut document = parse("www.a.b/x|y\n\n| a |\n| - |\n| b |\n");
    let link = links(&document)[0];
    let cell = document
        .descendants(document.root())
        .filter(|&id| {
            matches!(
                document.node(id).unwrap().kind,
                EventKind::Start(Tag::TableCell { .. })
            )
        })
        .last()
        .unwrap();
    let text = document.children(cell).next().unwrap();
    document.detach(text).unwrap();
    document.attach(cell, link, None).unwrap();
    document.detach(paragraphs(&document)[0]).unwrap();
    assert_serializes(&document, "| a |\n| --- |\n| www.a.b/x\\|y |\n");
    // The text after the pipe stays in the cell.
    let html =
        base_markdown::to_html_with_options(&markdown(&document), &base_markdown::Options::gfm());
    assert!(
        html.contains("y</a></td>") || html.contains("|y</td>"),
        "{}",
        html
    );
}

#[test]
fn text_after_an_empty_text_still_follows_the_bare_link() {
    let mut document = parse("www.a\\_b.com\n");
    let paragraph = paragraphs(&document)[0];
    let link = links(&document)[0];
    let empty = document
        .append_child(paragraph, EventKind::Text(Text::from(String::new())))
        .unwrap();
    document.attach(paragraph, empty, Some(link)).unwrap();
    assert_serializes(&document, "www.a\\_b.com\n");
}
