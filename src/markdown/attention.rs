// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Attention sequence selection.
//!
//! Emphasis, strong and strikethrough are first written with their preferred sequence. Once a phrasing container is complete, its attention is checked the way the parser pairs it; where sequences would pair wrongly, such as `*a**b*` for two adjacent emphasis nodes, other sequences are tried. Characters around attention are encoded as character references where needed so its sequences can open and close.
//!
//! JS equivalent: `serialize`, `render`, `check`, `pair` and `improve` in `lib/util/container-phrasing.js` and `encodeInfo` in `lib/util/encode-info.js` of `mdast-util-to-markdown` 2.2.1.
use crate::util::char::{classify, Kind};
use alloc::{string::String, vec, vec::Vec};
use core::{fmt::Write, ops::Range};

/// Sequences for emphasis, strong and strikethrough, in order of preference.
pub(super) static EMPHASIS_ASTERISK: &[&str] = &["*", "_"];
pub(super) static EMPHASIS_UNDERSCORE: &[&str] = &["_", "*"];
pub(super) static STRONG_ASTERISK: &[&str] = &["**", "__"];
pub(super) static STRONG_UNDERSCORE: &[&str] = &["__", "**"];
pub(super) static STRIKETHROUGH: &[&str] = &["~~", "~"];

/// Attention written into the output with its preferred sequence.
pub(super) struct Record {
    /// Output offset of the opening sequence.
    pub open: usize,
    /// Output offset of the closing sequence.
    pub close: usize,
    /// Sequences that can be used, in order of preference.
    pub sequences: &'static [&'static str],
}

/// Serialized phrasing: text, or attention with its children in the following items.
enum Item {
    Text(Range<usize>),
    Attention { record: usize, end: usize },
}

enum Value {
    Text(Range<usize>),
    Owned(String),
    Sequence(&'static str),
}

struct Token {
    value: Value,
    attention: Option<usize>,
}

struct Mistake {
    attention: Vec<usize>,
    index: usize,
}

struct Run {
    tokens: Range<usize>,
    markers: Vec<usize>,
    start: usize,
    end: usize,
    open: bool,
    close: bool,
    split: bool,
}

struct Solver<'a> {
    text: &'a str,
    records: &'a [Record],
    items: Vec<Item>,
    before: Option<char>,
    after: Option<char>,
}

/// Rewrite `output[start..]`, which contains `records`, so that its attention forms.
///
/// `before` and `after` are the characters around the phrasing; `None` is the edge of a block.
pub(super) fn resolve(
    output: &mut String,
    start: usize,
    records: &[Record],
    before: Option<char>,
    after: Option<char>,
) {
    let mut items = Vec::with_capacity(records.len() * 3);
    let mut next = 0;
    build(records, start, &mut next, start, output.len(), &mut items);
    let solver = Solver {
        text: &output[start..],
        records,
        items,
        before,
        after,
    };
    let Some(tokens) = solver.serialize() else {
        return;
    };
    let mut result = String::with_capacity(output.len() - start + 16);
    for token in &tokens {
        result.push_str(solver.value(token));
    }
    output.truncate(start);
    output.push_str(&result);
}

/// Collect items between `from` and `to` from records in document order.
fn build(
    records: &[Record],
    base: usize,
    next: &mut usize,
    from: usize,
    to: usize,
    items: &mut Vec<Item>,
) {
    let mut cursor = from;
    while let Some(record) = records.get(*next).filter(|record| record.open < to) {
        let size = record.sequences[0].len();
        if record.open > cursor {
            items.push(Item::Text(cursor - base..record.open - base));
        }
        let index = items.len();
        let attention = *next;
        // A placeholder until the end of the children is known.
        items.push(Item::Text(0..0));
        *next += 1;
        build(records, base, next, record.open + size, record.close, items);
        items[index] = Item::Attention {
            record: attention,
            end: items.len(),
        };
        cursor = record.close + size;
    }
    if to > cursor {
        items.push(Item::Text(cursor - base..to - base));
    }
}

/// Whether to encode the characters inside and outside an attention sequence.
///
/// JS equivalent: `encodeInfo` in `lib/util/encode-info.js`. Punctuation is never encoded.
fn encode_info(outside: Kind, inside: Kind, marker: char) -> (bool, bool) {
    match (outside, inside) {
        // Underscores do not form between letters; asterisks and tildes do.
        (Kind::Other, Kind::Other) => (marker == '_', marker == '_'),
        (Kind::Other, Kind::Whitespace) | (Kind::Whitespace, Kind::Whitespace) => (true, true),
        (Kind::Other, Kind::Punctuation) => (false, true),
        (Kind::Punctuation, Kind::Whitespace) => (true, false),
        _ => (false, false),
    }
}

fn write_reference(output: &mut String, character: char) {
    write!(output, "&#x{:X};", u32::from(character)).expect("String writing is infallible");
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

/// Classify a character, where `None` is the edge of a block, which counts as whitespace.
fn kind(character: Option<char>) -> Kind {
    character.map_or(Kind::Whitespace, classify)
}

impl Solver<'_> {
    fn value<'v>(&'v self, token: &'v Token) -> &'v str {
        match &token.value {
            Value::Text(range) => &self.text[range.clone()],
            Value::Owned(value) => value,
            Value::Sequence(value) => value,
        }
    }

    fn sequence(&self, record: usize, chosen: &[usize]) -> &'static str {
        self.records[record].sequences[chosen[record]]
    }

    /// Encode the first or last character of a text token as a character reference.
    ///
    /// Underscores between letters are written unescaped, but next to a reference they can form attention, so they are escaped here. Documents can split text where `mdast-util-to-markdown` would not, so escaping when the text is written cannot see every attention boundary.
    ///
    /// Only text is encoded: sequences next to attention are punctuation, which is never encoded.
    fn encode(&self, token: &mut Token, start: bool) {
        let value = self.value(token);
        let mut result = String::with_capacity(value.len() + 8);
        if start {
            let character = value.chars().next().expect("text tokens are not empty");
            write_reference(&mut result, character);
            let rest = &value[character.len_utf8()..];
            let run = rest.bytes().take_while(|&byte| byte == b'_').count();
            result.extend(core::iter::repeat("\\_").take(run));
            result.push_str(&rest[run..]);
        } else {
            let character = value
                .chars()
                .next_back()
                .expect("text tokens are not empty");
            let rest = &value[..value.len() - character.len_utf8()];
            let run = rest.bytes().rev().take_while(|&byte| byte == b'_').count();
            // Escaped underscores alternate with backslashes, so an escaped one ends any run.
            // An underscore can also be what keeps a literal `www.` or `http://` domain from linking.
            let before = &rest[..rest.len() - run];
            let run = if before
                .bytes()
                .rev()
                .take_while(|&byte| byte == b'\\')
                .count()
                % 2
                == 1
                || literal_domain(before)
            {
                0
            } else {
                run
            };
            let rest = &rest[..rest.len() - run];
            result.push_str(rest);
            // An odd number of backslashes would escape the `&`.
            if run == 0 && rest.bytes().rev().take_while(|&byte| byte == b'\\').count() % 2 == 1 {
                result.push('\\');
            }
            result.extend(core::iter::repeat("\\_").take(run));
            write_reference(&mut result, character);
        }
        token.value = Value::Owned(result);
    }

    /// Escape an unescaped tilde at the start or end of a text token next to an attention sequence.
    ///
    /// Only tildes at the start of a line are otherwise escaped; at the edge of attention, a tilde can pair with one in other text, and next to a tilde sequence it would join its run.
    fn escape_tilde(&self, token: &mut Token, start: bool) {
        if token.attention.is_some() {
            return;
        }
        let value = self.value(token);
        // Escape the whole run, as part of a run would still pair.
        let (before, run, after) = if start {
            let run = value.bytes().take_while(|&byte| byte == b'~').count();
            ("", run, &value[run..])
        } else {
            let run = value.bytes().rev().take_while(|&byte| byte == b'~').count();
            let before = &value[..value.len() - run];
            // An odd number of backslashes escapes the first tilde, which ends the run.
            let run = if before
                .bytes()
                .rev()
                .take_while(|&byte| byte == b'\\')
                .count()
                % 2
                == 1
            {
                run.saturating_sub(1)
            } else {
                run
            };
            (&value[..value.len() - run], run, "")
        };
        if run == 0 {
            return;
        }
        let mut result = String::with_capacity(value.len() + run);
        result.push_str(before);
        result.extend(core::iter::repeat("\\~").take(run));
        result.push_str(after);
        token.value = Value::Owned(result);
    }

    fn first(&self, token: &Token) -> Option<char> {
        self.value(token).chars().next()
    }

    fn last(&self, token: &Token) -> Option<char> {
        self.value(token).chars().next_back()
    }

    /// Render items with the chosen sequences, encoding characters around attention where needed.
    fn render(&self, chosen: &[usize]) -> Vec<Token> {
        let mut tokens = Vec::with_capacity(self.items.len() + self.records.len());
        self.render_level(
            0..self.items.len(),
            chosen,
            self.before,
            self.after,
            &mut tokens,
        );
        tokens
    }

    fn render_level(
        &self,
        items: Range<usize>,
        chosen: &[usize],
        before: Option<char>,
        after: Option<char>,
        tokens: &mut Vec<Token>,
    ) {
        let level = tokens.len();
        let mut encode_after = None;
        let mut escape_tilde_after = false;
        let mut previous = before;
        let mut index = items.start;
        while index < items.end {
            match self.items[index] {
                Item::Text(ref range) => {
                    let mut token = Token {
                        value: Value::Text(range.clone()),
                        attention: None,
                    };
                    // Attention before needed the character after it encoded.
                    if encode_after.is_some() && encode_after == self.first(&token) {
                        self.encode(&mut token, true);
                    }
                    if escape_tilde_after {
                        self.escape_tilde(&mut token, true);
                    }
                    encode_after = None;
                    escape_tilde_after = false;
                    previous = self.last(&token);
                    tokens.push(token);
                    index += 1;
                }
                Item::Attention { record, end } => {
                    let sequence = self.sequence(record, chosen);
                    let marker = sequence.chars().next().expect("sequences are not empty");
                    let outside_before = previous;
                    let outside_after = if end < items.end {
                        match self.items[end] {
                            Item::Text(ref range) => self.text[range.clone()].chars().next(),
                            Item::Attention { record, .. } => {
                                self.sequence(record, chosen).chars().next()
                            }
                        }
                    } else {
                        after
                    };
                    let open = tokens.len();
                    tokens.push(Token {
                        value: Value::Sequence(sequence),
                        attention: Some(record),
                    });
                    self.render_level(index + 1..end, chosen, Some(marker), Some(marker), tokens);
                    let inside = open + 1..tokens.len();
                    // A tilde at the edge of attention could pair with one outside it, and next to a tilde sequence it would join its run.
                    if !inside.is_empty() {
                        self.escape_tilde(&mut tokens[inside.start], true);
                        self.escape_tilde(&mut tokens[inside.end - 1], false);
                    }
                    escape_tilde_after = marker == '~';
                    if escape_tilde_after && open > level {
                        self.escape_tilde(&mut tokens[open - 1], false);
                    }
                    // Empty attention counts as a letter inside, as in JS.
                    let head = if inside.is_empty() {
                        Kind::Other
                    } else {
                        kind(self.first(&tokens[inside.start]))
                    };
                    let (open_inside, open_outside) =
                        encode_info(kind(outside_before), head, marker);
                    if open_inside && !inside.is_empty() {
                        self.encode(&mut tokens[inside.start], true);
                    }
                    let tail = if inside.is_empty() {
                        Kind::Other
                    } else {
                        kind(self.last(&tokens[inside.end - 1]))
                    };
                    let (close_inside, close_outside) =
                        encode_info(kind(outside_after), tail, marker);
                    if close_inside && !inside.is_empty() {
                        self.encode(&mut tokens[inside.end - 1], false);
                    }
                    // Encode the character before, unless it is a line ending or outside this level.
                    if open_outside && open > level && !matches!(outside_before, Some('\n' | '\r'))
                    {
                        self.encode(&mut tokens[open - 1], false);
                    }
                    if close_outside {
                        encode_after = outside_after;
                    }
                    tokens.push(Token {
                        value: Value::Sequence(sequence),
                        attention: Some(record),
                    });
                    previous = sequence.chars().next_back();
                    index = end;
                }
            }
        }
    }

    /// Check whether sequences form their attention, returning the first mistake.
    ///
    /// Sequences of `*` and `_` pair like emphasis and strong in `CommonMark`, others like strikethrough in GFM.
    fn check(&self, tokens: &[Token]) -> Option<Mistake> {
        let mut runs: Vec<Run> = Vec::new();
        let mut wrong: Vec<Option<Vec<usize>>> = (0..tokens.len()).map(|_| None).collect();
        let mut run_of = vec![usize::MAX; tokens.len()];
        let mut index = 0;
        while index < tokens.len() {
            if tokens[index].attention.is_none() {
                index += 1;
                continue;
            }
            let marker = self.first(&tokens[index]);
            let mut end = index + 1;
            while end < tokens.len()
                && tokens[end].attention.is_some()
                && self.first(&tokens[end]) == marker
            {
                end += 1;
            }
            let previous = kind(if index > 0 {
                self.last(&tokens[index - 1])
            } else {
                self.before
            });
            let next = kind(if end < tokens.len() {
                self.first(&tokens[end])
            } else {
                self.after
            });
            let open = next == Kind::Other || next == Kind::Punctuation && previous != Kind::Other;
            let close =
                previous == Kind::Other || previous == Kind::Punctuation && next != Kind::Other;
            // Underscores are strict: they cannot open or close inside words.
            let strict = marker == Some('_');
            let mut run = Run {
                tokens: index..end,
                markers: Vec::new(),
                start: 0,
                end: 0,
                open: open && (!strict || previous != Kind::Other || !close),
                close: close && (!strict || next != Kind::Other || !open),
                // Emphasis and strong runs split; others pair whole runs.
                split: matches!(marker, Some('*' | '_')),
            };
            for token in index..end {
                // Every sequence is wrong until it pairs with its other sequence.
                wrong[token] = Some((index..end).collect());
                run_of[token] = runs.len();
                let size = self.value(&tokens[token]).len();
                run.markers.extend(core::iter::repeat(token).take(size));
            }
            run.end = run.markers.len();
            runs.push(run);
            index = end;
        }
        self.pair(tokens, &mut runs, &mut wrong);
        let index = wrong.iter().position(Option::is_some)?;
        let involved = wrong[index]
            .take()
            .expect("a wrong token has involved tokens");
        Some(Mistake {
            attention: candidates(tokens, &involved, &runs, &run_of, self.records.len()),
            index,
        })
    }

    /// Pair runs like the parser, marking sequences that form their attention as fine.
    ///
    /// A pair forms the intended attention if its markers are exactly the opening and closing sequence of one attention. Otherwise the sequences involved are recorded with each other.
    fn pair(&self, tokens: &[Token], runs: &mut [Run], wrong: &mut [Option<Vec<usize>>]) {
        let mut openers_by_marker: Vec<(Option<char>, Vec<usize>)> = Vec::new();
        for closer in 0..runs.len() {
            let marker = self.first(&tokens[runs[closer].tokens.start]);
            let slot = if let Some(slot) = openers_by_marker
                .iter()
                .position(|(candidate, _)| *candidate == marker)
            {
                slot
            } else {
                openers_by_marker.push((marker, Vec::new()));
                openers_by_marker.len() - 1
            };
            let openers = &mut openers_by_marker[slot].1;
            let mut open = if runs[closer].close { openers.len() } else { 0 };
            while open > 0 && runs[closer].start < runs[closer].end {
                open -= 1;
                let opener = openers[open];
                let opener_size = runs[opener].end - runs[opener].start;
                let closer_size = runs[closer].end - runs[closer].start;
                let skip = if runs[closer].split {
                    // The rule of 3, on the original sizes of the runs as this crate's parser applies it, where `micromark` uses the sizes that are left.
                    let opener_run = runs[opener].markers.len();
                    let closer_run = runs[closer].markers.len();
                    (runs[opener].close || runs[closer].open)
                        && closer_run % 3 != 0
                        && (opener_run + closer_run) % 3 == 0
                } else {
                    // Others pair whole runs of the same size.
                    opener_size != runs[opener].markers.len() || opener_size != closer_size
                };
                if skip {
                    continue;
                }
                let size = if !runs[closer].split {
                    closer_size
                } else if opener_size > 1 && closer_size > 1 {
                    2
                } else {
                    1
                };
                let opening = &runs[opener].markers[runs[opener].end - size..runs[opener].end];
                let closing = &runs[closer].markers[runs[closer].start..runs[closer].start + size];
                let open_token = opening[0];
                let close_token = closing[0];
                // The markers of a sequence are adjacent, so matching ends of one sequence of that size are exactly it.
                if open_token == opening[size - 1]
                    && self.value(&tokens[open_token]).len() == size
                    && close_token == closing[size - 1]
                    && self.value(&tokens[close_token]).len() == size
                    && tokens[open_token].attention == tokens[close_token].attention
                {
                    wrong[open_token] = None;
                    wrong[close_token] = None;
                } else {
                    let mut involved: Vec<usize> = Vec::with_capacity(2 * size);
                    for &token in opening.iter().chain(closing) {
                        if !involved.contains(&token) {
                            involved.push(token);
                        }
                    }
                    // A sequence can be in several pairs.
                    // Sequences that formed their attention were consumed whole, so involved ones are still wrong.
                    for &token in &involved {
                        wrong[token]
                            .as_mut()
                            .expect("involved sequences are wrong")
                            .extend_from_slice(&involved);
                    }
                }
                runs[opener].end -= size;
                runs[closer].start += size;
                if runs[opener].start == runs[opener].end {
                    openers.remove(open);
                }
                // Search again from the start, as this run may close more.
                open = openers.len();
            }
            if runs[closer].open && runs[closer].start < runs[closer].end {
                openers.push(closer);
            }
        }
    }

    /// Try other sequences for the candidates of a mistake, using the first that moves the mistake further.
    fn improve(&self, chosen: &[usize], mistake: &Mistake) -> Option<Improvement> {
        for &attention in &mistake.attention {
            let current = chosen[attention];
            for sequence in 0..self.records[attention].sequences.len() {
                if sequence == current {
                    continue;
                }
                let mut trial = chosen.to_vec();
                trial[attention] = sequence;
                let tokens = self.render(&trial);
                let next = self.check(&tokens);
                if next
                    .as_ref()
                    .map_or(true, |next| next.index > mistake.index)
                {
                    return Some(Improvement {
                        chosen: trial,
                        tokens,
                        mistake: next,
                    });
                }
            }
        }
        None
    }

    /// Choose sequences and render, returning `None` when the output is unchanged.
    fn serialize(&self) -> Option<Vec<Token>> {
        let mut chosen = vec![0; self.records.len()];
        let initial = self.render(&chosen);
        let mut mistake = self.check(&initial);
        let mut tokens = None;
        while let Some(current) = mistake {
            let Some(improvement) = self.improve(&chosen, &current) else {
                // Keep the preferred sequences if other sequences do not help.
                tokens = None;
                break;
            };
            chosen = improvement.chosen;
            tokens = Some(improvement.tokens);
            mistake = improvement.mistake;
        }
        tokens.or_else(|| {
            initial
                .iter()
                .any(|token| matches!(token.value, Value::Owned(_)))
                .then_some(initial)
        })
    }
}

struct Improvement {
    chosen: Vec<usize>,
    tokens: Vec<Token>,
    mistake: Option<Mistake>,
}

/// Attention to try other sequences for: the attention involved and attention in runs with its sequences, later first.
fn candidates(
    tokens: &[Token],
    involved: &[usize],
    runs: &[Run],
    run_of: &[usize],
    records: usize,
) -> Vec<usize> {
    let mut related = vec![false; records];
    for (index, token) in tokens.iter().enumerate() {
        if let Some(attention) = token.attention {
            if involved
                .iter()
                .any(|&other| tokens[other].attention == Some(attention))
            {
                for sibling in runs[run_of[index]].tokens.clone() {
                    let sibling = tokens[sibling].attention.expect("runs contain attention");
                    related[sibling] = true;
                }
            }
        }
    }
    let mut ordered = Vec::new();
    for token in tokens {
        if let Some(attention) = token.attention {
            if core::mem::take(&mut related[attention]) {
                ordered.push(attention);
            }
        }
    }
    // Attention that opens later is inner or next.
    ordered.reverse();
    ordered
}
