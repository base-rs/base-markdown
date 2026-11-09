// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Attention sequence selection.
//!
//! Adjacent emphasis, strong or strikethrough of the same kind is written as one node, as its sequences would otherwise join into one run. Each other attention node chooses its sequence from the characters immediately around it, in constant time, so that its sequences open and close: a marker that would join a neighbouring run is avoided, and characters that would keep a sequence from flanking are encoded as character references, as `encodeInfo` in `mdast-util-to-markdown` does. Where neither marker works locally, the configured marker is used.
//!
//! Which runs pair also depends on the whole block, through the rule of 3 and runs that can both open and close (#110, #111). That is not predicted here: a block with nested or adjacent attention is read back, and where it reads differently, the writer tries the variants of `vary` until it reads back, or the block has no spelling.
use super::util::safe::Literal;
use crate::util::char::{classify, Kind};
use alloc::string::String;

/// Sequences for emphasis, strong and strikethrough, in order of preference.
pub(super) static EMPHASIS_ASTERISK: &[&str] = &["*", "_"];
pub(super) static EMPHASIS_UNDERSCORE: &[&str] = &["_", "*"];
pub(super) static STRONG_ASTERISK: &[&str] = &["**", "__"];
pub(super) static STRONG_UNDERSCORE: &[&str] = &["__", "**"];
pub(super) static STRIKETHROUGH: &[&str] = &["~~"];
/// Where `ParseOptions::strikethrough_single_tilde` reads one tilde and the text holds a run of
/// two tildes, which no escape keeps from closing two, and none of one.
pub(super) static STRIKETHROUGH_SINGLE: &[&str] = &["~", "~~"];

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
    /// The last character of the content, or `None` if it is empty.
    pub last: Option<char>,
    /// The character after the closing sequence; `None` is the edge of a block.
    pub after: Option<char>,
    /// Whether `after` starts the closing sequence of enclosing attention.
    pub after_closes: bool,
    /// Whether `first` and `after` start the language of code (#185): a language is written as it
    /// is, so the character cannot be encoded, and `_` next to it would join the run the parser
    /// reads the language from.
    pub first_language: bool,
    /// See `first_language`.
    pub after_language: bool,
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
/// An opening sequence that joins the closing sequence of attention before it does not form, and attention after it chooses its sequence to fit this one. One that joins the opening sequence of enclosing attention forms one run with it, which splits as intended unless the closing sequences join too, where only strong splits: `****a****` is strong in strong, but `**a**` is strong rather than emphasis in emphasis. Whether whole runs pair is left to reading the block back.
///
/// The choice is `None` where a language next to the attention rules out every sequence, such as
/// emphasis that ends in punctuation before a language, which `_` would join and `*` cannot close
/// (#185).
pub(super) fn choose(
    sequences: &'static [&'static str],
    surroundings: &Surroundings,
) -> Option<Choice> {
    let mut fallback = None;
    let mut languages = 0;
    for &sequence in sequences {
        let marker = sequence.chars().next().expect("sequences are not empty");
        let choice = encoding(sequence, marker, surroundings);
        if (marker == '_' || choice.encode_first) && surroundings.first_language
            || (marker == '_' || choice.encode_after) && surroundings.after_language
        {
            languages += 1;
            continue;
        }
        let joins_before = surroundings.before == Some(marker);
        let joins_after = surroundings.after_closes && surroundings.after == Some(marker);
        let forms = if surroundings.before_opens && joins_before {
            !joins_after || sequence.len() == 2
        } else {
            !(surroundings.before_closes && joins_before)
        };
        if !forms {
            continue;
        }
        if !(choice.encode_before
            || choice.encode_first
            || choice.encode_last
            || choice.encode_after)
        {
            return Some(choice);
        }
        fallback.get_or_insert(choice);
    }
    // Where a language rules out every sequence, none forms. Otherwise the preferred one is
    // written, and the block is read back.
    fallback.or_else(|| {
        let sequence = sequences[0];
        (languages < sequences.len()).then(|| {
            encoding(
                sequence,
                sequence.chars().next().expect("sequences are not empty"),
                surroundings,
            )
        })
    })
}

/// How many variants `vary` has, the first being the choice itself.
pub(super) const VARIANTS: u8 = 8;

/// A variant of `choice`, for a block whose attention does not read back as chosen: variant 0
/// is the choice itself, and the bits of others add, in order, the other marker, encoding the
/// characters outside the sequences, and encoding those inside. Punctuation is never encoded,
/// as a reference is punctuation too, nor a language, which is written as it is (#185), and
/// `_` is not chosen next to one.
pub(super) fn vary(
    choice: Choice,
    variant: u8,
    sequences: &'static [&'static str],
    surroundings: &Surroundings,
) -> Choice {
    let mut choice = match sequences
        .iter()
        .find(|&&sequence| sequence != choice.sequence)
        .filter(|_| variant & 1 != 0)
    {
        Some(&other) => {
            let marker = other.chars().next().expect("sequences are not empty");
            let other_choice = encoding(other, marker, surroundings);
            let language = (marker == '_' || other_choice.encode_first)
                && surroundings.first_language
                || (marker == '_' || other_choice.encode_after) && surroundings.after_language;
            if language {
                choice
            } else {
                other_choice
            }
        }
        None => choice,
    };
    let encodable = |character: Option<char>| {
        character.is_some_and(|character| classify(character) != Kind::Punctuation)
    };
    if variant & 2 != 0 {
        choice.encode_before |= encodable(surroundings.before);
        choice.encode_after |= encodable(surroundings.after) && !surroundings.after_language;
    }
    if variant & 4 != 0 {
        choice.encode_first |= encodable(surroundings.first) && !surroundings.first_language;
        choice.encode_last |= encodable(surroundings.last);
    }
    choice
}

/// Classify a character inside attention, where empty content, which a normalized tree does not have, counts as a letter, as in JS.
fn inside(character: Option<char>) -> Kind {
    character.map_or(Kind::Other, classify)
}

pub(super) fn encoding(
    sequence: &'static str,
    marker: char,
    surroundings: &Surroundings,
) -> Choice {
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
/// JS equivalent: `encodeInfo` in `lib/util/encode-info.js`, without its cases for whitespace inside, which a normalized tree does not have: [`crate::Document::normalize`] moves it out. Punctuation and whitespace are never encoded.
fn encode_info(outside: Kind, inside: Kind, marker: char) -> (bool, bool) {
    match (outside, inside) {
        // Underscores do not form between letters; asterisks and tildes do.
        (Kind::Other, Kind::Other) => (marker == '_', marker == '_'),
        (Kind::Other, Kind::Punctuation) => (false, true),
        _ => (false, false),
    }
}

/// Classify a character, where `None` is the edge of a block, which counts as whitespace.
fn kind(character: Option<char>) -> Kind {
    character.map_or(Kind::Whitespace, classify)
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

/// Write `character` as a character reference, or as it is, returning `false`, where the options
/// decode none.
fn write_reference(output: &mut String, character: char, literal: Literal) -> bool {
    literal.write(output, character, false)
}

/// Write `run` copies of `character` so that each reads back as text, returning `false` where the
/// options read no escape.
fn write_run(output: &mut String, character: char, run: usize, literal: Literal) -> bool {
    let mut written = true;
    for _ in 0..run {
        written &= literal.write(output, character, true);
    }
    written
}

/// Edits for the start of the text written after an attention sequence.
#[derive(Default)]
pub(super) struct Edge {
    /// The character to encode, if the text starts with it.
    pub encode: Option<char>,
    /// Whether to escape tildes the text starts with.
    pub tilde: bool,
}

/// Apply `edge` to text written at `output[start..]`, returning `false` where the options read
/// no spelling that the edit needs.
///
/// Underscores between letters are written unescaped, but next to a reference they can form attention, so they are escaped here.
pub(super) fn edit_start(output: &mut String, start: usize, edge: &Edge, literal: Literal) -> bool {
    let text = &output[start..];
    let mut result = String::new();
    let written;
    if edge.tilde && text.starts_with('~') {
        let run = text.bytes().take_while(|&byte| byte == b'~').count();
        written = write_run(&mut result, '~', run, literal);
        result.push_str(&text[run..]);
    } else if let Some(character) = edge.encode.filter(|&character| text.starts_with(character)) {
        let encoded = write_reference(&mut result, character, literal);
        let rest = &text[character.len_utf8()..];
        let run = rest.bytes().take_while(|&byte| byte == b'_').count();
        written = write_run(&mut result, '_', run, literal) && encoded;
        result.push_str(&rest[run..]);
    } else {
        return true;
    }
    output.truncate(start);
    output.push_str(&result);
    written
}

/// Escape tildes at the end of text written at `output[start..]`, as a sequence follows it,
/// returning `false` where the options read no escape.
///
/// The whole run is escaped, as part of a run would still pair.
pub(super) fn escape_tilde_end(output: &mut String, start: usize, literal: Literal) -> bool {
    let text = &output[start..];
    let run = text.bytes().rev().take_while(|&byte| byte == b'~').count();
    let before = &text[..text.len() - run];
    // An odd number of backslashes escapes the first tilde, which ends the run.
    let run = if literal.escapes_next(before) {
        run.saturating_sub(1)
    } else {
        run
    };
    if run == 0 {
        return true;
    }
    output.truncate(output.len() - run);
    write_run(output, '~', run, literal)
}

/// Encode the last character of text written at `output[start..]` if it is `character`, as a
/// sequence follows it, returning `false` where the options read no spelling the edit needs.
///
/// It is not if it was encoded already, as the first character of the same text.
pub(super) fn encode_end(
    output: &mut String,
    start: usize,
    character: char,
    literal: Literal,
) -> bool {
    let text = &output[start..];
    let Some(rest) = text.strip_suffix(character) else {
        return true;
    };
    let run = rest.bytes().rev().take_while(|&byte| byte == b'_').count();
    let before = &rest[..rest.len() - run];
    // Escaped underscores alternate with backslashes, so an escaped one ends any run.
    // An underscore can also be what keeps a literal `www.` or `http://` domain from linking.
    let run = if literal.escapes_next(before) || literal_domain(before) {
        0
    } else {
        run
    };
    // An odd number of backslashes would escape the `&`.
    let backslash = run == 0 && literal.escapes_next(rest);
    output.truncate(output.len() - character.len_utf8() - run);
    if backslash {
        output.push('\\');
    }
    let escaped = write_run(output, '_', run, literal);
    write_reference(output, character, literal) && escaped
}
