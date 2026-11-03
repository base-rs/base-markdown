// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! CommonMark limits an ordered list marker to 9 digits, so the serializer returns an error for a
//! list item whose number would have more (#155). The number is the list's start plus, when
//! `increment_list_marker` is on, the item's position among the list's items.

mod support;
use base_markdown::{
    markdown::Options, message::Message, to_markdown, to_markdown_node, to_markdown_with_options,
    Document, NodeId, ParseOptions,
};
use pretty_assertions::assert_eq;
use support::fixture::{
    to_markdown as to, to_markdown_with_options as to_with, List, ListItem, Node, Paragraph, Text,
    ThematicBreak,
};

fn item(value: &str) -> Node {
    Node::ListItem(ListItem {
        children: vec![Node::Paragraph(Paragraph {
            children: vec![Node::Text(Text {
                value: value.into(),
                position: None,
            })],
            position: None,
        })],
        position: None,
        spread: false,
        checked: None,
    })
}

fn ordered(start: u32, values: &[&str]) -> Node {
    Node::List(List {
        children: values.iter().map(|value| item(value)).collect(),
        position: None,
        ordered: true,
        start: Some(start),
        spread: false,
    })
}

fn fixed() -> Options {
    Options {
        increment_list_marker: false,
        ..Options::default()
    }
}

fn assert_too_long(result: Result<String, Message>, number: u64) {
    let error = result.unwrap_err();
    assert_eq!(error.rule_id, "list-number-too-long");
    assert_eq!(error.source, "markdown");
    assert_eq!(
        error.reason.to_string(),
        format!(
            "List item number {number} has more than 9 digits, so it cannot be written as a list marker"
        )
    );
}

#[test]
fn start_with_ten_digits_is_an_error() {
    assert_too_long(to(&ordered(1_000_000_000, &["a"])), 1_000_000_000);
    assert_too_long(
        to_with(&ordered(1_000_000_000, &["a"]), &fixed()),
        1_000_000_000,
    );
}

#[test]
fn start_with_nine_digits_is_written() {
    assert_eq!(to(&ordered(999_999_999, &["a"])).unwrap(), "999999999. a\n");
    assert_eq!(to(&ordered(0, &["a", "b"])).unwrap(), "0. a\n1. b\n");
}

#[test]
fn incremented_number_with_ten_digits_is_an_error() {
    assert_too_long(to(&ordered(999_999_999, &["a", "b"])), 1_000_000_000);
    assert_too_long(to(&ordered(999_999_990, &["a"; 11])), 1_000_000_000);
    assert_eq!(
        to(&ordered(999_999_990, &["a"; 10]))
            .unwrap()
            .lines()
            .last(),
        Some("999999999. a")
    );
}

#[test]
fn without_increment_only_start_must_fit() {
    assert_eq!(
        to_with(&ordered(999_999_999, &["a", "b", "c"]), &fixed()).unwrap(),
        "999999999. a\n999999999. b\n999999999. c\n"
    );
}

#[test]
fn largest_start_does_not_overflow() {
    assert_too_long(to(&ordered(u32::MAX, &["a", "b"])), u64::from(u32::MAX));
    assert_too_long(
        to_with(&ordered(u32::MAX, &["a"]), &fixed()),
        u64::from(u32::MAX),
    );
}

#[test]
fn empty_list_is_written() {
    assert_eq!(to(&ordered(u32::MAX, &[])).unwrap(), "");
}

#[test]
fn non_item_child_is_unsupported_first() {
    let list = Node::List(List {
        children: vec![
            item("a"),
            Node::ThematicBreak(ThematicBreak { position: None }),
        ],
        position: None,
        ordered: true,
        start: Some(u32::MAX),
        spread: false,
    });
    assert_eq!(to(&list).unwrap_err().rule_id, "unsupported-node");
}

fn parse(source: &str) -> Document<&str> {
    let mut document = Document::parse(source, &ParseOptions::default()).unwrap();
    document.normalize();
    document
}

fn items(document: &Document<&str>) -> Vec<NodeId> {
    let list = document.children(document.root()).next().unwrap();
    document
        .children(list)
        .filter(|&id| document.children(id).next().is_some())
        .collect()
}

/// The parser reads at most 9 digits as a marker, so `1000000000.` after an item continues its
/// paragraph lazily, and serializing that is unchanged.
#[test]
fn parsed_ten_digit_line_is_a_lazy_continuation() {
    let document = parse("999999999. a\n1000000000. b\n");
    assert_eq!(items(&document).len(), 1);
    assert_eq!(
        to_markdown(&document).unwrap(),
        "999999999. a\n           1000000000\\. b\n"
    );
}

/// A parsed list that starts at 999999999 with a second item would number it 1000000000, which
/// reads back as the first item's text, so it is an error unless numbers are not incremented.
#[test]
fn parsed_list_that_would_number_past_nine_digits() {
    let document = parse("999999999. a\n999999999. b\n");
    assert_too_long(to_markdown(&document), 1_000_000_000);
    assert_eq!(
        to_markdown_with_options(&document, &fixed()).unwrap(),
        "999999999. a\n999999999. b\n"
    );
}

/// An item serialized on its own keeps its position, and its number, in its list.
#[test]
fn item_serialized_alone() {
    let document = parse("999999998. a\n\n999999998. b\n\n999999998. c\n");
    let items = items(&document);
    assert_eq!(items.len(), 3);
    let options = Options::default();
    assert_eq!(
        to_markdown_node(&document, items[1], &options).unwrap(),
        "999999999. b\n"
    );
    assert_too_long(
        to_markdown_node(&document, items[2], &options),
        1_000_000_000,
    );
    assert_eq!(
        to_markdown_node(&document, items[2], &fixed()).unwrap(),
        "999999998. c\n"
    );
}
