// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Inserting from another document does work linear in what it inserts, counted with [`crate::work`].

extern crate std;

use crate::work::tests::{assert_against_control, counted};
use crate::{Document, InsertOptions, NodeId, ParseOptions};
use core::fmt::Write as _;
use std::borrow::ToOwned;
use std::string::String;
use std::vec::Vec;

fn parse(source: &str) -> Document {
    Document::parse(source.to_owned(), &ParseOptions::gfm()).unwrap()
}

/// Many inserts of a document with footnotes into one document, each after the last, take time
/// linear in the inserts, against the same inserts into many documents.
#[test]
fn many_inserts_are_linear() {
    let other = parse("# h\n\nText [r] [^n].\n\n[^n]: Note.\n\n[r]: /r\n");
    let steps = |documents: usize, inserts: usize| {
        let mut documents: Vec<Document> = (0..documents).map(|_| parse("# A\n")).collect();
        counted(|| {
            for document in &mut documents {
                let root = document.root();
                let mut last = document.children(root).last();
                for _ in 0..inserts {
                    let inserted = document
                        .insert_from(
                            root,
                            last,
                            &other,
                            other.children(other.root()),
                            &InsertOptions::default(),
                        )
                        .unwrap();
                    last = inserted.footnote_definitions.last().copied();
                }
            }
        })
    };
    assert_against_control(
        "many inserts into one document",
        1 << 8,
        |size| steps(1, size),
        |size| steps(size, 1),
    );
}

/// Many footnotes that clash, each relabelled, take time linear in their number, whether they
/// are of many identifiers or one identifier inserted again and again.
#[test]
fn many_clashing_footnotes_are_linear() {
    let source = |count: usize| {
        let mut source = String::new();
        for index in 0..count {
            write!(source, "[^f{index}] ").unwrap();
        }
        source.push_str("\n\n");
        for index in 0..count {
            write!(source, "[^f{index}]: n\n\n").unwrap();
        }
        source
    };
    // `documents` documents, each with `count` footnotes, or none unless `clash`, into each of
    // which `inserts` copies of `count` footnotes are inserted.
    let steps = |documents: usize, count: usize, inserts: usize, clash: bool| {
        let other = parse(&source(count));
        let target = if clash {
            source(count)
        } else {
            String::from("x\n")
        };
        let mut documents: Vec<Document> = (0..documents).map(|_| parse(&target)).collect();
        counted(|| {
            for document in &mut documents {
                let root = document.root();
                for _ in 0..inserts {
                    document
                        .insert_from(
                            root,
                            None,
                            &other,
                            other.children(other.root()),
                            &InsertOptions::default(),
                        )
                        .unwrap();
                }
            }
        })
    };
    assert_against_control(
        "many clashing footnotes",
        1 << 9,
        |size| steps(1, size, 1, true),
        |size| steps(1, size, 1, false),
    );
    assert_against_control(
        "one footnote clashing again and again",
        1 << 8,
        |size| steps(1, 1, size, true),
        |size| steps(size, 1, 1, true),
    );
}

/// Many inserts into one item, each after the last, take time linear in the inserts, against the
/// same inserts into many items, though the footnote definitions each brings collect after them.
#[test]
fn many_inserts_into_one_item_are_linear() {
    let other = parse("x [^n]\n\n[^n]: n\n");
    let steps = |items: usize, inserts: usize| {
        let mut document = parse(&"- a\n\n".repeat(items));
        let list = document.children(document.root()).next().unwrap();
        let items: Vec<NodeId> = document.children(list).collect();
        counted(|| {
            for &item in &items {
                let mut after = document.children(item).last();
                for _ in 0..inserts {
                    let inserted = document
                        .insert_from(
                            item,
                            after,
                            &other,
                            other.children(other.root()),
                            &InsertOptions::default(),
                        )
                        .unwrap();
                    after = inserted.nodes.last().copied();
                }
            }
        })
    };
    assert_against_control(
        "many inserts into one item",
        1 << 10,
        |size| steps(1, size),
        |size| steps(size, 1),
    );
}
