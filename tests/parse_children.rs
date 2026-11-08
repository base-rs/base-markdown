// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

// `Document::parse_children` parses Markdown into a node's children (#121).

mod support;

use base_markdown::{
    stream::{EventKind, StreamError, Tag, Target, Text},
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

/// Where each link of the tree, in document order, leads, bound references included, and
/// `unbound` for a reference that nothing binds.
fn destinations(document: &Document) -> Vec<String> {
    document
        .descendants(document.root())
        .filter_map(|id| match &document.node(id).unwrap().kind {
            EventKind::Start(Tag::Link(_)) => Some(
                document
                    .destination(id)
                    .map_or("unbound", |(to, _)| to)
                    .to_owned(),
            ),
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
        // Link text is read as written in a link, where whitespace at its edges stays.
        let link = matches!(
            document.node(parent).unwrap().kind,
            EventKind::Start(Tag::Link { .. })
        );
        assert_eq!(
            document.text_content(parent),
            if link {
                "  x y z <b>h</b> * &  "
            } else {
                "x y z <b>h</b> * &"
            },
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
    // A reference is stored by its name, as in a parsed document.
    assert!(matches!(
        &document.node(link).unwrap().kind,
        EventKind::Start(Tag::Link(Target::Reference(reference)))
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
fn a_block_fragments_definitions_bind_for_the_rest_of_the_document() {
    let source =
        "[r]: /before\n\n[x][old] [y][s]\n\n> Old [x][old].\n>\n> [old]: /old\n\n[^n]: Note.\n";
    let mut document = parse(source);
    let quote = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::BlockQuote))
    });
    // `[y][s]` was text, and stays text: being a name, it does not become a reference by an edit.
    document
        .parse_children(
            quote,
            "[a][r] [b][s] [c][old] [^n]\n\n[s]: /own-s\n[old]: /new-old\n",
            &gfm(),
        )
        .unwrap();
    // A definition outside the parent binds, the fragment's own bind, and so do they the
    // reference before the parent, as a definition among the replaced children no longer does.
    assert_eq!(
        destinations(&document),
        ["/new-old", "/before", "/own-s", "/new-old"]
    );
    assert_eq!(footnote_references(&document), 1);
    reads_back(document);
}

/// As an edit adds no copy of a binding, a fragment that defines a label or footnote identifier
/// that the attached document outside the replaced children defines, or that defines one twice,
/// is rejected, and nothing changes, not even whether the document is known to be settled.
#[test]
fn a_fragment_that_would_add_a_copy_of_a_binding_is_rejected_and_changes_nothing() {
    let source = "[r]: /before\n\n> Old.\n>\n> [old]: /old\n\n[^n]: After.\n";
    for fragment in [
        "[r]: /own\n",
        "[R]: /own\n",
        "Call.[^n]\n\n[^n]: Own.\n",
        "[s]: /one\n[s]: /two\n",
        "[^m]: One.\n\n[^m]: Two.\n",
        "> [r]: /deep\n",
    ] {
        let mut document = parse(source);
        let quote = find(&document, |kind| {
            matches!(kind, EventKind::Start(Tag::BlockQuote))
        });
        let before = canonical(&document);
        let children: Vec<_> = document.children(quote).collect();
        assert!(
            matches!(
                document.parse_children(quote, fragment, &gfm()),
                Err(StreamError::InvalidValue)
            ),
            "{:?}",
            fragment
        );
        assert_eq!(canonical(&document), before, "{fragment:?}");
        assert_eq!(document.children(quote).collect::<Vec<_>>(), children);
        // The document is still settled, so normalizing it changes nothing and looks nothing up.
        assert!(document.is_normalized());
        assert_eq!(document.definition("old"), children.last().copied());
    }
    // The replaced children's own labels can be defined again, and a detached tree takes copies.
    let mut document = parse(source);
    let quote = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::BlockQuote))
    });
    document
        .parse_children(quote, "[old]: /again\n", &gfm())
        .unwrap();
    document.detach(quote).unwrap();
    document
        .parse_children(quote, "[old]: /one\n\n[old]: /two\n", &gfm())
        .unwrap();
}

#[test]
fn an_existing_reference_binds_to_the_definition_that_replaces_its_own() {
    let mut document = parse("[x][r]\n\n> [r]: /a\n");
    let quote = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::BlockQuote))
    });
    document.parse_children(quote, "[r]: /b", &gfm()).unwrap();
    assert_eq!(destinations(&document), ["/b"]);
    assert_eq!(reads_back(document), "[x][r]\n\n> [r]: /b\n");
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
fn a_fragments_own_footnote_definition_binds_its_calls() {
    let mut document = parse("> Old.\n\n[^m]: After.\n");
    let quote = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::BlockQuote))
    });
    document
        .parse_children(quote, "Call.[^n] [^m]\n\n[^n]: Own.\n", &gfm())
        .unwrap();
    let own = document.footnote_definition("N").unwrap();
    assert_eq!(document.text_content(own), "Own.");
    assert_eq!(footnote_references(&document), 2);
    reads_back(document);
}

/// Parsing into the container of a label's first definition binds the fragment's references to
/// the next definition of the label, which a parsed document may have, as the first leaves.
#[test]
fn parsing_into_the_container_of_the_first_definition_binds_the_next_outside_it() {
    let options = gfm();
    let mut document = parse("> [a]: /first\n\nText.\n\n[a]: /second\n");
    let quote = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::BlockQuote))
    });
    document.parse_children(quote, "[a]", &options).unwrap();
    assert_eq!(destinations(&document), ["/second"]);
    let paragraph = document.children(document.root()).nth(2).unwrap();
    document.parse_children(paragraph, "[a]", &options).unwrap();
    assert_eq!(destinations(&document), ["/second", "/second"]);
    reads_back(document);
}

/// The definitions a fragment adds are indexed, and those of the replaced children are not, in
/// the attached document only.
#[test]
fn parsing_children_adds_and_removes_definitions() {
    let options = gfm();
    let mut document = parse("> [b]: /old\n\n[x][b] [x][c]\n");
    let quote = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::BlockQuote))
    });
    let old = document.definition("b").unwrap();
    document
        .parse_children(quote, "[c]: /c\n\n[^n]: Note.\n", &options)
        .unwrap();
    assert_eq!(document.definition("b"), None);
    let c = document.definition("c").unwrap();
    assert_eq!(document.node(c).unwrap().parent(), Some(quote));
    assert!(document.footnote_definition("N").is_some());
    // `[x][b]` is a name that nothing binds now, and `[x][c]` was read as text.
    assert_eq!(destinations(&document), ["unbound"]);
    // A detached parent's fragment is not indexed.
    document.detach(quote).unwrap();
    document
        .parse_children(quote, "[d]: /d\n", &options)
        .unwrap();
    assert_eq!(document.definition("d"), None);
    assert_eq!(document.definition("c"), None);
    // The old definition is detached, as `extract_children` leaves it.
    assert_eq!(document.node(old).unwrap().parent(), None);
}

/// An item's spread is the fragment's, which the copied blocks have no source to infer it from,
/// so a loose item whose fragment holds a blank line is written with it.
#[test]
fn an_item_takes_the_spread_of_its_fragment() {
    let mut document = parse("- old\n\n- z\n");
    let item = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::Item { .. }))
    });
    document.parse_children(item, "b\n\n<b>\n", &gfm()).unwrap();
    assert_eq!(
        document.syntax(item),
        Some(NodeSyntax::ListItem { spread: true })
    );
    assert_eq!(reads_back(document), "- b\n\n  <b>\n\n- z\n");
    let mut document = parse("- old\n\n- z\n");
    let item = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::Item { .. }))
    });
    document.parse_children(item, "b\n> q\n", &gfm()).unwrap();
    assert_eq!(
        document.syntax(item),
        Some(NodeSyntax::ListItem { spread: false })
    );
    reads_back(document);
}

#[test]
fn a_detached_parent_resolves_against_the_trees_definitions() {
    let mut document = parse("> Old.\n>\n> [r]: /u\n\n[s]: /s\n");
    let quote = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::BlockQuote))
    });
    let paragraph = document.children(quote).next().unwrap();
    document.detach(quote).unwrap();
    // A detached tree binds only its own references, so `[s]` stays text.
    document
        .parse_children(paragraph, "[r] [s]", &gfm())
        .unwrap();
    let link = document.children(paragraph).next().unwrap();
    assert!(matches!(
        &document.node(link).unwrap().kind,
        EventKind::Start(Tag::Link(Target::Reference(_)))
    ));
    assert_eq!(document.destination(link), Some(("/u", None)));
    assert_eq!(document.text_content(paragraph), "r [s]");
    // Parsing into the quote, its definition no longer binds.
    document.parse_children(quote, "[r] [s]", &gfm()).unwrap();
    assert_eq!(document.text_content(quote), "[r] [s]");
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

/// Parsing into each of many sections, each a block quote that defines a label of its own, which
/// the fragment defines again and refers to, takes time linear in the sections: each parse asks
/// the index about the labels its fragment uses and passes over the replaced children's own
/// definitions, without searching the document. Timed against the same sections, one in each of as
/// many small documents.
#[test]
fn parsing_into_every_section_of_a_document_is_linear() {
    let section = |index: usize| format!("> [d{index}]: /old\n>\n> [d{index}] [r]\n\n");
    let build = |sections: usize, first: usize| {
        let mut source = String::from("[r]: /u\n\n");
        for index in first..first + sections {
            source.push_str(&section(index));
        }
        parse(&source)
    };
    let time = |mut documents: Vec<Document>| {
        let quotes: Vec<Vec<(NodeId, usize)>> = documents
            .iter()
            .map(|document| {
                document
                    .children(document.root())
                    .filter(|id| quote(&document.node(*id).unwrap().kind))
                    .map(|id| {
                        let definition = document.children(id).next().unwrap();
                        let label = match &document.node(definition).unwrap().kind {
                            EventKind::Definition { label, .. } => label
                                .resolve(document.source())
                                .unwrap()
                                .trim_start_matches('d')
                                .parse()
                                .unwrap(),
                            _ => unreachable!(),
                        };
                        (id, label)
                    })
                    .collect()
            })
            .collect();
        let fragments: Vec<Vec<String>> = quotes
            .iter()
            .map(|quotes| {
                quotes
                    .iter()
                    .map(|(_, label)| format!("[d{label}] [r] [^n]\n\n[d{label}]: /new\n"))
                    .collect()
            })
            .collect();
        timed(|| {
            for ((document, quotes), fragments) in documents.iter_mut().zip(&quotes).zip(&fragments)
            {
                for ((quote, _), fragment) in quotes.iter().zip(fragments) {
                    document.parse_children(*quote, fragment, &gfm()).unwrap();
                }
            }
        })
    };
    support::linear::assert_against_control(
        "parse_children into each section",
        1 << 9,
        |size| time(vec![build(size, 0)]),
        |size| time((0..size).map(|index| build(1, index)).collect()),
    );
}

/// Parsing into the innermost paragraph and block quote of a deep nest of block quotes, again and
/// again, takes time linear in the parses whatever the depth: nothing walks up through the nest.
/// Timed against the same parses into a nest of one.
#[test]
fn parsing_deep_into_a_nest_is_linear() {
    let time = |depth: usize, parses: usize| {
        let mut source = String::from("[r]: /u\n\n");
        source.push_str(&">".repeat(depth));
        source.push_str(" p\n");
        let mut document = parse(&source);
        let paragraph = find(&document, |kind| {
            matches!(kind, EventKind::Start(Tag::Paragraph { .. }))
        });
        let quote = document.node(paragraph).unwrap().parent().unwrap();
        timed(|| {
            for _ in 0..parses {
                document
                    .parse_children(quote, "q [r] [d]\n\n[d]: /d\n", &gfm())
                    .unwrap();
                let paragraph = document.children(quote).next().unwrap();
                document
                    .parse_children(paragraph, "s [r] [d]", &gfm())
                    .unwrap();
            }
        })
    };
    support::linear::assert_against_control(
        "parse_children deep in a nest",
        1 << 8,
        |size| time(size, size),
        |size| time(1, size),
    );
}

/// Assert that parsing `fragment` into the first node of `source` that `parent` accepts builds the
/// tree that `source` with `PLACEHOLDER` replaced by `spliced` reads as, the fragment written there.
fn assert_splices(
    source: &str,
    parent: impl Fn(&EventKind<'_>) -> bool,
    fragment: &str,
    spliced: &str,
) {
    let mut document = parse(source);
    let id = find(&document, parent);
    document.parse_children(id, fragment, &gfm()).unwrap();
    let written = parse(&source.replacen("PLACEHOLDER", spliced, 1));
    assert_eq!(canonical(&document), canonical(&written), "{fragment:?}");
    assert_eq!(rendered(&document), rendered(&written), "{fragment:?}");
}

fn quote(kind: &EventKind<'_>) -> bool {
    matches!(kind, EventKind::Start(Tag::BlockQuote))
}

/// Each footnote call of a chain is found only once the one around it resolves, so recording the
/// keys a parse looks up and parsing again with them would take a parse for each; resolving each
/// lookup as the parser makes it takes one (#121).
#[test]
fn a_chain_of_references_resolves_in_one_parse() {
    let definitions: String = (0..33)
        .map(|index| format!("[^n{index}]: Note.\n\n[n{index}]: /u{index}\n\n"))
        .collect();
    let source = format!("> PLACEHOLDER\n\n{definitions}");
    for wrap in ["[^n{}]({})", "[{}][n{}]", "[[^n{}] {}]"] {
        let fragment = (0..33).fold(String::from("x"), |inner, index| {
            wrap.replacen("{}", &index.to_string(), 1)
                .replacen("{}", &inner, 1)
        });
        assert_splices(&source, quote, &fragment, &fragment);
    }
}

/// A fragment read in an item gains no separator that the parser writes only at the root: not
/// after its last block, and not after a paragraph that the item's tight list makes tight. A
/// separator between two paragraphs stays, as only one makes them two, though a tight list cannot
/// write them apart, which normalizing does not yet fix (#216).
#[test]
fn a_fragment_read_in_an_item_adds_no_separator_the_parser_would_not() {
    let item = |kind: &EventKind<'_>| matches!(kind, EventKind::Start(Tag::Item { .. }));
    for (fragment, spliced) in [
        ("a\n", "a"),
        ("a\n\n", "a"),
        ("a\n[^c]: x", "a\n  [^c]: x"),
        ("a\n[^c]: x\n", "a\n  [^c]: x"),
        ("a\n> q\n", "a\n  > q"),
        ("> q\n", "> q"),
        ("a\n- b\n", "a\n  - b"),
        ("# h\n", "# h"),
    ] {
        assert_splices("- PLACEHOLDER\n- z\n", item, fragment, spliced);
        assert_splices("- PLACEHOLDER\n\n- z\n", item, fragment, spliced);
    }
    let mut document = parse("- PLACEHOLDER\n");
    let id = find(&document, item);
    document.parse_children(id, "a\n\nb\n", &gfm()).unwrap();
    assert_eq!(
        kinds(&document, id)
            .iter()
            .filter(|kind| kind.starts_with("Separator"))
            .count(),
        1
    );
}

/// Phrasing in a link is read as the text of a link, so a fragment that the parser would read as a
/// link in place of the one around it is rejected, wherever it is in the link, as is one whose
/// brackets would take the link's own, such as `[a` or `a\`, and literal
/// autolinks do not form there; an angle autolink is not a link the parser reads as ending the one
/// around it, so it stays.
#[test]
fn a_link_rejects_a_fragment_that_holds_a_link() {
    let source = "[*t*](/u)\n\n[a]: /a\n\n[^n]: Note.\n";
    let link = |kind: &EventKind<'_>| matches!(kind, EventKind::Start(Tag::Link { .. }));
    let emphasis = |kind: &EventKind<'_>| matches!(kind, EventKind::Start(Tag::Emphasis));
    for fragment in [
        "[a]",
        "x [b](/b) y",
        "[^n][a]",
        "[x][a]",
        "*[a]*",
        "[a",
        "\\[a]",
        "a\\",
    ] {
        for parent in [link, emphasis] {
            let mut document = parse(source);
            let id = find(&document, parent);
            assert!(
                matches!(
                    document.parse_children(id, fragment, &gfm()),
                    Err(StreamError::InvalidNesting)
                ),
                "{:?}",
                fragment
            );
            assert_eq!(to_markdown(&document).unwrap(), source);
        }
    }
    let source = "[PLACEHOLDER](/u)\n\n[a]: /a\n\n[^n]: Note.\n";
    for fragment in ["<http://a>", "www.a.com", "`[a]`", "\\[a\\]", "![i](/i)"] {
        assert_splices(source, link, fragment, fragment);
    }
}

/// Phrasing in a table cell is read as the content of a cell, so a `|` ends it unless it is
/// escaped, in code too, and the result is the cell the fragment written there reads as.
#[test]
fn a_cell_reads_a_fragment_as_written_in_a_cell() {
    let source = "| PLACEHOLDER |\n| - |\n\n[a]: /a\n";
    let cell = |kind: &EventKind<'_>| matches!(kind, EventKind::Start(Tag::TableCell { .. }));
    for fragment in [
        ")a`\\|[a]`a\t",
        "a\\|b",
        "`a\\|b`",
        "[a\\|b](/u)",
        "[a]",
        " *a* ",
        "www.a.com",
        "a\\",
        "&#124;",
        "\\",
    ] {
        assert_splices(source, cell, fragment, fragment);
    }
    for fragment in ["a | b", "`a|b`", "a\nb", "|", "a |\n|-|\n| b"] {
        let mut document = parse(source);
        let id = find(&document, cell);
        assert!(
            matches!(
                document.parse_children(id, fragment, &gfm()),
                Err(StreamError::InvalidNesting)
            ),
            "{:?}",
            fragment
        );
    }
    // A cell is read as one whatever the options.
    let mut document = parse(source);
    let id = find(&document, cell);
    let mut options = gfm();
    options.constructs.table = false;
    document.parse_children(id, "a\\|b", &options).unwrap();
    assert_eq!(document.text_content(id), "a|b");
    // Phrasing inside a cell reads as in the cell too.
    let mut document = parse("| *x* |\n| - |\n");
    let emphasis = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::Emphasis))
    });
    document.parse_children(emphasis, "a\\|b", &gfm()).unwrap();
    assert_eq!(document.text_content(emphasis), "a|b");
    reads_back(document);
}
