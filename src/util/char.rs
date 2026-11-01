// Released under the MIT License.
// Copyright, 2022-2025, by Titus Wormer.
// Copyright, 2023, by pinkforest(she/her).
// Copyright, 2026, by Samuel Williams.

//! Deal with bytes, chars, and kinds.
use crate::util::unicode::PUNCTUATION;

/// Character kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Whitespace.
    ///
    /// ## Example
    ///
    /// ```markdown
    /// > | **a_b_ c**.
    ///    ^      ^    ^
    /// ```
    Whitespace,
    /// Punctuation.
    ///
    /// ## Example
    ///
    /// ```markdown
    /// > | **a_b_ c**.
    ///     ^^ ^ ^    ^
    /// ```
    Punctuation,
    /// Everything else.
    ///
    /// ## Example
    ///
    /// ```markdown
    /// > | **a_b_ c**.
    ///       ^ ^  ^
    /// ```
    Other,
}

/// Classify whether a `char` represents whitespace, punctuation, or something
/// else.
///
/// Used for attention (emphasis, strong), whose sequences can open or close
/// based on the class of surrounding characters.
///
/// ## References
///
/// * [`micromark-util-classify-character` in `micromark`](https://github.com/micromark/micromark/blob/main/packages/micromark-util-classify-character/dev/index.js)
pub fn classify(char: char) -> Kind {
    // Unicode whitespace.
    if char.is_whitespace() {
        Kind::Whitespace
    }
    // Unicode punctuation.
    else if char.is_ascii_punctuation()
        || (!char.is_ascii() && PUNCTUATION.binary_search(&char).is_ok())
    {
        Kind::Punctuation
    }
    // Everything else.
    else {
        Kind::Other
    }
}

/// Like [`classify`], but supports eof as whitespace.
pub fn classify_opt(char_opt: Option<char>) -> Kind {
    char_opt.map_or(Kind::Whitespace, classify)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify() {
        assert_eq!(
            classify(' '),
            Kind::Whitespace,
            "should classify whitespace"
        );

        assert_eq!(
            classify('.'),
            Kind::Punctuation,
            "should classify punctuation"
        );

        assert_eq!(classify('a'), Kind::Other, "should classify other");
    }

    #[test]
    fn classifies_unicode_punctuation_with_sorted_table() {
        assert!(PUNCTUATION.windows(2).all(|pair| pair[0] < pair[1]));
        for &character in &PUNCTUATION {
            assert_eq!(classify(character), Kind::Punctuation);
        }
        assert_eq!(classify('界'), Kind::Other);
        assert_eq!(classify('\u{2003}'), Kind::Whitespace);
        assert_eq!(classify('\0'), Kind::Other);
    }
}
