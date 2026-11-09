// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! The linearity tests of the clauses of `context/normalize-serialize.md`, which count work with [`crate::work`] rather than time it, so they are unit tests. The other clauses' tests are in `tests/contract.rs`.

extern crate std;

use crate::stream::{EventKind, Tag};
use crate::work::tests::{assert_linear, counted};
use crate::{to_markdown, CompileOptions, Document, NodeId, ParseOptions};
use std::string::String;
use std::vec::Vec;

fn parse(source: &str) -> Document<&str> {
    Document::parse(source, &ParseOptions::default()).unwrap()
}

fn html<S: AsRef<str>>(document: &Document<S>) -> String {
    let mut output = String::new();
    document
        .render_html(&CompileOptions::default(), &mut output)
        .unwrap();
    output
}

fn first<S: AsRef<str>>(document: &Document<S>, id: NodeId) -> NodeId {
    document.children(id).next().unwrap()
}

#[test]
fn clause_3_1_normalize_is_linear() {
    assert_linear("normalize", 400, |size| {
        let source = "a\\*b *c* _d_ [e][f] [^g]\n\n- h\n\n  [f]: /i\n".repeat(size);
        let mut document = Document::parse(source, &ParseOptions::default()).unwrap();
        counted(|| document.normalize())
    });
}

#[test]
fn clause_7_1_write_and_render_are_linear() {
    assert_linear("write and render", 300, |size| {
        let source = "# h\n\na *b* [c][d] `e`\n\n- f\n  > g\n\n[d]: /u\n".repeat(size);
        let mut document = Document::parse(source, &ParseOptions::default()).unwrap();
        document.normalize();
        counted(|| {
            to_markdown(&document).unwrap();
            html(&document);
        })
    });
}

#[test]
#[ignore = "#226: lookups in a detached tree build a table each time"]
fn clause_7_2_issue_226_detached_lookups() {
    assert_linear("text_content in a detached tree", 500, |size| {
        let source = "> [a]\n>\n".repeat(size) + "\n[a]: /u\n";
        let mut document = Document::parse(source, &ParseOptions::default()).unwrap();
        let quote = first(&document, document.root());
        document.detach(quote).unwrap();
        let paragraphs: Vec<NodeId> = document.children(quote).collect();
        counted(|| {
            for &paragraph_id in &paragraphs {
                document.text_content(paragraph_id);
            }
        })
    });
}

#[test]
#[ignore = "#233: edits deep in a phrasing nest walk every ancestor"]
fn clause_7_2_issue_233_phrasing_depth() {
    assert_linear("append_child deep in phrasing", 200, |size| {
        let mut document = parse("a\n");
        let mut parent = first(&document, document.root());
        for _ in 0..size {
            parent = document
                .append_child(parent, EventKind::Start(Tag::Emphasis))
                .unwrap();
        }
        counted(|| {
            for _ in 0..size {
                document.append_child(parent, EventKind::HardBreak).unwrap();
            }
        })
    });
}
