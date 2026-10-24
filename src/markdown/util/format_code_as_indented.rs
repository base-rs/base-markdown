// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

//! Code formatting rules adapted from `mdast-util-to-markdown`.
use crate::markdown::state::State;
use regex::Regex;

pub fn format_code_as_indented(value: &str, no_language: bool, state: &State) -> bool {
    let non_whitespace = value.chars().any(|character| !character.is_whitespace());
    let blank = Regex::new(r"^[\t ]*(?:[\r\n]|$)|(?:^|[\r\n])[\t ]*$").unwrap();
    !state.options.fences
        && !value.is_empty()
        && no_language
        && non_whitespace
        && !blank.is_match(value)
}
