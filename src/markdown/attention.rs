// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Attention sequence selection.
//!
//! Adjacent emphasis, strong or strikethrough of the same kind is written as one node, as its sequences would otherwise join into one run. Each other attention node chooses its sequence from the characters immediately around it, in constant time, so that its sequences open and close: a marker that would join a neighbouring run is avoided, and characters that would keep a sequence from flanking are encoded as character references, as `encodeInfo` in `mdast-util-to-markdown` does. Where neither marker works locally, the configured marker is used. Nested attention can encode a neighbour after a sequence was chosen, so the serializer checks each sequence again against the characters actually written, with `flanks`.
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
    /// The size of the run of markers that `before` ends, if `before_opens`.
    pub opening_run: usize,
    /// The first character of the content, or `None` if it is empty.
    pub first: Option<char>,
    /// The sequence size of emphasis (1) or strong (2) the content starts with, which may join the opening sequence, or 0.
    pub first_size: usize,
    /// Whether the content ends with emphasis or strong, whose closing sequence may join the closing sequence.
    pub last_attention: bool,
    /// The last character of the content, or `None` if it is empty.
    pub last: Option<char>,
    /// The character after the closing sequence; `None` is the edge of a block.
    pub after: Option<char>,
    /// Whether `after` starts the closing sequence of enclosing attention.
    pub after_closes: bool,
    /// Whether enclosing attention is open, by marker (`*`, `_`) and sequence size (1, 2).
    pub enclosing: [[bool; 2]; 2],
    /// The runs of the attention directly enclosing this one, if any.
    pub enclosing_runs: Option<Runs>,
}

/// The delimiter runs of attention whose content is being written.
#[derive(Clone, Copy)]
pub(super) struct Runs {
    /// The size of the run its opening sequence is part of.
    pub opening: usize,
    /// Whether that run can both open and close, as written.
    pub opening_both: bool,
    /// The size of the run its closing sequence will be part of, including enclosing closing sequences it joins.
    pub closing: usize,
    /// The character after that run; `None` is the edge of a block.
    pub after: Option<char>,
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
pub(super) fn choose(sequences: &'static [&'static str], surroundings: &Surroundings) -> Choice {
    let mut fallback = None;
    for &sequence in sequences {
        let marker = sequence.chars().next().expect("sequences are not empty");
        let choice = encoding(sequence, marker, surroundings);
        let joins_before = surroundings.before == Some(marker);
        let forms = if surroundings.before_opens && joins_before {
            if surroundings.after_closes && surroundings.after == Some(marker) {
                sequence.len() == 2
            } else {
                !closing_unpaired(&choice, marker, surroundings)
            }
        } else {
            !(surroundings.before_closes && joins_before)
                && !closes_enclosing(&choice, marker, surroundings)
                && !closing_blocked(&choice, marker, surroundings)
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

/// Whether a sequence of `marker` written between `before` and `after`, where `None` is the edge of a block, can open and close.
///
/// This checks the characters as written, where the choice of sequence predicts them: characters next to nested attention can be encoded after the choice is made.
pub(super) fn flanks(marker: char, before: Option<char>, after: Option<char>) -> (bool, bool) {
    flanking(marker, kind(before), kind(after))
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
    let before = written(kind(surroundings.before), choice.encode_before);
    let first = written(inside(surroundings.first), choice.encode_first);
    flanking(marker, before, first).1
}

/// Whether the rule of 3 keeps the closing sequence from pairing with the run its opening sequence joins.
///
/// The rule applies if the closing sequence can also open; the sizes count the whole runs.
fn closing_unpaired(choice: &Choice, marker: char, surroundings: &Surroundings) -> bool {
    let size = choice.sequence.len();
    let run = surroundings.opening_run + size + surroundings.first_size;
    // Strong and emphasis sequences are never a multiple of 3 themselves.
    if (run + size) % 3 != 0 {
        return false;
    }
    let last = written(inside(surroundings.last), choice.encode_last);
    let after = written(kind(surroundings.after), choice.encode_after);
    let (open, close) = flanking(marker, last, after);
    open && close
}

/// Whether the rule of 3 keeps the enclosing opening sequence from pairing with what remains of a closing run that this closing sequence joins, where it would pair if this sequence did not join it.
///
/// The joined run pairs with this opening sequence first, then with the enclosing one. That pair fails if one of its runs can both open and close and their sizes add up to a multiple of 3, unless both are multiples of 3. Encoded whitespace before `*` can make an opening run that can also close, as in `&#x9;*<http://a.b>*em**`, so this one is written with `_` instead.
fn closing_blocked(choice: &Choice, marker: char, surroundings: &Surroundings) -> bool {
    let Some(enclosing) = surroundings.enclosing_runs else {
        return false;
    };
    // The closing sequences of emphasis or strong the content ends with may join the run too, which is not known yet.
    if !surroundings.after_closes
        || surroundings.after != Some(marker)
        || surroundings.last_attention
    {
        return false;
    }
    let unpaired = |run: usize, before: Kind| {
        let (open, close) = flanking(marker, before, kind(enclosing.after));
        (enclosing.opening_both || open && close)
            && (enclosing.opening + run) % 3 == 0
            && (enclosing.opening % 3 != 0 || run % 3 != 0)
    };
    let last = written(inside(surroundings.last), choice.encode_last);
    // Alone, the enclosing closing run follows the other marker.
    unpaired(choice.sequence.len() + enclosing.closing, last)
        && !unpaired(enclosing.closing, Kind::Punctuation)
}

/// Classify a character inside attention, where empty attention counts as a letter, as in JS.
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
        encode_first: encode_first && surroundings.first.is_some(),
        encode_last: encode_last && surroundings.last.is_some(),
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
