// Released under the MIT License.
// Copyright, 2022-2025, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

//! Turn bytes of markdown into events.
use crate::definitions::{DefinitionLookup, Definitions};
use crate::event::{Event, Point};
use crate::state::{Name as StateName, State};
#[cfg(test)]
use crate::subtokenize::subtokenize;
use crate::tokenizer::Tokenizer;
use crate::ParseOptions;
use alloc::{borrow::Cow, string::String, vec::Vec};

/// Info needed, in all content types, when parsing markdown.
///
/// Importantly, this contains a set of known definitions.
/// It also references the input value as bytes (`u8`).
#[derive(Debug)]
pub struct ParseState<'a> {
    /// Configuration.
    pub options: &'a ParseOptions,
    /// List of chars.
    pub bytes: &'a [u8],
    /// Set of defined definition identifiers.
    pub definitions: DefinitionLookup<'a>,
    /// Set of defined GFM footnote definition identifiers.
    pub gfm_footnote_definitions: Cow<'a, [String]>,
}

impl<'a> ParseState<'a> {
    /// Inline parsing needs a borrowed lookup context, never another definition owner.
    pub fn inline(
        bytes: &'a [u8],
        options: &'a ParseOptions,
        definitions: &'a Definitions,
        footnotes: &'a [String],
    ) -> Self {
        Self {
            bytes,
            options,
            definitions: DefinitionLookup::Prepared(definitions),
            gfm_footnote_definitions: Cow::Borrowed(footnotes),
        }
    }
}

/// Turn a string of markdown into events.
///
/// Passes the bytes back so the compiler can access the source.
pub fn prepare<'a>(value: &'a str, options: &'a ParseOptions) -> (Vec<Event>, ParseState<'a>) {
    let bytes = value.as_bytes();

    let mut parse_state = ParseState {
        options,
        bytes,
        definitions: DefinitionLookup::Keys(Vec::new()),
        gfm_footnote_definitions: Cow::Borrowed(&[]),
    };

    let start = Point {
        line: 1,
        column: 1,
        index: 0,
        vs: 0,
    };
    let mut tokenizer = Tokenizer::new(start, &parse_state);

    let state = tokenizer.push(
        (0, 0),
        (parse_state.bytes.len(), 0),
        State::Next(StateName::DocumentStart),
    );
    let result = tokenizer.flush(state, true);
    let events = tokenizer.events;

    parse_state.gfm_footnote_definitions = Cow::Owned(result.gfm_footnote_definitions);
    parse_state.definitions = DefinitionLookup::from_keys(result.definitions);
    (events, parse_state)
}

/// Fully resolve grammar tokens for tokenizer regression tests.
#[cfg(test)]
pub fn parse<'a>(value: &'a str, options: &'a ParseOptions) -> (Vec<Event>, ParseState<'a>) {
    let (mut events, mut parse_state) = prepare(value, options);
    loop {
        let mut result = subtokenize(&mut events, &parse_state, None);
        parse_state
            .gfm_footnote_definitions
            .to_mut()
            .append(&mut result.gfm_footnote_definitions);
        let DefinitionLookup::Keys(keys) = &mut parse_state.definitions else {
            unreachable!("the grammar-only parser owns its definition identifiers");
        };
        keys.append(&mut result.definitions);
        keys.sort_unstable();
        keys.dedup();
        if result.done {
            return (events, parse_state);
        }
    }
}
