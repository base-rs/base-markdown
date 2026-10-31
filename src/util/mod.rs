// Released under the MIT License.
// Copyright, 2022, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

//! Utilities used when processing markdown.
pub(crate) mod autolink;
pub mod char;
pub mod character_reference;
pub mod constant;
pub mod encode;
pub(crate) mod html_text;
pub(crate) mod inline_code_info;
pub mod line_ending;
pub mod normalize_identifier;
pub mod sanitize_uri;
pub(crate) mod scheme;
pub mod tagfilter;
pub mod unicode;
