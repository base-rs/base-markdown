// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

//! Code formatting rules adapted from `mdast-util-to-markdown`.
use crate::markdown::state::State;

/// Whether code is written indented: when the options ask for no fences, or the document's
/// dialect has no fenced code, and the code has a shape that indented code reads back as. With
/// indented code off, it is always fenced.
pub fn format_code_as_indented(value: &str, no_info: bool, state: &State) -> bool {
    let non_whitespace = value.chars().any(|character| !character.is_whitespace());
    let begins_blank = value.trim_start_matches([' ', '\t']).starts_with('\n');
    let ends_blank = value.trim_end_matches([' ', '\t']).ends_with('\n');
    let constructs = &state.parse.constructs;
    (!state.options.fences || !constructs.code_fenced)
        && constructs.code_indented
        && !value.is_empty()
        && no_info
        && non_whitespace
        && !begins_blank
        && !ends_blank
}
