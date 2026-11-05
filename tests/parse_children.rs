// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

// `Document::parse_children` parses Markdown into a node's children (#121).

mod support;

use base_markdown::{
    stream::{EventKind, StreamError, Tag, Text},
    to_markdown, CompileOptions, Document, NodeId, NodeSyntax, ParseOptions, ReferenceKind,
};
use support::fixture::canonical;
use support::linear::timed;

fn gfm() -> ParseOptions {
    ParseOptions::gfm()
}

fn parse(source: &str) -> Document {
    Document::parse(source.to_owned(), &gfm()).unwrap()
}

fn rendered<S: AsRef<str>>(document: &Document<S>) -> String {
    let mut output = String::new();
    document
        .render_html(&CompileOptions::gfm(), &mut output)
        .unwrap();
    output
}

/// Normalize `document`, serialize it, and assert that it reads back as the same tree and HTML
/// and is written the same again. Returns the Markdown.
fn reads_back(mut document: Document) -> String {
    document.normalize();
    let markdown = to_markdown(&document).unwrap();
    let mut reparsed = Document::parse(markdown.as_str(), &gfm()).unwrap();
    reparsed.normalize();
    assert_eq!(canonical(&reparsed), canonical(&document), "{markdown:?}");
    assert_eq!(rendered(&reparsed), rendered(&document), "{markdown:?}");
    assert_eq!(to_markdown(&reparsed).unwrap(), markdown);
    markdown
}

/// The first node in document order whose kind `matches` accepts.
fn find(document: &Document, matches: impl Fn(&EventKind<'_>) -> bool) -> NodeId {
    document
        .descendants(document.root())
        .find(|id| matches(&document.node(*id).unwrap().kind))
        .unwrap()
}

fn kinds(document: &Document, parent: NodeId) -> Vec<String> {
    document
        .descendants(parent)
        .skip(1)
        .map(|id| format!("{:?}", document.node(id).unwrap().kind))
        .collect()
}

fn destinations(document: &Document) -> Vec<String> {
    document
        .descendants(document.root())
        .filter_map(|id| match &document.node(id).unwrap().kind {
            EventKind::Start(Tag::Link { destination, .. }) => {
                Some(destination.resolve(document.source()).unwrap().to_owned())
            }
            _ => None,
        })
        .collect()
}

fn footnote_references(document: &Document) -> usize {
    document
        .descendants(document.root())
        .filter(|id| {
            matches!(
                document.node(*id).unwrap().kind,
                EventKind::FootnoteReference { .. }
            )
        })
        .count()
}

#[test]
fn a_paragraph_takes_the_parsed_phrasing_of_its_new_content() {
    let mut document = parse("Old *text*.\n\nNext.\n");
    let paragraph = document.children(document.root()).next().unwrap();
    let old: Vec<_> = document.children(paragraph).collect();
    document
        .parse_children(paragraph, "Visit www.google.com for more details.", &gfm())
        .unwrap();
    let link = document.children(paragraph).nth(1).unwrap();
    assert_eq!(destinations(&document), ["https://www.google.com"]);
    assert_eq!(
        document.syntax(link),
        Some(NodeSyntax::Link {
            form: base_markdown::LinkForm::Bare
        })
    );
    // The replaced children are detached and keep their IDs.
    for id in old {
        assert_eq!(document.node(id).unwrap().parent(), None);
    }
    assert_eq!(
        reads_back(document),
        "Visit www.google.com for more details.\n\nNext.\n"
    );
}

#[test]
fn every_container_that_holds_phrasing_reads_a_paragraphs_content() {
    let source = "# Heading\n\n*a* **b** ~~c~~ [d](/d) ![e](/e)\n\n| f |\n| - |\n| g |\n";
    let containers: [fn(&EventKind<'_>) -> bool; 7] = [
        |kind| matches!(kind, EventKind::Start(Tag::Heading(_))),
        |kind| matches!(kind, EventKind::Start(Tag::Paragraph { .. })),
        |kind| matches!(kind, EventKind::Start(Tag::Emphasis)),
        |kind| matches!(kind, EventKind::Start(Tag::Strong)),
        |kind| matches!(kind, EventKind::Start(Tag::Delete)),
        |kind| matches!(kind, EventKind::Start(Tag::Link { .. })),
        |kind| matches!(kind, EventKind::Start(Tag::TableCell { .. })),
    ];
    for container in containers {
        let mut document = parse(source);
        let parent = find(&document, container);
        document
            .parse_children(parent, "  x _y_ `z` <b>h</b> \\* &amp;  ", &gfm())
            .unwrap();
        assert_eq!(
            document.text_content(parent),
            "x y z <b>h</b> * &",
            "{:?}",
            document.node(parent).unwrap().kind
        );
        reads_back(document);
    }
    // An image's alternative text is written as plain text.
    let mut document = parse(source);
    let image = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::Image { .. }))
    });
    document.parse_children(image, "x &amp; y", &gfm()).unwrap();
    assert_eq!(document.text_content(image), "x & y");
    reads_back(document);
}

#[test]
fn a_container_that_holds_phrasing_rejects_block_syntax_and_is_left_unchanged() {
    for fragment in [
        "# h",
        "a\n\nb",
        "- x",
        "1. x",
        "a\n===",
        "a\n---",
        "[a]: /b",
        "[^a]: b",
        "    code",
        "```\ncode\n```",
        "> q",
        "***",
        "<div>\n",
        "| a |\n| - |",
        "---\nk: v\n---\n",
    ] {
        let mut document = parse("Some *old* text.\n");
        let paragraph = document.children(document.root()).next().unwrap();
        let before = kinds(&document, document.root());
        assert!(
            matches!(
                document.parse_children(paragraph, fragment, &gfm()),
                Err(StreamError::InvalidNesting)
            ),
            "{:?}",
            fragment
        );
        assert_eq!(kinds(&document, document.root()), before);
        assert_eq!(to_markdown(&document).unwrap(), "Some *old* text.\n");
    }
}

#[test]
fn a_fragment_of_nothing_empties_the_parent() {
    for fragment in ["", "\n\n", "  \n \n"] {
        let mut document = parse("A [*b*](/c).\n\n> Quote.\n");
        let link = find(&document, |kind| {
            matches!(kind, EventKind::Start(Tag::Link { .. }))
        });
        document.parse_children(link, fragment, &gfm()).unwrap();
        assert_eq!(document.children(link).count(), 0);
        let quote = find(&document, |kind| {
            matches!(kind, EventKind::Start(Tag::BlockQuote))
        });
        document.parse_children(quote, fragment, &gfm()).unwrap();
        assert_eq!(document.children(quote).count(), 0);
        reads_back(document);
    }
}

#[test]
fn every_container_that_holds_blocks_reads_blocks() {
    let source = "> Quote.\n\n- Item.\n\nCall.[^n]\n\n[^n]: Note.\n";
    let fragment = "# Title\n\nText with `code`.\n\n- a\n- b\n\n```rust\nfn main() {}\n```\n";
    let containers: [fn(&EventKind<'_>) -> bool; 4] = [
        |kind| matches!(kind, EventKind::Start(Tag::Document { .. })),
        |kind| matches!(kind, EventKind::Start(Tag::BlockQuote)),
        |kind| matches!(kind, EventKind::Start(Tag::Item { .. })),
        |kind| matches!(kind, EventKind::Start(Tag::FootnoteDefinition { .. })),
    ];
    for container in containers {
        let mut document = parse(source);
        let parent = find(&document, container);
        document.parse_children(parent, fragment, &gfm()).unwrap();
        let blocks: Vec<_> = document
            .children(parent)
            .map(|id| document.node(id).unwrap().kind.clone())
            .filter(|kind| !matches!(kind, EventKind::Separator { .. }))
            .collect();
        assert!(matches!(
            blocks.as_slice(),
            [
                EventKind::Start(Tag::Heading(1)),
                EventKind::Start(Tag::Paragraph { .. }),
                EventKind::Start(Tag::List { .. }),
                EventKind::CodeBlock { .. },
            ]
        ));
        reads_back(document);
    }
}

#[test]
fn lists_tables_and_their_parts_and_leaves_are_rejected() {
    let mut document = parse("- a\n\n| b |\n| - |\n| c |\n");
    let before = kinds(&document, document.root());
    let containers: [fn(&EventKind<'_>) -> bool; 5] = [
        |kind| matches!(kind, EventKind::Start(Tag::List { .. })),
        |kind| matches!(kind, EventKind::Start(Tag::Table)),
        |kind| matches!(kind, EventKind::Start(Tag::TableHead)),
        |kind| matches!(kind, EventKind::Start(Tag::TableBody)),
        |kind| matches!(kind, EventKind::Start(Tag::TableRow)),
    ];
    for container in containers {
        let parent = find(&document, container);
        assert!(matches!(
            document.parse_children(parent, "- x", &gfm()),
            Err(StreamError::InvalidNesting)
        ));
    }
    let text = find(&document, |kind| matches!(kind, EventKind::Text(_)));
    assert!(matches!(
        document.parse_children(text, "x", &gfm()),
        Err(StreamError::Unbalanced)
    ));
    let foreign = parse("a\n\nb\n\nc\n\nd\n\ne\n\nf\n\ng\n\nh\n");
    let missing = foreign.descendants(foreign.root()).last().unwrap();
    assert!(matches!(
        document.parse_children(missing, "x", &gfm()),
        Err(StreamError::InvalidRange)
    ));
    assert_eq!(kinds(&document, document.root()), before);
}

#[test]
fn references_and_footnote_calls_resolve_against_the_documents_definitions() {
    let mut document = parse("Old.\n\n[r]: /u \"t\"\n\n[^n]: Note.\n");
    let paragraph = document.children(document.root()).next().unwrap();
    document
        .parse_children(
            paragraph,
            "[x][R], [r][], [r], [y][missing], [^n] and [^missing].",
            &gfm(),
        )
        .unwrap();
    assert_eq!(destinations(&document), ["/u", "/u", "/u"]);
    assert_eq!(footnote_references(&document), 1);
    let link = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::Link { .. }))
    });
    // A reference keeps its label, which reads back to the same definition.
    assert!(matches!(
        &document.node(link).unwrap().kind,
        EventKind::Start(Tag::Link { reference: Some(reference), .. })
            if reference.kind == ReferenceKind::Full
                && reference.label.resolve(document.source()).ok() == Some("R")
    ));
    assert_eq!(
        reads_back(document),
        "[x][R], [r][], [r], \\[y]\\[missing], [^n] and \\[^missing].\n\n[r]: /u \"t\"\n\n[^n]: Note.\n"
    );
}

#[test]
fn a_fragment_without_definitions_or_references_needs_no_definitions() {
    let mut document = parse("Old.\n\n[r]: /u\n");
    let paragraph = document.children(document.root()).next().unwrap();
    document.parse_children(paragraph, "r] x", &gfm()).unwrap();
    assert_eq!(document.text_content(paragraph), "r] x");
}

#[test]
fn a_footnote_call_inside_a_link_stays_text_as_the_parser_leaves_it() {
    // The parser reads the call and leaves the link's brackets text instead.
    let whole = parse("[a [^n]](/u)\n\n[^n]: Note.\n");
    assert_eq!(footnote_references(&whole), 1);
    assert!(destinations(&whole).is_empty());
    let mut document = parse("[a *b*](/u)\n\n[^n]: Note.\n");
    let emphasis = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::Emphasis))
    });
    document.parse_children(emphasis, "c [^n]", &gfm()).unwrap();
    assert_eq!(footnote_references(&document), 0);
    assert_eq!(document.text_content(emphasis), "c [^n]");
    let link = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::Link { .. }))
    });
    document.parse_children(link, "[^n] d", &gfm()).unwrap();
    assert_eq!(footnote_references(&document), 0);
    reads_back(document);
    // An image outside a link can hold a call.
    let mut document = parse("![a](/u)\n\n[^n]: Note.\n");
    let image = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::Image { .. }))
    });
    document.parse_children(image, "b [^n]", &gfm()).unwrap();
    assert_eq!(footnote_references(&document), 1);
}

#[test]
fn a_block_fragments_definitions_take_their_place_in_document_order() {
    let source = "[r]: /before\n\n> Old [x][old].\n>\n> [old]: /old\n\n[r]: /after\n\n[s]: /after-s\n\n[t]: /after-t\n\n[^n]: Note.\n";
    let mut document = parse(source);
    let quote = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::BlockQuote))
    });
    document
        .parse_children(
            quote,
            "[a][r] [b][s] [c][t] [d][old] [^n]\n\n[r]: /own-r\n[s]: /own-s\n[s]: /own-s2\n",
            &gfm(),
        )
        .unwrap();
    // A definition before the parent wins over the fragment's own, and the fragment's own over
    // one after it. A definition among the replaced children no longer counts.
    assert_eq!(destinations(&document), ["/before", "/own-s", "/after-t"]);
    assert_eq!(footnote_references(&document), 1);
    reads_back(document);
}

#[test]
fn an_existing_reference_that_a_fragment_definition_shadows_is_written_inline() {
    let mut document = parse("[x][r]\n\n> Quote.\n\n[r]: /a\n");
    let quote = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::BlockQuote))
    });
    document.parse_children(quote, "[r]: /b", &gfm()).unwrap();
    assert_eq!(destinations(&document), ["/a"]);
    // It is written as a resource, so it reads back as one, to the same destination.
    let markdown = to_markdown(&document).unwrap();
    assert_eq!(markdown, "[x](/a)\n\n> [r]: /b\n\n[r]: /a\n");
    let reparsed = Document::parse(markdown.clone(), &gfm()).unwrap();
    assert_eq!(destinations(&reparsed), ["/a"]);
    assert_eq!(rendered(&reparsed), rendered(&document));
    assert_eq!(to_markdown(&reparsed).unwrap(), markdown);
}

#[test]
fn frontmatter_is_never_read_and_the_documents_own_is_kept() {
    let mut options = gfm();
    options.constructs.frontmatter = true;
    let mut document = Document::parse("---\nk: v\n---\n\nOld.\n".to_owned(), &options).unwrap();
    let root = document.root();
    document
        .parse_children(root, "---\ntitle: x\n---\n", &options)
        .unwrap();
    assert_eq!(document.frontmatter(), Some("k: v\n"));
    let blocks: Vec<_> = document
        .children(root)
        .map(|id| document.node(id).unwrap().kind.clone())
        .filter(|kind| !matches!(kind, EventKind::Separator { .. }))
        .collect();
    assert!(matches!(
        blocks.as_slice(),
        [EventKind::Rule, EventKind::Start(Tag::Heading(2))]
    ));
    let markdown = to_markdown(&document).unwrap();
    let reparsed = Document::parse(markdown.as_str(), &options).unwrap();
    assert_eq!(canonical(&reparsed), canonical(&document), "{markdown:?}");
}

#[test]
fn an_item_and_its_paragraphs_follow_the_lists_tightness() {
    let mut document = parse("- a\n- b\n");
    let item = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::Item { .. }))
    });
    document.parse_children(item, "c", &gfm()).unwrap();
    assert_eq!(rendered(&document), "<ul>\n<li>c</li>\n<li>b</li>\n</ul>\n");
    document
        .parse_children(item, "c\n\n```\nd\n```", &gfm())
        .unwrap();
    assert!(matches!(
        document.node(item).unwrap().kind,
        EventKind::Start(Tag::Item {
            block_end: true,
            ..
        })
    ));
    reads_back(document);
}

#[test]
fn an_inline_root_becomes_a_root_of_blocks() {
    let mut document = Document::parse(String::new(), &gfm()).unwrap();
    let root = document.root();
    document
        .append_child(root, EventKind::Text(Text::Literal("inline")))
        .unwrap();
    document.parse_children(root, "# h\n\np", &gfm()).unwrap();
    let markdown = to_markdown(&document).unwrap();
    assert_eq!(markdown, "# h\n\np\n");
    let reparsed = Document::parse(markdown.as_str(), &gfm()).unwrap();
    assert_eq!(canonical(&reparsed), canonical(&document));
}

#[test]
fn values_are_owned_and_line_endings_stored_as_line_feeds() {
    let mut document = parse("> Old.\n");
    let quote = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::BlockQuote))
    });
    let fragment = String::from("a\r\nb [c](/d)\r\n\r\n    code\r\n");
    document.parse_children(quote, &fragment, &gfm()).unwrap();
    drop(fragment);
    assert_eq!(document.source(), "> Old.\n");
    assert_eq!(
        reads_back(document),
        "> a\n> b [c](/d)\n>\n> ```\n> code\n> ```\n"
    );
}

#[test]
fn a_table_cell_rejects_a_line_ending_and_is_left_unchanged() {
    for fragment in ["a\nb", "a  \nb", "a\\\nb", "a\r\nb"] {
        let mut document = parse("| a |\n| - |\n| b |\n");
        let cell = find(&document, |kind| {
            matches!(kind, EventKind::Start(Tag::TableCell { header: false, .. }))
        });
        assert!(
            matches!(
                document.parse_children(cell, fragment, &gfm()),
                Err(StreamError::InvalidNesting)
            ),
            "{:?}",
            fragment
        );
        assert_eq!(document.text_content(cell), "b");
    }
    // Other phrasing parents take one.
    let mut document = parse("c\n");
    let paragraph = document.children(document.root()).next().unwrap();
    document.parse_children(paragraph, "a\nb", &gfm()).unwrap();
    reads_back(document);
}

#[test]
fn labels_resolve_exactly_as_the_parser_reads_them() {
    // A label with an escaped bracket, and one continued on a line of a block quote.
    let mut document = parse("Old.\n\n> Quote.\n\n[a\\]b]: /escaped\n\n[a b]: /quoted\n");
    let paragraph = document.children(document.root()).next().unwrap();
    document
        .parse_children(paragraph, "[a\\]b] and [x][A\\]B]", &gfm())
        .unwrap();
    assert_eq!(destinations(&document), ["/escaped", "/escaped"]);
    let quote = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::BlockQuote))
    });
    document
        .parse_children(quote, "> [x][a\n> b]", &gfm())
        .unwrap();
    assert_eq!(destinations(&document), ["/escaped", "/escaped", "/quoted"]);
    reads_back(document);
}

#[test]
fn a_fragments_own_footnote_definition_comes_before_one_after_its_parent() {
    let mut document = parse("> Old.\n\n[^n]: After.\n");
    let quote = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::BlockQuote))
    });
    document
        .parse_children(quote, "Call.[^n]\n\n[^n]: Own.\n", &gfm())
        .unwrap();
    let own = document.footnote_definition("n").unwrap();
    assert_eq!(document.text_content(own), "Own.");
    assert_eq!(footnote_references(&document), 1);
    reads_back(document);
}

#[test]
fn a_detached_parent_resolves_against_the_trees_definitions() {
    let mut document = parse("Old.\n\n[r]: /u\n");
    let paragraph = document.children(document.root()).next().unwrap();
    document.detach(paragraph).unwrap();
    document.parse_children(paragraph, "[r]", &gfm()).unwrap();
    assert!(matches!(
        &document.node(document.children(paragraph).next().unwrap()).unwrap().kind,
        EventKind::Start(Tag::Link { destination, .. })
            if destination.resolve(document.source()).ok() == Some("/u")
    ));
}

/// Parsing into every paragraph of a document takes time linear in the document: each parse
/// looks up the definitions its fragment refers to, without searching the document (#121). The
/// paragraphs of one large document are timed against the same paragraphs, one in each of as
/// many small documents, a control of the same work whose documents are short, so the ratio of
/// the two growths shows only work that depends on the size of the document: searching the
/// document's blocks in each parse grew 2.7 times the control's growth here.
#[test]
fn parsing_into_every_paragraph_of_a_document_is_linear() {
    let _timing = support::linear::TIMING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let definitions = "[r]: /u\n\n[^n]: Note.\n\n";
    let large = |count: usize| vec![parse(&(String::from(definitions) + &"p\n\n".repeat(count)))];
    let small = |count: usize| {
        (0..count)
            .map(|_| parse(&(String::from(definitions) + "p\n")))
            .collect::<Vec<_>>()
    };
    let time = |documents: &[Document]| {
        let mut documents = documents.to_vec();
        let paragraphs: Vec<Vec<NodeId>> = documents
            .iter()
            .map(|document| {
                document
                    .children(document.root())
                    .filter(|id| {
                        matches!(
                            document.node(*id).unwrap().kind,
                            EventKind::Start(Tag::Paragraph { .. })
                        )
                    })
                    .collect()
            })
            .collect();
        timed(|| {
            for (document, paragraphs) in documents.iter_mut().zip(&paragraphs) {
                for &paragraph in paragraphs {
                    document
                        .parse_children(paragraph, "q [r] *s* [^n]", &gfm())
                        .unwrap();
                }
            }
        })
        .as_secs_f64()
    };
    let documents = [
        large(1 << 10),
        large(1 << 12),
        small(1 << 10),
        small(1 << 12),
    ];
    let mut ratios = Vec::new();
    // Noise makes linear work look slower only now and then, so a ratio over the bound is
    // measured again up to twice.
    for _ in 0..3 {
        let times: Vec<f64> = documents.iter().map(|documents| time(documents)).collect();
        let ratio = (times[1] / times[0]) / (times[3] / times[2]);
        eprintln!("linear: {times:?}, {ratio:.2}x the control's growth");
        if ratio < 1.4 {
            return;
        }
        ratios.push(ratio);
    }
    panic!("the document grew {:?} times the control's growth", ratios);
}
