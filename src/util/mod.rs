// Released under the MIT License.
// Copyright, 2022, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

//! Utilities used when processing markdown.
pub(crate) mod autolink;
pub mod char;
pub mod character_reference;
pub mod constant;
pub mod edit_map;
pub mod encode;
pub(crate) mod html_text;
pub mod infer;
pub(crate) mod inline_code_info;
pub mod line_ending;
pub mod normalize_identifier;
pub mod sanitize_uri;
pub mod skip;
pub mod slice;
pub mod tagfilter;
pub mod unicode;
