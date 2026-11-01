// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Attention sequence selection.
//!
//! Adjacent emphasis, strong or strikethrough of the same kind is written as one node, as its sequences would otherwise join into one run. Each other attention node chooses its sequence from the characters immediately around it, in constant time, so that its sequences open and close: a marker that would join a neighbouring run is avoided, and characters that would keep a sequence from flanking are encoded as character references, as `encodeInfo` in `mdast-util-to-markdown` does. Where neither marker works locally, the configured marker is used.
use crate::util::char::{classify, Kind};
use alloc::string::String;
use core::fmt::Write;

/// Sequences for emphasis, strong and strikethrough, in order of preference.
pub(super) static EMPHASIS_ASTERISK: &[&str] = &["*", "_"];
pub(super) static EMPHASIS_UNDERSCORE: &[&str] = &["_", "*"];
pub(super) static STRONG_ASTERISK: &[&str] = &["**", "__"];
pub(super) static STRONG_UNDERSCORE: &[&str] = &["__", "**"];
pub(super) static STRIKETHROUGH: &[&str] = &["~~"];

/// The characters around attention, as far as they are known before it is written.
pub(super) struct Surroundings {
    /// The character before the opening sequence; `None` is the edge of a block.
    pub before: Option<char>,
    /// Whether `before` ends the closing sequence of attention before this one.
    pub before_closes: bool,
    /// Whether `before` ends the opening sequence of enclosing attention.
    pub before_opens: bool,
    /// The first character of the content, or `None` if it is empty.
    pub first: Option<char>,
    /// The sequence size of emphasis (1) or strong (2) the content starts with, which may join the opening sequence, or 0.
    pub first_size: usize,
    /// The last character of the content, or `None` if it is empty.
    pub last: Option<char>,
    /// The sequence size of emphasis (1) or strong (2) the content ends with, which may join the closing sequence, or 0.
    pub last_size: usize,
    /// The character after the closing sequence; `None` is the edge of a block.
    pub after: Option<char>,
    /// Whether `after` starts the closing sequence of enclosing attention.
    pub after_closes: bool,
    /// The attention directly enclosing this one.
    pub parent: Parent,
    /// Whether enclosing attention is open, by marker (`*`, `_`) and sequence size (1, 2).
    pub enclosing: [[bool; 2]; 2],
}

/// The runs of markers that attention is written with, as far as attention directly inside it needs.
#[derive(Clone, Copy)]
pub(super) struct Parent {
    /// The size of the run of markers its opening sequence ends, including enclosing opening sequences it joins.
    pub opening_run: usize,
    /// The size of the run of markers its closing sequence starts, including enclosing closing sequences it joins.
    pub closing_run: usize,
    /// The kind of the character before its opening run.
    pub before: Kind,
    /// The kind of the character after its closing run.
    pub after: Kind,
    /// Whether its opening run can both open and close, if no child joins it.
    pub opening_both: bool,
    /// Whether its closing run can both open and close, if no child joins it.
    pub closing_both: bool,
    /// Whether no child can join its closing run.
    pub closing_final: bool,
    /// Whether its opening sequence joins the opening run of its own parent.
    pub joins_opening: bool,
}

impl Parent {
    /// No enclosing attention.
    pub(super) const NONE: Self = Self {
        opening_run: 0,
        closing_run: 0,
        before: Kind::Whitespace,
        after: Kind::Whitespace,
        opening_both: false,
        closing_both: false,
        closing_final: true,
        joins_opening: false,
    };

    /// The parent that attention written as `choice` in `surroundings` is to its children.
    pub(super) fn new(choice: &Choice, surroundings: &Surroundings) -> Self {
        let size = choice.sequence.len();
        let marker = choice
            .sequence
            .chars()
            .next()
            .expect("sequences are not empty");
        let parent = &surroundings.parent;
        let joins_opening = surroundings.before_opens && surroundings.before == Some(marker);
        let (opening_run, before) = if joins_opening {
            (parent.opening_run + size, parent.before)
        } else {
            (
                size,
                written(kind(surroundings.before), choice.encode_before),
            )
        };
        let (closing_run, after) =
            if surroundings.after_closes && surroundings.after == Some(marker) {
                (parent.closing_run + size, parent.after)
            } else {
                (size, written(kind(surroundings.after), choice.encode_after))
            };
        let first = written(inside(surroundings.first), choice.encode_first);
        let last = written(inside(surroundings.last), choice.encode_last);
        Self {
            opening_run,
            closing_run,
            before,
            after,
            opening_both: both(flanking(marker, before, first)),
            closing_both: both(flanking(marker, last, after)),
            closing_final: surroundings.last_size == 0,
            joins_opening,
        }
    }

    /// Extend the opening run by that of a child, once written, if the child joins it.
    pub(super) fn extend(&mut self, child: &Self) {
        if child.joins_opening {
            self.opening_run = child.opening_run;
            self.opening_both = child.opening_both;
        }
    }
}

/// The chosen sequence and the characters around it to encode.
pub(super) struct Choice {
    pub sequence: &'static str,
    pub encode_before: bool,
    pub encode_first: bool,
    pub encode_last: bool,
    pub encode_after: bool,
}

/// Choose a sequence that forms: the first that needs nothing encoded, then the first that needs encoding, then the preferred one.
///
/// An opening sequence that joins the closing sequence of attention before it does not form, and attention after it chooses its sequence to fit this one. One that joins the opening sequence of enclosing attention forms one run with it, which splits as intended unless the closing sequences join too, where only strong splits: `****a****` is strong in strong, but `**a**` is strong rather than emphasis in emphasis. Any other opening sequence must not be able to close enclosing attention.
///
/// Where a sequence joins a run of enclosing attention at one edge only, the rule of 3, which compares the lengths of whole runs, must still let the runs at both edges pair: `**a*.**` is emphasis in emphasis, but written as `**a*.*\*` its outer closing sequence can also open, and cannot close the run of 2, so it is written as `*_a_.*\*`. Attention the content starts or ends with may join the same run, so the check is skipped where that would change the run.
pub(super) fn choose(sequences: &'static [&'static str], surroundings: &Surroundings) -> Choice {
    let mut fallback = None;
    for &sequence in sequences {
        let marker = sequence.chars().next().expect("sequences are not empty");
        let choice = encoding(sequence, marker, surroundings);
        let joins_before = surroundings.before == Some(marker);
        let joins_after = surroundings.after_closes && surroundings.after == Some(marker);
        let forms = if surroundings.before_opens && joins_before {
            if joins_after {
                sequence.len() == 2
            } else {
                !closing_unpaired(&choice, marker, surroundings)
            }
        } else {
            !(surroundings.before_closes && joins_before)
                && !closes_enclosing(&choice, marker, surroundings)
                && !(joins_after && opening_unpaired(&choice, marker, surroundings))
        };
        if !forms {
            continue;
        }
        if !(choice.encode_before
            || choice.encode_first
            || choice.encode_last
            || choice.encode_after)
        {
            return choice;
        }
        fallback.get_or_insert(choice);
    }
    fallback.unwrap_or_else(|| {
        let sequence = sequences[0];
        encoding(
            sequence,
            sequence.chars().next().expect("sequences are not empty"),
            surroundings,
        )
    })
}

/// Whether a delimiter run between `before` and `after` can open and close, as CommonMark defines it.
fn flanking(marker: char, before: Kind, after: Kind) -> (bool, bool) {
    let left = after != Kind::Whitespace && !(after == Kind::Punctuation && before == Kind::Other);
    let right =
        before != Kind::Whitespace && !(before == Kind::Punctuation && after == Kind::Other);
    if marker == '_' {
        (
            left && (!right || before == Kind::Punctuation),
            right && (!left || after == Kind::Punctuation),
        )
    } else {
        (left, right)
    }
}

/// The kind of a character next to a sequence, where an encoded character is written as a reference, which starts with `&` and ends with `;`.
fn written(kind: Kind, encoded: bool) -> Kind {
    if encoded {
        Kind::Punctuation
    } else {
        kind
    }
}

/// Whether an opening sequence could also close, and so close enclosing attention of its marker.
///
/// The rule of 3 keeps a sequence of one marker from closing one of two, and two from closing one.
fn closes_enclosing(choice: &Choice, marker: char, surroundings: &Surroundings) -> bool {
    if marker == '~'
        || !surroundings.enclosing[usize::from(marker == '_')][choice.sequence.len() - 1]
    {
        return false;
    }
    opening_flanking(choice, marker, surroundings).1
}

/// Whether the rule of 3 keeps the closing sequence, or the parent's closing run, from pairing with the parent's opening run, which the opening sequence joins.
///
/// Attention the content starts with may join the run too, which makes the character after the run unknown, and attention it ends with may join the closing sequence, which makes the size of its run unknown.
fn closing_unpaired(choice: &Choice, marker: char, surroundings: &Surroundings) -> bool {
    let parent = &surroundings.parent;
    let size = choice.sequence.len();
    let run = parent.opening_run + size + surroundings.first_size;
    let first = written(inside(surroundings.first), choice.encode_first);
    let opening_both = surroundings.first_size == 0 && both(flanking(marker, parent.before, first));
    let last = written(inside(surroundings.last), choice.encode_last);
    let after = written(kind(surroundings.after), choice.encode_after);
    let closing_both = both(flanking(marker, last, after));
    rule_of_three(
        run,
        size,
        closing_both || (opening_both && surroundings.last_size == 0),
    ) || (surroundings.first_size == 0
        && parent.closing_final
        && rule_of_three(run, parent.closing_run, opening_both || parent.closing_both))
}

/// Whether the rule of 3 keeps the opening sequence, or the parent's opening run, from pairing with the parent's closing run, which the closing sequence joins.
///
/// Attention the content starts or ends with may join the opening sequence or the run too, which makes their sizes unknown.
fn opening_unpaired(choice: &Choice, marker: char, surroundings: &Surroundings) -> bool {
    let parent = &surroundings.parent;
    if surroundings.last_size != 0 {
        return false;
    }
    let size = choice.sequence.len();
    let run = size + parent.closing_run;
    let last = written(inside(surroundings.last), choice.encode_last);
    let closing_both = both(flanking(marker, last, parent.after));
    let opening_both = both(opening_flanking(choice, marker, surroundings));
    (surroundings.first_size == 0 && rule_of_three(size, run, opening_both || closing_both))
        || rule_of_three(parent.opening_run, run, parent.opening_both || closing_both)
}

/// Whether the rule of 3 keeps an opening and a closing run of these sizes from pairing, where it `applies` as one of them can both open and close.
fn rule_of_three(opening: usize, closing: usize, applies: bool) -> bool {
    applies && (opening + closing) % 3 == 0 && !(opening % 3 == 0 && closing % 3 == 0)
}

/// Whether an opening sequence can open and close.
fn opening_flanking(choice: &Choice, marker: char, surroundings: &Surroundings) -> (bool, bool) {
    let before = written(kind(surroundings.before), choice.encode_before);
    let first = written(inside(surroundings.first), choice.encode_first);
    flanking(marker, before, first)
}

fn both((open, close): (bool, bool)) -> bool {
    open && close
}

/// Classify a character inside attention, where empty content, which a normalized tree does not have, counts as a letter, as in JS.
fn inside(character: Option<char>) -> Kind {
    character.map_or(Kind::Other, classify)
}

fn encoding(sequence: &'static str, marker: char, surroundings: &Surroundings) -> Choice {
    let (encode_first, encode_before) = encode_info(
        kind(surroundings.before),
        inside(surroundings.first),
        marker,
    );
    let (encode_last, encode_after) =
        encode_info(kind(surroundings.after), inside(surroundings.last), marker);
    Choice {
        sequence,
        encode_before,
        encode_first,
        encode_last,
        encode_after,
    }
}

/// Whether to encode the characters inside and outside an attention sequence.
///
/// JS equivalent: `encodeInfo` in `lib/util/encode-info.js`. Punctuation is never encoded, and neither is whitespace outside, which JS encodes next to whitespace inside: once the inside is a reference, the sequence flanks without it.
fn encode_info(outside: Kind, inside: Kind, marker: char) -> (bool, bool) {
    match (outside, inside) {
        // Underscores do not form between letters; asterisks and tildes do.
        (Kind::Other, Kind::Other) => (marker == '_', marker == '_'),
        (Kind::Other, Kind::Whitespace) => (true, true),
        (Kind::Other, Kind::Punctuation) => (false, true),
        (Kind::Punctuation | Kind::Whitespace, Kind::Whitespace) => (true, false),
        _ => (false, false),
    }
}

/// Classify a character, where `None` is the edge of a block, which counts as whitespace.
fn kind(character: Option<char>) -> Kind {
    character.map_or(Kind::Whitespace, classify)
}

/// Whether `text` ends with an odd number of backslashes, which escape the next character.
pub(super) fn escapes_next(text: &str) -> bool {
    text.bytes().rev().take_while(|&byte| byte == b'\\').count() % 2 == 1
}

/// Whether `text` ends with a domain that a literal `www.`, `http://` or `https://` would turn into a link.
fn literal_domain(text: &str) -> bool {
    let domain = text
        .bytes()
        .rev()
        .take_while(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_'))
        .count();
    let (prefix, domain) = text.split_at(text.len() - domain);
    domain
        .get(..4)
        .is_some_and(|start| start.eq_ignore_ascii_case("www."))
        || prefix.ends_with("http://")
        || prefix.ends_with("https://")
}

fn write_reference(output: &mut String, character: char) {
    write!(output, "&#x{:X};", u32::from(character)).expect("String writing is infallible");
}

/// Edits for the start of the text written after an attention sequence.
#[derive(Default)]
pub(super) struct Edge {
    /// The character to encode, if the text starts with it.
    pub encode: Option<char>,
    /// Whether to escape tildes the text starts with.
    pub tilde: bool,
}

/// Apply `edge` to text written at `output[start..]`.
///
/// Underscores between letters are written unescaped, but next to a reference they can form attention, so they are escaped here.
pub(super) fn edit_start(output: &mut String, start: usize, edge: &Edge) {
    let text = &output[start..];
    let mut result = String::new();
    if edge.tilde && text.starts_with('~') {
        let run = text.bytes().take_while(|&byte| byte == b'~').count();
        result.extend(core::iter::repeat("\\~").take(run));
        result.push_str(&text[run..]);
    } else if let Some(character) = edge.encode.filter(|&character| text.starts_with(character)) {
        write_reference(&mut result, character);
        let rest = &text[character.len_utf8()..];
        let run = rest.bytes().take_while(|&byte| byte == b'_').count();
        result.extend(core::iter::repeat("\\_").take(run));
        result.push_str(&rest[run..]);
    } else {
        return;
    }
    output.truncate(start);
    output.push_str(&result);
}

/// Escape tildes at the end of text written at `output[start..]`, as a sequence follows it.
///
/// The whole run is escaped, as part of a run would still pair.
pub(super) fn escape_tilde_end(output: &mut String, start: usize) {
    let text = &output[start..];
    let run = text.bytes().rev().take_while(|&byte| byte == b'~').count();
    let before = &text[..text.len() - run];
    // An odd number of backslashes escapes the first tilde, which ends the run.
    let run = if escapes_next(before) {
        run.saturating_sub(1)
    } else {
        run
    };
    if run == 0 {
        return;
    }
    output.truncate(output.len() - run);
    output.extend(core::iter::repeat("\\~").take(run));
}

/// Encode the last character of text written at `output[start..]` if it is `character`, as a sequence follows it.
///
/// It is not if it was encoded already, as the first character of the same text.
pub(super) fn encode_end(output: &mut String, start: usize, character: char) {
    let text = &output[start..];
    let Some(rest) = text.strip_suffix(character) else {
        return;
    };
    let run = rest.bytes().rev().take_while(|&byte| byte == b'_').count();
    let before = &rest[..rest.len() - run];
    // Escaped underscores alternate with backslashes, so an escaped one ends any run.
    // An underscore can also be what keeps a literal `www.` or `http://` domain from linking.
    let run = if escapes_next(before) || literal_domain(before) {
        0
    } else {
        run
    };
    // An odd number of backslashes would escape the `&`.
    let backslash = run == 0 && escapes_next(rest);
    output.truncate(output.len() - character.len_utf8() - run);
    if backslash {
        output.push('\\');
    }
    output.extend(core::iter::repeat("\\_").take(run));
    write_reference(output, character);
}
