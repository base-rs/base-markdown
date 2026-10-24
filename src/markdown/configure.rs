// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

//! Configuration.
//!
//! JS equivalent: https://github.com/syntax-tree/mdast-util-to-markdown/blob/fd6a508/lib/types.js#L307.
use crate::message::Message;
use alloc::{format, string::String};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Configuration for indent of lists.
pub enum IndentOptions {
    /// Depends on the item and its parent list: uses `IndentOptions::One` if
    /// the item and list are tight and `IndentOptions::Tab` otherwise.
    Mixed,
    /// The size of the bullet plus one space.
    One,
    /// Tab stop.
    Tab,
}

/// Configuration for soft line breaks in phrasing content.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LineWrapping {
    /// Preserve soft line breaks from the syntax tree (the default).
    #[default]
    Preserve,
    /// Replace soft line breaks with spaces.
    ///
    /// Explicit Markdown hard breaks, code, and block boundaries are preserved.
    Unwrap,
}

/// Configuration.
#[derive(Clone, Debug)]
pub struct Options {
    /// How to handle soft line breaks in text (default: `LineWrapping::Preserve`).
    pub line_wrapping: LineWrapping,
    /// Marker to use for bullets of items in unordered lists (`'*'`, `'+'`, or
    /// `'-'`, default: `'-'`).
    pub bullet: char,
    /// Marker to use for bullets of items in ordered lists (`'.'` or `')'`,
    /// default: `'.'`).
    pub bullet_ordered: char,
    /// Marker to use in certain cases where the primary bullet doesn’t work
    /// (`'*'`, `'+'`, or `'-'`, default: `'-'` when bullet is `'*'`, `'*'`
    /// otherwise).
    pub bullet_other: char,
    /// Whether to add the same number of number signs (`#`) at the end of an
    /// ATX heading as the opening sequence (`bool`, default: `false`).
    pub close_atx: bool,
    /// Marker to use for emphasis (`'*'` or `'_'`, default: `'*'`).
    pub emphasis: char,
    /// Marker to use for fenced code (``'`'`` or `'~'`, default: ``'`'``).
    pub fence: char,
    /// Whether to use fenced code always (`bool`, default: `true`).
    /// The default is to use fenced code if there is a language defined,
    /// if the code is empty,
    /// or if it starts or ends in blank lines.
    pub fences: bool,
    /// Whether to increment the counter of ordered lists items (`bool`,
    /// default: `true`).
    pub increment_list_marker: bool,
    /// How to indent the content of list items (default: `IndentOptions::One`).
    pub list_item_indent: IndentOptions,
    /// Marker to use for titles (`'"'` or `"'"`, default: `'"'`).
    pub quote: char,
    /// Whether to always use resource links (`bool`, default: `false`).
    /// The default is to use autolinks (`<https://example.com>`) when possible
    /// and resource links (`[text](url)`) otherwise.
    pub resource_link: bool,
    /// Marker to use for thematic breaks (`'*'`, `'-'`, or `'_'`, default:
    /// `'*'`).
    pub rule: char,
    /// Number of markers to use for thematic breaks (`u32`, default: `3`, min:
    /// `3`).
    pub rule_repetition: u32,
    /// Whether to add spaces between markers in thematic breaks (`bool`,
    /// default: `false`).
    pub rule_spaces: bool,
    /// Whether to use setext headings when possible (`bool`, default:
    /// `false`).
    /// The default is to always use ATX headings (`# heading`) instead of
    /// setext headings (`heading\n=======`).
    /// Setext headings cannot be used for empty headings or headings with a
    /// rank of three or more.
    pub setext: bool,
    /// Whether to support math (text) with a single dollar (`bool`, default: `true`).
    /// Single dollars work in Pandoc and many other places, but often interfere with “normal”
    /// dollars in text.
    /// If you turn this off, you can still use two or more dollars for text math.
    pub single_dollar_text_math: bool,
    /// Marker to use for strong (`'*'` or `'_'`, default: `'*'`).
    pub strong: char,
    /// Whether to join definitions without a blank line (`bool`, default:
    /// `false`).
    pub tight_definitions: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            line_wrapping: LineWrapping::default(),
            bullet: '-',
            bullet_ordered: '.',
            bullet_other: '-',
            close_atx: false,
            emphasis: '*',
            fence: '`',
            fences: true,
            increment_list_marker: true,
            list_item_indent: IndentOptions::One,
            quote: '"',
            resource_link: false,
            rule: '*',
            rule_repetition: 3,
            rule_spaces: false,
            setext: false,
            single_dollar_text_math: true,
            strong: '*',
            tight_definitions: false,
        }
    }
}

impl Options {
    pub(super) fn check_bullet(&self) -> Result<char, Message> {
        let marker = self.bullet;

        if marker != '*' && marker != '+' && marker != '-' {
            return Err(invalid_option(
                format!("Cannot serialize items with `{marker}` for `options.bullet`, expected `*`, `+`, or `-`"),
                "unexpected-marker",
            ));
        }

        Ok(marker)
    }

    pub(super) fn check_bullet_ordered(&self) -> Result<char, Message> {
        let marker = self.bullet_ordered;

        if marker != '.' && marker != ')' {
            return Err(invalid_option(format!("Cannot serialize items with `{marker}` for `options.bullet_ordered`, expected `.` or `)`"), "unexpected-marker"));
        }

        Ok(marker)
    }

    pub(super) fn check_bullet_other(&self) -> Result<char, Message> {
        let bullet = self.check_bullet()?;
        let mut bullet_other = self.bullet_other;

        if bullet != '*' {
            bullet_other = '*';
        }

        if bullet_other != '*' && bullet_other != '+' && bullet_other != '-' {
            return Err(invalid_option(format!("Cannot serialize items with `{bullet_other}` for `options.bullet_other`, expected `*`, `+`, or `-`"), "unexpected-marker"));
        }

        if bullet_other == bullet {
            return Err(invalid_option(
                format!("Expected `bullet` (`{bullet}`) and `bullet_other` (`{bullet_other}`) to be different"),
                "bullet-match-bullet_other",
            ));
        }

        Ok(bullet_other)
    }

    pub(super) fn check_emphasis(&self) -> Result<char, Message> {
        let marker = self.emphasis;

        if marker != '*' && marker != '_' {
            return Err(invalid_option(
                format!("Cannot serialize emphasis with `{marker}` for `options.emphasis`, expected `*`, or `_`"),
                "unexpected-marker",
            ));
        }

        Ok(marker)
    }

    pub(super) fn check_fence(&self) -> Result<char, Message> {
        let marker = self.fence;

        if marker != '`' && marker != '~' {
            return Err(invalid_option(
                format!("Cannot serialize code with `{marker}` for `options.fence`, expected `` ` `` or `~`"),
                "unexpected-marker",
            ));
        }

        Ok(marker)
    }

    pub(super) fn check_quote(&self) -> Result<char, Message> {
        let marker = self.quote;

        if marker != '"' && marker != '\'' {
            return Err(invalid_option(
                format!("Cannot serialize title with `{marker}`  for `options.quote`, expected `\"`, or `'`"),
                "unexpected-marker",
            ));
        }

        Ok(marker)
    }

    pub(super) fn check_rule(&self) -> Result<char, Message> {
        let marker = self.rule;

        if marker != '*' && marker != '-' && marker != '_' {
            return Err(invalid_option(
                format!("Cannot serialize rules with `{marker}` for `options.rule`, expected `*`, `-`, or `_`"),
                "unexpected-marker",
            ));
        }

        Ok(marker)
    }

    pub(super) fn check_rule_repetition(&self) -> Result<u32, Message> {
        let repetition = self.rule_repetition;

        if repetition < 3 {
            return Err(invalid_option(format!("Cannot serialize rules with repetition `{repetition}` for `options.rule_repetition`, expected `3` or more"), "unexpected-marker"));
        }

        Ok(repetition)
    }

    pub(super) fn check_strong(&self) -> Result<char, Message> {
        let marker = self.strong;

        if marker != '*' && marker != '_' {
            return Err(invalid_option(
                format!("Cannot serialize strong with `{marker}` for `options.strong`, expected `*`, or `_`"),
                "unexpected-marker",
            ));
        }

        Ok(marker)
    }
}

fn invalid_option(reason: String, rule_id: &'static str) -> Message {
    Message {
        span: None,
        reason: reason.into(),
        rule_id,
        source: "mdast-util-to-markdown",
    }
}

#[cfg(test)]
mod tests;
