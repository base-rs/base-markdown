// Released under the MIT License.
// Copyright, 2022, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

//! Utilities used when processing markdown.
pub mod char;
pub mod character_reference;
pub mod constant;
pub mod edit_map;
pub mod encode;
pub mod gfm_tagfilter;
pub mod infer;
pub(crate) mod inline_code_info;
pub mod line_ending;
pub mod normalize_identifier;
pub mod sanitize_uri;
pub mod skip;
pub mod slice;
pub mod unicode;
