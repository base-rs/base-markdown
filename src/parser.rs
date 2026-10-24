// Released under the MIT License.
// Copyright, 2022-2025, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

//! Turn bytes of markdown into events.
use crate::definitions::Definitions;
use crate::event::{Event, Point};
use crate::state::{Name as StateName, State};
use crate::tokenizer::Tokenizer;
use crate::ParseOptions;
use alloc::{string::String, vec::Vec};

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
    pub definitions: Option<&'a Definitions>,
    /// Set of defined GFM footnote definition identifiers.
    pub gfm_footnote_definitions: &'a [String],
}

impl<'a> ParseState<'a> {
    /// Block and string preparation do not resolve references.
    pub fn new(bytes: &'a [u8], options: &'a ParseOptions) -> Self {
        Self {
            bytes,
            options,
            definitions: None,
            gfm_footnote_definitions: &[],
        }
    }

    /// Inline parsing needs a borrowed lookup context, never another definition owner.
    pub fn inline(
        bytes: &'a [u8],
        options: &'a ParseOptions,
        definitions: &'a Definitions,
        footnotes: &'a [String],
    ) -> Self {
        Self {
            definitions: Some(definitions),
            gfm_footnote_definitions: footnotes,
            ..Self::new(bytes, options)
        }
    }
}

/// Turn a string of markdown into events.
///
/// Prepare block tokens and footnote identifiers before inline reference resolution.
pub fn prepare(value: &str, options: &ParseOptions) -> (Vec<Event>, Vec<String>) {
    let bytes = value.as_bytes();

    let parse_state = ParseState::new(bytes, options);

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

    (events, result.gfm_footnote_definitions)
}
