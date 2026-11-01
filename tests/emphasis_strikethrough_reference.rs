// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Emphasis next to GFM strikethrough, checked against a reference written from the specification text.
//!
//! The reference follows CommonMark 0.31.2, section 6.2 "Emphasis and strong emphasis" and the "process emphasis" procedure in appendix A. GFM strikethrough adds `~` runs of one or two tildes, which use the same flanking rules, match only a closer of the same length, and are not subject to the rule of 3.

use base_markdown::{to_html_with_options, Options};
use pretty_assertions::assert_eq;

struct Run {
    marker: char,
    length: usize,
    remaining: usize,
    can_open: bool,
    can_close: bool,
    opening_tags: Vec<&'static str>,
    closing_tags: Vec<&'static str>,
}

enum Piece {
    Text(char),
    Run(usize),
}

fn whitespace(character: Option<char>) -> bool {
    character.map_or(true, char::is_whitespace)
}

fn punctuation(character: Option<char>) -> bool {
    character.is_some_and(|character| character.is_ascii_punctuation())
}

/// Render a single-line paragraph containing only text and delimiter runs.
fn reference(source: &str) -> String {
    let characters: Vec<char> = source.chars().collect();
    let mut pieces = Vec::new();
    let mut runs: Vec<Run> = Vec::new();
    let mut index = 0;
    while index < characters.len() {
        let marker = characters[index];
        if !matches!(marker, '*' | '_' | '~') {
            pieces.push(Piece::Text(marker));
            index += 1;
            continue;
        }
        let start = index;
        while index < characters.len() && characters[index] == marker {
            index += 1;
        }
        let length = index - start;
        if marker == '~' && length > 2 {
            pieces.extend(characters[start..index].iter().copied().map(Piece::Text));
            continue;
        }
        let before = start.checked_sub(1).map(|before| characters[before]);
        let after = characters.get(index).copied();
        // A left-flanking delimiter run is not followed by Unicode whitespace, and either not followed by punctuation or preceded by whitespace or punctuation.
        let left = !whitespace(after)
            && (!punctuation(after) || whitespace(before) || punctuation(before));
        let right = !whitespace(before)
            && (!punctuation(before) || whitespace(after) || punctuation(after));
        let (can_open, can_close) = if marker == '_' {
            (
                left && (!right || punctuation(before)),
                right && (!left || punctuation(after)),
            )
        } else {
            (left, right)
        };
        pieces.push(Piece::Run(runs.len()));
        runs.push(Run {
            marker,
            length,
            remaining: length,
            can_open,
            can_close,
            opening_tags: Vec::new(),
            closing_tags: Vec::new(),
        });
    }

    // The delimiter stack, searched without the `openers_bottom` shortcut, which does not change the result.
    let mut stack: Vec<usize> = (0..runs.len()).collect();
    let mut position = 0;
    while position < stack.len() {
        let closer = stack[position];
        if !runs[closer].can_close {
            position += 1;
            continue;
        }
        let opener = (0..position).rev().find(|&candidate| {
            let opener = &runs[stack[candidate]];
            let closer = &runs[closer];
            opener.marker == closer.marker
                && opener.can_open
                && if closer.marker == '~' {
                    opener.remaining == closer.remaining
                } else {
                    // The rule of 3 uses the lengths of the original delimiter runs.
                    !((opener.can_close || closer.can_open)
                        && (opener.length + closer.length) % 3 == 0
                        && !(opener.length % 3 == 0 && closer.length % 3 == 0))
                }
        });
        let Some(opener_position) = opener else {
            if runs[closer].can_open {
                position += 1;
            } else {
                stack.remove(position);
            }
            continue;
        };
        let opener = stack[opener_position];
        let (tag, take) = if runs[closer].marker == '~' {
            ("del", runs[closer].remaining)
        } else if runs[opener].remaining >= 2 && runs[closer].remaining >= 2 {
            ("strong", 2)
        } else {
            ("em", 1)
        };
        runs[opener].remaining -= take;
        runs[closer].remaining -= take;
        runs[opener].opening_tags.insert(0, tag);
        runs[closer].closing_tags.push(tag);
        stack.drain(opener_position + 1..position);
        position = opener_position + 1;
        if runs[opener].remaining == 0 {
            stack.remove(opener_position);
            position -= 1;
        }
        if runs[closer].remaining == 0 {
            stack.remove(position);
        }
    }

    let mut html = String::from("<p>");
    for piece in pieces {
        match piece {
            Piece::Text(character) => html.push(character),
            Piece::Run(run) => {
                let run = &runs[run];
                for tag in &run.closing_tags {
                    html.push_str(&format!("</{tag}>"));
                }
                html.extend(std::iter::repeat(run.marker).take(run.remaining));
                for tag in &run.opening_tags {
                    html.push_str(&format!("<{tag}>"));
                }
            }
        }
    }
    html.push_str("</p>");
    html
}

// Deliberate divergence from micromark in the first and last cases: it treats the `*` between `~~` and `b` as able to close, so `*b*` fails the rule of 3 against the final run. That `*` is preceded by punctuation and followed by a letter, so it is not right-flanking and `*b*` is emphasis. micromark expects `<p>.<em><strong><strong><del>a</del>*b</strong></strong></em></p>` and `<p><em><strong><strong><del>a</del>*b</strong></strong></em></p>`.
#[test]
fn rule_of_three_uses_original_run_lengths_next_to_tildes() {
    for (source, expected) in [
        (
            ".*****~~a~~*b*****",
            "<p>.*<strong><strong><del>a</del><em>b</em></strong></strong></p>",
        ),
        (
            ".*****~~a~~_b_*****",
            "<p>.<em><strong><strong><del>a</del><em>b</em></strong></strong></em></p>",
        ),
        (
            ".*****~~a~~*****",
            "<p>.<em><strong><strong><del>a</del></strong></strong></em></p>",
        ),
        (
            "*****~~a~~*b*****",
            "<p>*<strong><strong><del>a</del><em>b</em></strong></strong></p>",
        ),
    ] {
        assert_eq!(reference(source), expected, "reference for {source:?}");
        assert_eq!(
            to_html_with_options(source, &Options::gfm()),
            expected,
            "{source:?}"
        );
    }
}

#[test]
fn emphasis_runs_around_strikethrough_match_the_reference() {
    let markers = ['*', '_'];
    let mut runs = Vec::new();
    for marker in markers {
        for length in 1..=6 {
            runs.push(marker.to_string().repeat(length));
        }
    }
    for middle in ["~~x~~", "~x~", "~~x~~*b*", "~~x~~_b_", "x~~y~~", "~x~y"] {
        for before in ["", "a", ".", "a "] {
            for after in ["", "b", ".", " b"] {
                for opening in &runs {
                    for closing in &runs {
                        let source = format!("{before}{opening}{middle}{closing}{after}");
                        assert_eq!(
                            to_html_with_options(&source, &Options::gfm()),
                            reference(&source),
                            "{source:?}"
                        );
                    }
                }
            }
        }
    }
}
