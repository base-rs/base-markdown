// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use crate::{
    construct::{
        content, flow, frontmatter, gfm_autolink_literal, html_flow, html_text, label_end,
        partial_data,
    },
    event::Point,
    grammar_parser::GrammarParser,
    parser::ParseState,
    state::{Name as StateName, State},
    tokenizer::Tokenizer,
    Constructs, ParseOptions,
};

fn with_tokenizer<T>(
    source: &str,
    options: &ParseOptions,
    current: Option<u8>,
    run: impl FnOnce(&mut Tokenizer<'_>) -> T,
) -> T {
    let parse_state = ParseState::new(source.as_bytes(), options);
    let mut tokenizer = Tokenizer::new(
        Point {
            line: 1,
            column: 1,
            index: 0,
            vs: 0,
        },
        &parse_state,
    );
    tokenizer.current = current;
    run(&mut tokenizer)
}

fn parse(source: &str, options: &ParseOptions) {
    GrammarParser::new(source, options).for_each(|event| {
        event.unwrap();
    });
}

#[test]
fn partial_data_retries_when_there_is_no_initial_marker() {
    with_tokenizer("", &ParseOptions::default(), None, |tokenizer| {
        assert_eq!(
            partial_data::start(tokenizer),
            State::Retry(StateName::DataAtBreak)
        );
    });
}

#[test]
fn content_resolver_subtokenizes_even_an_empty_event_stream() {
    with_tokenizer("", &ParseOptions::default(), None, |tokenizer| {
        assert!(content::resolve(tokenizer).done);
    });
}

#[test]
fn gfm_autolink_literal_accepts_uppercase_protocol_and_www_prefixes() {
    let options = ParseOptions {
        constructs: Constructs {
            gfm_autolink_literal: true,
            ..Constructs::default()
        },
        ..ParseOptions::default()
    };

    with_tokenizer("H", &options, Some(b'H'), |tokenizer| {
        assert_eq!(
            gfm_autolink_literal::protocol_start(tokenizer),
            State::Retry(StateName::GfmAutolinkLiteralProtocolPrefixInside)
        );
    });

    with_tokenizer("W", &options, Some(b'W'), |tokenizer| {
        assert_eq!(
            gfm_autolink_literal::www_start(tokenizer),
            State::Retry(StateName::GfmAutolinkLiteralWwwPrefixInside)
        );
    });
}

#[test]
fn accepts_eof_and_line_endings_after_content_definitions_and_frontmatter() {
    with_tokenizer("", &ParseOptions::default(), None, |tokenizer| {
        assert_eq!(content::definition_after(tokenizer), State::Ok);
    });

    parse("[label]: /url", &ParseOptions::default());
    parse("[label]: /url\n", &ParseOptions::default());

    let frontmatter_options = ParseOptions {
        constructs: Constructs {
            frontmatter: true,
            ..Constructs::default()
        },
        ..ParseOptions::default()
    };
    parse("---\ntitle: Example\n---", &frontmatter_options);
    parse("---\ntitle: Example\n---\n", &frontmatter_options);
}

#[test]
#[should_panic(expected = "expected eol/eof after closing fence")]
fn frontmatter_after_rejects_other_bytes() {
    with_tokenizer(
        "x",
        &ParseOptions::default(),
        Some(b'x'),
        frontmatter::after,
    );
}

#[test]
#[should_panic(expected = "assertion failed: matches!(tokenizer.current, None | Some(b'\\n'))")]
fn content_definition_after_rejects_other_bytes() {
    with_tokenizer(
        "x",
        &ParseOptions::default(),
        Some(b'x'),
        content::definition_after,
    );
}

#[test]
#[should_panic(expected = "unexpected eol/eof")]
fn content_chunk_rejects_an_empty_start() {
    with_tokenizer("", &ParseOptions::default(), None, content::chunk_start);
}

#[test]
#[should_panic(expected = "expected eol/eof")]
fn blank_line_after_rejects_non_line_endings() {
    with_tokenizer(
        "x",
        &ParseOptions::default(),
        Some(b'x'),
        flow::blank_line_after,
    );
}

#[test]
#[should_panic(expected = "expected eof/eol")]
fn frontmatter_content_end_rejects_other_bytes() {
    with_tokenizer(
        "x",
        &ParseOptions::default(),
        Some(b'x'),
        frontmatter::content_end,
    );
}

#[test]
#[should_panic(expected = "expected eol")]
fn html_flow_continuation_requires_a_line_ending() {
    with_tokenizer(
        "x",
        &ParseOptions::default(),
        Some(b'x'),
        html_flow::continuation_start_non_lazy,
    );
}

#[test]
#[should_panic(expected = "expected eol")]
fn html_text_line_ending_requires_a_line_ending() {
    with_tokenizer(
        "x",
        &ParseOptions::default(),
        Some(b'x'),
        html_text::line_ending_before,
    );
}

#[test]
#[should_panic(expected = "expected `(`")]
fn resource_start_requires_an_opening_parenthesis() {
    with_tokenizer(
        "x",
        &ParseOptions::default(),
        Some(b'x'),
        label_end::resource_start,
    );
}

#[test]
#[should_panic(expected = "expected `[`")]
fn full_reference_start_requires_an_opening_bracket() {
    with_tokenizer(
        "x",
        &ParseOptions::default(),
        Some(b'x'),
        label_end::reference_full,
    );
}
