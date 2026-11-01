// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use base_markdown::stream::{EventKind, Tag, Text};
use base_markdown::{Document, LinkForm, NodeId, NodeSyntax, ParseOptions};
use pretty_assertions::assert_eq;

mod support;
use support::fixture::canonical;

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

/// Serializing an edited tree gives `expected`, which reads back as the same tree and serializes
/// back to itself.
fn assert_serializes(document: &Document<String>, expected: &str) {
    let markdown = markdown(document);
    assert_eq!(markdown, expected);
    let reparsed = parse(&markdown);
    assert_eq!(canonical(&reparsed), canonical(document), "{markdown:?}");
    assert_eq!(self::markdown(&reparsed), markdown);
}

fn bare() -> Option<NodeSyntax<'static>> {
    Some(NodeSyntax::Link {
        form: LinkForm::Bare,
    })
}

#[test]
fn moved_literal_autolinks_keep_their_form_but_fall_back_next_to_each_other() {
    let mut document = parse("www.a.b www.c.d\n");
    let paragraph = paragraphs(&document)[0];
    let first = document.children(paragraph).next().unwrap();
    let second = links(&document)[1];
    document.attach(paragraph, second, Some(first)).unwrap();
    for link in links(&document) {
        assert_eq!(document.syntax(link), bare());
    }
    // Neither link has whitespace on both sides, so neither is written bare.
    assert_serializes(
        &document,
        "[www.c.d](http://www.c.d)[www.a.b](http://www.a.b)&#x20;\n",
    );
}

#[test]
fn literal_autolinks_moved_between_paragraphs_fall_back_next_to_text() {
    for (source, from, to, expected) in [
        (
            "www.a.b x\n\nc d\n",
            0,
            1,
            "&#x20;x\n\nc d[www.a.b](http://www.a.b)\n",
        ),
        (
            "c d\n\nwww.a.b x\n",
            1,
            0,
            "c d[www.a.b](http://www.a.b)\n\n&#x20;x\n",
        ),
        (
            "c d\n\nhttps://a.b x\n",
            1,
            0,
            "c d<https://a.b>\n\n&#x20;x\n",
        ),
    ] {
        let mut document = parse(source);
        let paragraphs = paragraphs(&document);
        let link = links(&document)[0];
        assert_eq!(
            document.node(link).unwrap().parent(),
            Some(paragraphs[from])
        );
        document.attach(paragraphs[to], link, None).unwrap();
        assert_eq!(document.syntax(link), bare());
        assert_serializes(&document, expected);
    }
}

#[test]
fn literal_autolinks_after_edited_neighbours_fall_back() {
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
    assert_serializes(&document, "*a*[www.a.b](http://www.a.b)\n");
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

/// A paragraph of `before`, a bare link with `text` to `destination` inside `wrapper` when given,
/// and `after`.
fn constructed(
    before: &str,
    wrapper: Option<Tag<'static>>,
    text: &str,
    destination: &str,
    after: &str,
) -> Document<String> {
    let mut document = parse("x\n");
    let root = document.root();
    let paragraph = paragraphs(&document)[0];
    document.detach(paragraph).unwrap();
    let paragraph = document
        .append_child(root, EventKind::Start(Tag::Paragraph { tight: false }))
        .unwrap();
    for (value, link) in [(before, false), ("", true), (after, false)] {
        if link {
            let parent = match wrapper.clone() {
                Some(tag) => document
                    .append_child(paragraph, EventKind::Start(tag))
                    .unwrap(),
                None => paragraph,
            };
            let link = document
                .append_child(
                    parent,
                    EventKind::Start(Tag::Link {
                        destination: Text::from(destination.to_owned()),
                        title: None,
                    }),
                )
                .unwrap();
            document
                .append_child(link, EventKind::Text(Text::from(text.to_owned())))
                .unwrap();
            document.set_syntax(link, bare()).unwrap();
        } else if !value.is_empty() {
            document
                .append_child(paragraph, EventKind::Text(Text::from(value.to_owned())))
                .unwrap();
        }
    }
    document
}

#[test]
fn constructed_bare_links_are_bare_only_between_boundaries() {
    for (before, wrapper, text, destination, after, expected) in [
        (
            "",
            None,
            "https://a.b/c",
            "https://a.b/c",
            "",
            "https://a.b/c\n",
        ),
        (
            "a ",
            None,
            "https://a.b/c",
            "https://a.b/c",
            " b",
            "a https://a.b/c b\n",
        ),
        (
            "a\t",
            None,
            "www.a.b",
            "http://www.a.b",
            "\tb",
            "a\twww.a.b\tb\n",
        ),
        ("a\n", None, "a@b.c", "mailto:a@b.c", "\nb", "a\na@b.c\nb\n"),
        (
            "a",
            None,
            "https://a.b/c",
            "https://a.b/c",
            "",
            "a<https://a.b/c>\n",
        ),
        (
            "",
            None,
            "https://a.b/c",
            "https://a.b/c",
            "b",
            "<https://a.b/c>b\n",
        ),
        (
            "",
            None,
            "https://a.b/c",
            "https://a.b/c",
            ".",
            "<https://a.b/c>.\n",
        ),
        (
            "a ",
            None,
            "www.a.b",
            "http://www.a.b",
            "\u{2014}",
            "a [www.a.b](http://www.a.b)\u{2014}\n",
        ),
        ("a ", None, "a@b.c", "mailto:a@b.c", "-", "a <a@b.c>-\n"),
        (
            "",
            Some(Tag::Emphasis),
            "https://a.b/c",
            "https://a.b/c",
            "",
            "*<https://a.b/c>*\n",
        ),
        (
            "",
            Some(Tag::Strong),
            "www.a.b",
            "http://www.a.b",
            "",
            "**[www.a.b](http://www.a.b)**\n",
        ),
        (
            "",
            Some(Tag::Delete),
            "a@b.c",
            "mailto:a@b.c",
            "",
            "~~<a@b.c>~~\n",
        ),
        // Whitespace that would be written as a reference does not count.
        (
            "a ",
            None,
            "https://a.b/c",
            "https://a.b/c",
            " ",
            "a <https://a.b/c>&#x20;\n",
        ),
        (
            "\u{a0}",
            None,
            "https://a.b/c",
            "https://a.b/c",
            "",
            "\u{a0}<https://a.b/c>\n",
        ),
        (
            "a ",
            None,
            "https://a.b/c",
            "https://a.b/c",
            " \nb",
            "a <https://a.b/c>&#x20;\nb\n",
        ),
        (
            "a ",
            None,
            "https://a.b/c",
            "https://a.b/c",
            "\t \nb",
            "a <https://a.b/c>\t&#x20;\nb\n",
        ),
        // Text that is not the destination can only be written explicitly.
        ("a", None, "b", "https://a.b/c", "", "a[b](https://a.b/c)\n"),
    ] {
        let document = constructed(before, wrapper, text, destination, after);
        assert_serializes(&document, expected);
    }
}

#[test]
fn bare_links_in_table_cells_with_a_pipe_are_explicit() {
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
    assert_eq!(document.syntax(link), bare());
    assert_serializes(
        &document,
        "| a |\n| --- |\n| [www.a.b/x\\|y](http://www.a.b/x\\|y) |\n",
    );
}

#[test]
fn empty_text_after_a_bare_link_writes_nothing_beside_it() {
    let mut document = parse("www.a.b\n");
    let paragraph = paragraphs(&document)[0];
    let link = links(&document)[0];
    let empty = document
        .append_child(paragraph, EventKind::Text(Text::from(String::new())))
        .unwrap();
    assert_eq!(
        document.children(paragraph).collect::<Vec<_>>(),
        [link, empty]
    );
    assert_serializes(&document, "www.a.b\n");
}

#[test]
fn text_moved_after_strong_around_a_bare_link_makes_it_fall_back() {
    let mut document = parse("**bold www.x.y**\n\n-x\n");
    let paragraphs = paragraphs(&document);
    let moved = document.children(paragraphs[1]).next().unwrap();
    document.detach(moved).unwrap();
    document.attach(paragraphs[0], moved, None).unwrap();
    document.detach(paragraphs[1]).unwrap();
    assert_serializes(&document, "**bold [www.x.y](http://www.x.y)**-x\n");
}

#[test]
fn a_tilde_written_at_the_edges_of_strikethrough_is_escaped() {
    for (value, expected) in [
        ("~5 min", "~~\\~5 min~~\n"),
        ("5 min~", "~~5 min\\~~~\n"),
        ("~", "~~\\~~~\n"),
        ("~~", "~~\\~\\~~~\n"),
        ("5 min\\~", "~~5 min\\\\\\~~~\n"),
    ] {
        let mut document = parse("~~x~~\n");
        let paragraph = paragraphs(&document)[0];
        let strikethrough = document.children(paragraph).next().unwrap();
        let text = document.children(strikethrough).next().unwrap();
        *document.text_mut(text).unwrap() = value.to_owned();
        assert_serializes(&document, expected);
    }
}

#[test]
fn bare_links_with_edited_text_fall_back() {
    let mut document = parse("see www.a.b now\n");
    let link = links(&document)[0];
    let text = document.children(link).next().unwrap();
    *document.text_mut(text).unwrap() = "click here".to_owned();
    assert_eq!(document.syntax(link), bare());
    assert_serializes(&document, "see [click here](http://www.a.b) now\n");
    // Text that is a literal to another destination is not written bare either.
    *document.text_mut(text).unwrap() = "https://evil.example".to_owned();
    assert_serializes(
        &document,
        "see [https://evil.example](http://www.a.b) now\n",
    );
}

#[test]
fn bare_links_are_bare_only_when_their_text_reads_back_as_the_same_literal() {
    for (text, destination, expected) in [
        ("www.a.b", "http://www.a.b", "www.a.b\n"),
        ("a@b.co", "mailto:a@b.co", "a@b.co\n"),
        ("https://a.b", "https://a.b", "https://a.b\n"),
        // A changed destination.
        ("www.a.b", "https://www.a.b", "[www.a.b](https://www.a.b)\n"),
        ("www.a.b", "www.a.b", "[www.a.b](www.a.b)\n"),
        ("a@b.co", "a@b.co", "[a@b.co](a@b.co)\n"),
        ("https://a.b", "https://c.d", "[https://a.b](https://c.d)\n"),
        (
            "https://evil.example",
            "https://a.b",
            "[https://evil.example](https://a.b)\n",
        ),
        // Text that is not one whole literal.
        ("<script>", "https://a.b", "[\\<script>](https://a.b)\n"),
        (
            "[x](javascript:alert(1))",
            "https://a.b",
            "[\\[x\\](javascript:alert(1))](https://a.b)\n",
        ),
        (
            "www.a.b c",
            "http://www.a.b c",
            "[www.a.b c](<http://www.a.b c>)\n",
        ),
    ] {
        let document = constructed("", None, text, destination, "");
        assert_serializes(&document, expected);
    }
}
