// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! When a run of attention markers opens and closes, and which runs pair, as `CommonMark` 0.31.2
//! §6.2 and GFM strikethrough define it.
//!
//! These are pure functions of the characters around a run and of run lengths. The inline
//! parser resolves attention with them, and the Markdown writer chooses its sequences with them,
//! so that the two cannot drift apart.
use crate::util::char::Kind;
use core::convert::TryFrom;

/// Whether a run of `marker` (`*`, `_` or `~`) between characters of kinds `before` and `after`
/// can open and close. The edge of a block, or of the text a run is read in, counts as
/// whitespace.
///
/// A run is left-flanking when it is not followed by whitespace, and not followed by punctuation
/// unless it follows whitespace or punctuation; right-flanking is the mirror. `*` and `~` open
/// when left-flanking and close when right-flanking; `_` also needs, to open, not to be
/// right-flanking or to follow punctuation, and to close, not to be left-flanking or to precede
/// punctuation, so that it forms no attention inside a word.
pub fn sides(marker: u8, before: Kind, after: Kind) -> (bool, bool) {
    let left = after != Kind::Whitespace && (after != Kind::Punctuation || before != Kind::Other);
    let right = before != Kind::Whitespace && (before != Kind::Punctuation || after != Kind::Other);
    if marker == b'_' {
        (
            left && (!right || before == Kind::Punctuation),
            right && (!left || after == Kind::Punctuation),
        )
    } else {
        (left, right)
    }
}

/// A run of markers, as far as pairing it needs.
#[derive(Clone, Copy, Debug)]
pub struct Run {
    /// How many markers are left in it.
    pub size: usize,
    /// How many markers it had before any paired, modulo 3, which the rule of 3 compares.
    pub modulo: u8,
    /// Whether it can open.
    pub open: bool,
    /// Whether it can close.
    pub close: bool,
}

impl Run {
    /// A run of `size` markers, none of which have paired.
    pub fn whole(size: usize, open: bool, close: bool) -> Self {
        Run {
            size,
            modulo: u8::try_from(size % 3).expect("a remainder is small"),
            open,
            close,
        }
    }
}

/// Whether `closing`, a run of `marker` that can close, pairs with `opening`, an earlier run of
/// the same marker that can open.
///
/// Tildes pair only with a run of as many tildes. Asterisks and underscores pair unless the rule
/// of 3 keeps them apart: where one of the runs can both open and close, the lengths of the two
/// runs must not add up to a multiple of 3, unless both are multiples of 3.
pub fn pairs(marker: u8, opening: Run, closing: Run) -> bool {
    if marker == b'~' {
        opening.size == closing.size
    } else {
        !((opening.close || closing.open)
            && (opening.modulo + closing.modulo).is_multiple_of(3)
            && (opening.modulo != 0 || closing.modulo != 0))
    }
}

/// Whether an opening run of `marker` and `size` that can also close would close an earlier
/// open run of the same marker and `enclosing` size, before either has paired: the parser
/// tries a run that can close as a closing run first.
pub fn closes_enclosing(marker: u8, enclosing: usize, size: usize) -> bool {
    pairs(
        marker,
        Run::whole(enclosing, true, false),
        Run::whole(size, true, true),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use Kind::{Other as O, Punctuation as P, Whitespace as W};

    #[test]
    fn flanking() {
        // Asterisks and tildes.
        for &marker in b"*~" {
            assert_eq!(sides(marker, W, O), (true, false));
            assert_eq!(sides(marker, P, O), (true, false));
            assert_eq!(sides(marker, O, O), (true, true));
            assert_eq!(sides(marker, P, P), (true, true));
            assert_eq!(sides(marker, O, P), (false, true));
            assert_eq!(sides(marker, O, W), (false, true));
            assert_eq!(sides(marker, W, W), (false, false));
        }
        // Underscores form no attention inside a word.
        assert_eq!(sides(b'_', O, O), (false, false));
        assert_eq!(sides(b'_', P, P), (true, true));
        assert_eq!(sides(b'_', P, O), (true, false));
        assert_eq!(sides(b'_', O, P), (false, true));
    }

    #[test]
    fn rule_of_three() {
        let run = Run::whole;
        assert!(pairs(b'*', run(1, true, false), run(2, false, true)));
        assert!(!pairs(b'*', run(1, true, true), run(2, false, true)));
        assert!(!pairs(b'*', run(1, true, false), run(2, true, true)));
        assert!(pairs(b'*', run(1, true, true), run(1, true, true)));
        assert!(pairs(b'*', run(2, true, true), run(2, true, true)));
        assert!(pairs(b'*', run(3, true, true), run(3, true, true)));
        assert!(pairs(b'~', run(1, true, true), run(1, true, true)));
        assert!(!pairs(b'~', run(1, true, false), run(2, false, true)));
    }
}
