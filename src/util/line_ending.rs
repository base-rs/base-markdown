// Released under the MIT License.
// Copyright, 2022, by Bernhard Berger.
// Copyright, 2022-2025, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

use alloc::{str::FromStr, string::String};
use core::ops::Range;

/// Complete physical line endings, treating CRLF as one range.
pub(crate) fn line_endings(value: &str) -> impl Iterator<Item = Range<usize>> + '_ {
    let bytes = value.as_bytes();
    let mut at = 0;
    core::iter::from_fn(move || {
        let start = at + memchr::memchr2(b'\r', b'\n', &bytes[at..])?;
        at = start + 1;
        if bytes[start] == b'\r' && bytes.get(at) == Some(&b'\n') {
            at += 1;
        }
        Some(start..at)
    })
}

/// Start of one terminal physical line ending before `end`, or `end` itself.
#[inline]
pub(crate) fn terminal_ending_start(bytes: &[u8], mut end: usize) -> usize {
    if bytes.get(end.wrapping_sub(1)) == Some(&b'\n') {
        end -= 1;
    }
    if bytes.get(end.wrapping_sub(1)) == Some(&b'\r') {
        end -= 1;
    }
    end
}

/// Remove one terminal physical line ending, preserving preceding line endings.
pub(crate) fn trim_terminal_ending(value: &str) -> &str {
    &value[..terminal_ending_start(value.as_bytes(), value.len())]
}

/// Type of line endings in markdown.
///
/// Particularly when working with Windows, you might want to use
/// `LineEnding::CarriageReturnLineFeed`.
///
/// ## Examples
///
/// ```
/// use base_markdown::LineEnding;
/// # fn main() {
///
/// // Use a CR + LF combination:
/// let crlf = LineEnding::CarriageReturnLineFeed;
/// # }
/// ```
#[derive(Clone, Debug, Default, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum LineEnding {
    /// Both a carriage return (`\r`) and a line feed (`\n`).
    ///
    /// ## Example
    ///
    /// ```markdown
    /// a␍␊
    /// b
    /// ```
    #[cfg_attr(feature = "serde", serde(rename = "\r\n"))]
    CarriageReturnLineFeed,
    /// Sole carriage return (`\r`).
    ///
    /// ## Example
    ///
    /// ```markdown
    /// a␍
    /// b
    /// ```
    #[cfg_attr(feature = "serde", serde(rename = "\r"))]
    CarriageReturn,
    /// Sole line feed (`\n`).
    ///
    /// ## Example
    ///
    /// ```markdown
    /// a␊
    /// b
    /// ```
    #[default]
    #[cfg_attr(feature = "serde", serde(rename = "\n"))]
    LineFeed,
}

impl LineEnding {
    /// Detect the first authored line ending without decoding unrelated UTF-8 text.
    pub(crate) fn from_source(source: &str) -> Option<Self> {
        let bytes = source.as_bytes();
        let index = memchr::memchr2(b'\r', b'\n', bytes)?;
        Some(if bytes[index] == b'\n' {
            Self::LineFeed
        } else if bytes.get(index + 1) == Some(&b'\n') {
            Self::CarriageReturnLineFeed
        } else {
            Self::CarriageReturn
        })
    }

    /// Turn the line ending into a [str].
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            LineEnding::CarriageReturnLineFeed => "\r\n",
            LineEnding::CarriageReturn => "\r",
            LineEnding::LineFeed => "\n",
        }
    }
}

impl FromStr for LineEnding {
    type Err = String;

    /// Turn a string into a line ending.
    ///
    /// ## Panics
    ///
    /// Panics if `code` is not `\r\n`, `\r`, or `\n`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "\r\n" => Ok(LineEnding::CarriageReturnLineFeed),
            "\r" => Ok(LineEnding::CarriageReturn),
            "\n" => Ok(LineEnding::LineFeed),
            _ => Err("Expected CR, LF, or CRLF".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_complete_physical_line_endings() {
        let value = "é\r\nα\rb\n";
        assert_eq!(
            line_endings(value)
                .map(|range| &value[range])
                .collect::<alloc::vec::Vec<_>>(),
            ["\r\n", "\r", "\n"]
        );
    }

    #[test]
    fn trims_only_one_terminal_physical_line_ending() {
        for (source, expected) in [
            ("", ""),
            ("é", "é"),
            ("\n", ""),
            ("\r", ""),
            ("\r\n", ""),
            ("é\n", "é"),
            ("é\r", "é"),
            ("é\r\n", "é"),
            ("é\n\n", "é\n"),
            ("é\r\r\n", "é\r"),
            ("é\n\r", "é\n"),
        ] {
            assert_eq!(trim_terminal_ending(source), expected);
            let with_suffix = alloc::format!("{source}suffix");
            assert_eq!(
                terminal_ending_start(with_suffix.as_bytes(), source.len()),
                expected.len(),
            );
        }
    }

    #[test]
    fn detects_the_first_source_line_ending() {
        for (source, expected) in [
            ("plain é", None),
            ("é\rfirst\n", Some(LineEnding::CarriageReturn)),
            ("\n\r", Some(LineEnding::LineFeed)),
            ("é\r\nfirst\r", Some(LineEnding::CarriageReturnLineFeed)),
        ] {
            assert_eq!(LineEnding::from_source(source), expected);
        }
    }

    #[test]
    fn test_line_ending() {
        assert_eq!(
            "\r".parse(),
            Ok(LineEnding::CarriageReturn),
            "should support turning a string into a carriage return"
        );
        assert_eq!(
            LineEnding::CarriageReturn.as_str(),
            "\r",
            "should support turning a carriage return into a string"
        );

        assert_eq!(
            "\n".parse(),
            Ok(LineEnding::LineFeed),
            "should support turning a string into a line feed"
        );
        assert_eq!(
            LineEnding::LineFeed.as_str(),
            "\n",
            "should support turning a line feed into a string"
        );

        assert_eq!(
            "\r\n".parse(),
            Ok(LineEnding::CarriageReturnLineFeed),
            "should support turning a string into a carriage return + line feed"
        );
        assert_eq!(
            LineEnding::CarriageReturnLineFeed.as_str(),
            "\r\n",
            "should support turning a carriage return + line feed into a string"
        );

        assert_eq!(
            "aaa".parse::<LineEnding>(),
            Err("Expected CR, LF, or CRLF".into()),
            "should error when parsing a non-eol"
        );
    }
}
