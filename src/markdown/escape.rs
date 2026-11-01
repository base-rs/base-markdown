// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Fixed Markdown escaping rules. Scopes and neighbor predicates are static;
//! matching allocates nothing and retains regex-style non-overlapping matches.
use super::{construct_name::ConstructName, util::safe::EscapeInfos};
use alloc::{string::String, vec::Vec};
use core::ops::Range;

const fn bit(name: ConstructName) -> u32 {
    1 << name as u32
}
use ConstructName::*;
const PHRASING: u32 = bit(Phrasing);
const FULL_SPANS: u32 = bit(Autolink)
    | bit(DestinationLiteral)
    | bit(DestinationRaw)
    | bit(Reference)
    | bit(TitleApostrophe)
    | bit(TitleQuote);
const LANGUAGE: u32 = bit(CodeFencedLangGraveAccent) | bit(CodeFencedLangTilde);
const NEWLINES: u32 = LANGUAGE
    | bit(CodeFencedMetaGraveAccent)
    | bit(CodeFencedMetaTilde)
    | bit(DestinationLiteral)
    | bit(HeadingAtx)
    | bit(MathFlowMeta);

#[derive(Clone, Copy)]
enum Before {
    Any,
    Previous(u8),
    Line,
    Digits,
}
#[derive(Clone, Copy)]
enum After {
    Any,
    Set(&'static [u8]),
    LineEnd,
    SpaceOrEnd,
    Entity,
    Html,
    /// A byte that can start an email local part but not HTML, so `<` needs an escape only before a complete email autolink.
    Email,
    Math,
    /// A line ending that ends the context, such as the one written after a paragraph.
    Closing,
}
struct Rule {
    byte: u8,
    before: Before,
    after: After,
    include: u32,
    exclude: u32,
}
// These are static records, not runtime constructors.
macro_rules! rule {
    ($byte:expr, $before:expr, $after:expr, $include:expr, $exclude:expr $(,)?) => {
        Rule {
            byte: $byte,
            before: $before,
            after: $after,
            include: $include,
            exclude: $exclude,
        }
    };
}
use After::{Any as A, Set};
use Before::{Any as B, Digits, Line, Previous};
static RULES: &[Rule] = &[
    rule!(b'\t', B, Set(b"\r\n"), PHRASING, 0),
    rule!(b'\t', Previous(b'\n'), A, PHRASING, 0),
    rule!(b'\t', Previous(b'\r'), A, PHRASING, 0),
    rule!(b'\t', B, A, LANGUAGE, 0),
    rule!(b'\r', B, A, NEWLINES, 0),
    rule!(b'\n', B, A, NEWLINES, 0),
    // Output line endings are LF, so a CR in phrasing would read back as LF.
    rule!(b'\r', B, A, PHRASING, 0),
    // A line ending after another one is a blank line, which ends the paragraph, and one before
    // the line ending that ends a block is trimmed or makes a blank line.
    rule!(b'\n', Previous(b'\n'), A, PHRASING, 0),
    rule!(b'\n', B, After::Closing, PHRASING, 0),
    // A table row is one line.
    rule!(b'\n', B, A, bit(TableCell), 0),
    rule!(b' ', B, Set(b"\r\n"), PHRASING, 0),
    rule!(b' ', Previous(b'\n'), A, PHRASING, 0),
    rule!(b' ', Previous(b'\r'), A, PHRASING, 0),
    rule!(b' ', B, A, LANGUAGE, 0),
    rule!(b'!', B, Set(b"["), PHRASING, FULL_SPANS),
    rule!(b'"', B, A, bit(TitleQuote), 0),
    rule!(b'#', Line, A, 0, 0),
    rule!(b'#', B, After::LineEnd, bit(HeadingAtx), 0),
    rule!(b'&', B, After::Entity, PHRASING, 0),
    rule!(b'\'', B, A, bit(TitleApostrophe), 0),
    rule!(b'(', B, A, bit(DestinationRaw), 0),
    rule!(b'(', Previous(b']'), A, PHRASING, FULL_SPANS),
    rule!(b')', Digits, A, 0, 0),
    rule!(b')', B, A, bit(DestinationRaw), 0),
    rule!(b'*', Line, Set(b" \t\r\n*"), 0, 0),
    rule!(b'*', B, A, PHRASING, FULL_SPANS),
    rule!(b'+', Line, Set(b" \t\r\n"), 0, 0),
    rule!(b'-', Line, Set(b" \t\r\n-"), 0, 0),
    rule!(b'.', Digits, After::SpaceOrEnd, 0, 0),
    rule!(b'<', Line, After::Html, 0, 0),
    rule!(b'<', B, After::Html, PHRASING, FULL_SPANS),
    rule!(b'<', B, After::Email, PHRASING, FULL_SPANS),
    rule!(b'<', B, A, bit(DestinationLiteral), 0),
    rule!(b'=', Line, A, 0, 0),
    rule!(b'>', Line, A, 0, 0),
    rule!(b'>', B, A, bit(DestinationLiteral), 0),
    rule!(b'[', Line, A, 0, 0),
    rule!(b'[', B, A, PHRASING, FULL_SPANS),
    rule!(b'[', B, A, bit(Label) | bit(Reference), 0),
    rule!(b'\\', B, Set(b"\r\n"), PHRASING, 0),
    rule!(b']', B, A, bit(Label) | bit(Reference), 0),
    rule!(b'|', B, A, bit(TableCell), 0),
    rule!(b'_', Line, A, 0, 0),
    rule!(b'_', B, A, PHRASING, FULL_SPANS),
    rule!(b'`', Line, A, 0, 0),
    rule!(
        b'`',
        B,
        A,
        bit(CodeFencedLangGraveAccent) | bit(CodeFencedMetaGraveAccent),
        0,
    ),
    rule!(b'`', B, A, PHRASING, FULL_SPANS),
    rule!(b'~', Line, A, 0, 0),
    rule!(b'$', B, After::Math, PHRASING, 0),
    rule!(b'$', B, A, bit(MathFlowMeta), 0),
    rule!(b'$', Line, Set(b"$"), 0, 0),
];

// A first character outside every escape rule stays literal in any formatting scope.
// CR/LF and ASCII spaces/tabs also stay on the full-render path for unwrap/task trimming.
pub(super) fn literal_prefix(value: &str) -> Option<char> {
    let first = value.chars().next()?;
    (!RULES.iter().any(|rule| char::from(rule.byte) == first)).then_some(first)
}

// Borrow neighboring text instead of assembling a copy of the growing output prefix.
#[derive(Clone, Copy)]
struct Context<'a>([&'a [u8]; 4]);

impl Context<'_> {
    fn get(self, mut position: usize) -> Option<u8> {
        for part in self.0 {
            if position < part.len() {
                return Some(part[position]);
            }
            position -= part.len();
        }
        None
    }

    fn find(self, byte: u8, start: usize) -> Option<usize> {
        let mut offset = 0;
        for part in self.0 {
            let from = start.saturating_sub(offset);
            if from < part.len() {
                if let Some(index) = memchr::memchr(byte, &part[from..]) {
                    return Some(offset + from + index);
                }
            }
            offset += part.len();
        }
        None
    }

    /// The last `byte` in `low..high`.
    fn rfind(self, byte: u8, low: usize, high: usize) -> Option<usize> {
        let mut offset = self.0.iter().map(|part| part.len()).sum::<usize>();
        for part in self.0.iter().rev() {
            offset -= part.len();
            let from = low.saturating_sub(offset).min(part.len());
            let to = high.saturating_sub(offset).min(part.len());
            if from < to {
                if let Some(index) = memchr::memrchr(byte, &part[from..to]) {
                    return Some(offset + from + index);
                }
            }
        }
        None
    }
}

fn scopes(stack: &[ConstructName]) -> u32 {
    stack.iter().fold(0, |mask, name| mask | bit(*name))
}

impl Rule {
    fn active(&self, scopes: u32) -> bool {
        (self.include == 0 || self.include & scopes != 0) && self.exclude & scopes == 0
    }
    fn at(&self, value: Context<'_>, position: usize, single_math: bool) -> Option<Range<usize>> {
        let start = match self.before {
            Before::Any => position,
            Before::Previous(byte) => {
                if position == 0 || value.get(position - 1) != Some(byte) {
                    return None;
                }
                position - 1
            }
            Before::Line | Before::Digits => {
                let mut at = position;
                if matches!(self.before, Before::Digits) {
                    // Markdown ordered-list markers use ASCII digits.
                    while at > 0 && value.get(at - 1).is_some_and(|byte| byte.is_ascii_digit()) {
                        at -= 1;
                    }
                    if at == position {
                        return None;
                    }
                }
                while at > 0 && matches!(value.get(at - 1), Some(b' ' | b'\t')) {
                    at -= 1;
                }
                if at == 0 || !matches!(value.get(at - 1), Some(b'\r' | b'\n')) {
                    return None;
                }
                at - 1
            }
        };
        let next = value.get(position + 1);
        let width = match self.after {
            After::Any => 0,
            After::Math if single_math => 0,
            After::Math => {
                if next == Some(b'$') {
                    1
                } else {
                    return None;
                }
            }
            After::Set(set) => {
                if next.is_some_and(|byte| set.contains(&byte)) {
                    1
                } else {
                    return None;
                }
            }
            After::LineEnd => match next {
                None => 0,
                Some(b'\r' | b'\n') => 1,
                _ => return None,
            },
            After::SpaceOrEnd => match next {
                None => 0,
                Some(b' ' | b'\t' | b'\r' | b'\n') => 1,
                _ => return None,
            },
            After::Entity => {
                if next.is_some_and(|byte| byte == b'#' || byte.is_ascii_alphabetic()) {
                    1
                } else {
                    return None;
                }
            }
            After::Html => {
                if next.is_some_and(|byte| {
                    matches!(byte, b'!' | b'/' | b'?') || byte.is_ascii_alphabetic()
                }) {
                    1
                } else {
                    return None;
                }
            }
            After::Email => {
                if next.is_some_and(|byte| {
                    crate::util::autolink::atext(byte)
                        && !matches!(byte, b'/' | b'?')
                        && !byte.is_ascii_alphabetic()
                }) {
                    1
                } else {
                    return None;
                }
            }
            After::Closing => match (next, value.get(position + 2), value.get(position + 3)) {
                (Some(b'\n' | b'\r'), None, _) | (Some(b'\r'), Some(b'\n'), None) => 0,
                _ => return None,
            },
        };
        Some(start..position + 1 + width)
    }
    fn matches<'a>(
        &'a self,
        value: Context<'a>,
        single_math: bool,
    ) -> impl Iterator<Item = (Range<usize>, usize)> + 'a {
        self.matches_from(value, 0, single_math)
    }
    /// Matches as a scan from the start finds them, from `from` on, where `from` is a point that no
    /// candidate before it overlaps.
    fn matches_from<'a>(
        &'a self,
        value: Context<'a>,
        from: usize,
        single_math: bool,
    ) -> impl Iterator<Item = (Range<usize>, usize)> + 'a {
        let mut search = from;
        let mut minimum = from;
        core::iter::from_fn(move || {
            while let Some(position) = value.find(self.byte, search) {
                if let Some(range) = self
                    .at(value, position, single_math)
                    .filter(|range| range.start >= minimum)
                {
                    search = range.end;
                    minimum = range.end;
                    return Some((range, position));
                }
                search = position + 1;
            }
            None
        })
    }
    /// Where a scan can start and still accept the same matches as a scan from the start, given
    /// the first candidate's `range` at `position`. Matches only consume their neighbours through
    /// overlapping candidates, so this walks back over the overlapping cluster that reaches it.
    /// A candidate ends at most two bytes after its position, so the walk stays within the cluster.
    fn restart(
        &self,
        value: Context<'_>,
        range: Range<usize>,
        position: usize,
        single_math: bool,
    ) -> usize {
        let mut restart = range.start;
        let mut below = position;
        while let Some(candidate) = value.rfind(self.byte, restart.saturating_sub(1), below) {
            below = candidate;
            if let Some(range) = self
                .at(value, candidate, single_math)
                .filter(|range| range.end > restart)
            {
                restart = restart.min(range.start);
            }
        }
        restart
    }
}

/// Collect the escapes `input` needs between `before`, the text before it, and `after`. Positions
/// count from the start of `before`, which is the text a construct written before the output
/// starts with, such as the line ending before a paragraph, and then the output before `input`.
pub(super) fn collect(
    [lead, before]: [&str; 2],
    input: &str,
    after: &str,
    stack: &[ConstructName],
    single_math: bool,
    output: &mut Vec<(usize, EscapeInfos)>,
) {
    let value = Context([
        lead.as_bytes(),
        before.as_bytes(),
        input.as_bytes(),
        after.as_bytes(),
    ]);
    let before = lead.len() + before.len();
    let start = before.saturating_sub(1);
    let end = before + input.len();
    let scopes = scopes(stack);
    for rule in RULES.iter().filter(|rule| rule.active(scopes)) {
        // Only input escapes and the immediately preceding escape can affect output.
        // Find the first match from there, then walk on from a point where the matches
        // before it no longer change which neighbours later matches consume.
        let flags = || EscapeInfos {
            before: !matches!(rule.before, Before::Any),
            // What closes the context is never escaped, so an escape before it always applies.
            after: !matches!(rule.after, After::Any | After::Closing)
                && !(matches!(rule.after, After::Math) && single_math),
            email: matches!(rule.after, After::Email),
        };
        let mut at = start;
        let mut first = None;
        while let Some(position) = value.find(rule.byte, at).filter(|position| *position < end) {
            if let Some(range) = rule.at(value, position, single_math) {
                // An email match consumes `<` and one atext byte, never another `<`, so its
                // matches cannot overlap and need no restart.
                if !matches!(rule.after, After::Email) {
                    first = Some((range, position));
                    break;
                }
                output.push((position, flags()));
            }
            at = position + 1;
        }
        let Some((range, position)) = first else {
            continue;
        };
        let restart = rule.restart(value, range, position, single_math);
        for (_, position) in rule.matches_from(value, restart, single_math) {
            if position >= start && position < end {
                output.push((position, flags()));
            }
        }
    }
}

/// Whether text in `stack` is parsed for strikethrough, which is phrasing outside the spans that
/// are read literally, such as destinations and titles.
pub(super) fn strikethrough(stack: &[ConstructName]) -> bool {
    let scopes = scopes(stack);
    scopes & PHRASING != 0 && scopes & FULL_SPANS == 0
}

/// Add to `applied` the text `~` that could pair as strikethrough, so that none does.
///
/// `input` is written at output position `start`, between `before` and `after`, and `applied`
/// holds the sorted positions that already receive an escape or a character reference. Each
/// complete run of `~` is judged as written, by CommonMark's flanking rules, where an escaped or
/// encoded neighbour is punctuation. Strikethrough pairs runs of one or two tildes of equal length,
/// so a run escapes whole when it can close, or when it has two tildes and can open, as the
/// sequences of strikethrough are two tildes and can also close. A single tilde that can only open
/// is written as it is, since every run that could close it is escaped. A longer run never pairs,
/// unless an escape such as the one at the start of a line splits it, so then it escapes whole.
pub(super) fn tildes(
    input: &str,
    start: usize,
    [before, after]: [Option<char>; 2],
    applied: &mut Vec<usize>,
) {
    use crate::util::char::{classify, Kind};
    let existing = applied.len();
    // Queries rise, so membership needs one pass over the existing positions.
    let contains = |applied: &[usize], cursor: &mut usize, position: usize| {
        while *cursor < existing && applied[*cursor] < position {
            *cursor += 1;
        }
        *cursor < existing && applied[*cursor] == position
    };
    let mut cursor = 0;
    for run in super::util::scan::marker_runs(input.as_bytes(), b'~') {
        let previous = match input[..run.start].chars().next_back() {
            Some(character)
                if contains(
                    applied,
                    &mut cursor,
                    start + run.start - character.len_utf8(),
                ) =>
            {
                Kind::Punctuation
            }
            Some(character) => classify(character),
            None => before.map_or(Kind::Whitespace, classify),
        };
        let first = cursor;
        let split = run
            .clone()
            .any(|at| contains(applied, &mut cursor, start + at));
        let next = match input[run.end..].chars().next() {
            Some(_) if contains(applied, &mut cursor, start + run.end) => Kind::Punctuation,
            Some(character) => classify(character),
            None => after.map_or(Kind::Whitespace, classify),
        };
        let left =
            next != Kind::Whitespace && !(next == Kind::Punctuation && previous == Kind::Other);
        let right =
            previous != Kind::Whitespace && !(previous == Kind::Punctuation && next == Kind::Other);
        let length = run.len();
        if split || (length <= 2 && (right || (length == 2 && left))) {
            let mut cursor = first;
            for at in run {
                if !contains(applied, &mut cursor, start + at) {
                    applied.push(start + at);
                }
            }
        }
    }
    if applied.len() > existing {
        applied.sort_unstable();
    }
}

pub(super) fn unsafe_newline(stack: &[ConstructName]) -> bool {
    scopes(stack) & NEWLINES != 0
}

pub(super) fn normalize_inline(value: &mut String) {
    for rule in RULES
        .iter()
        .filter(|rule| matches!(rule.before, Before::Line | Before::Digits))
    {
        loop {
            let first = rule
                .matches(Context([b"", b"", value.as_bytes(), b""]), false)
                .next();
            let Some((range, _)) = first else {
                break;
            };
            let at = range.start;
            let from = at
                - usize::from(
                    at > 0 && value.as_bytes()[at - 1] == b'\r' && value.as_bytes()[at] == b'\n',
                );
            value.replace_range(from..at + 1, " ");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{format, string::ToString};

    #[test]
    fn borrowed_context_preserves_matches_across_every_split() {
        for value in [
            "",
            "ordinary é α 世界",
            "\r\n \t12. next",
            "\n*\n*\n*\n* ",
            "\r\n-\r\n-\r\n-\r\n",
            "](![x] &amp; <$a> $$ ",
            "\\\r\n\t é\n  # heading\r\n",
            "\n123456) x\r\n",
            "`**__~~$",
            "a$$$$$ $$$$\n$$$",
            "\n1)\n22)\n -\n-\n-",
        ] {
            let boundaries: Vec<_> = (0..=value.len())
                .filter(|&at| value.is_char_boundary(at))
                .collect();
            for &start in &boundaries {
                for &end in boundaries.iter().filter(|&&end| end >= start) {
                    let context = Context([
                        b"",
                        &value.as_bytes()[..start],
                        &value.as_bytes()[start..end],
                        &value.as_bytes()[end..],
                    ]);
                    for single in [false, true] {
                        for rule in RULES {
                            let expected: Vec<_> = rule
                                .matches(Context([b"", b"", value.as_bytes(), b""]), single)
                                .collect();
                            let actual: Vec<_> = rule.matches(context, single).collect();
                            assert_eq!(actual, expected, "{value:?}, {start}..{end}");
                        }
                        for scope in [
                            Phrasing,
                            Label,
                            DestinationRaw,
                            HeadingAtx,
                            CodeFencedLangGraveAccent,
                        ] {
                            let mut actual = Vec::new();
                            collect(
                                ["", &value[..start]],
                                &value[start..end],
                                &value[end..],
                                &[scope],
                                single,
                                &mut actual,
                            );
                            let mut expected = Vec::new();
                            for rule in RULES.iter().filter(|rule| rule.active(bit(scope))) {
                                for (_, position) in rule.matches(context, single) {
                                    if position >= start.saturating_sub(1) && position < end {
                                        expected.push((
                                            position,
                                            !matches!(rule.before, Before::Any),
                                            !matches!(rule.after, After::Any | After::Closing)
                                                && !(matches!(rule.after, After::Math) && single),
                                        ));
                                    }
                                }
                            }
                            let actual: Vec<_> = actual
                                .into_iter()
                                .map(|(at, flags)| (at, flags.before, flags.after))
                                .collect();
                            assert_eq!(actual, expected, "{value:?}, {start}..{end}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn fixed_matchers_preserve_non_overlapping_regex_rules() {
        let contexts = [
            "", "\n", "\r\n  ", "\n\t123", "x]", "\n12", "\n123 ", "é", "\n\n", "\r", "\r\n",
        ];
        // A regular expression consumes what follows, so `After::Closing` is checked at each byte.
        let closing = regex::Regex::new("\\A(?:\n|\r\n?)\\z").unwrap();
        for rule in RULES {
            for single in [false, true] {
                let before = match rule.before {
                    Before::Any => "",
                    Before::Previous(b'\n') => "(\\n)",
                    Before::Previous(b'\r') => "(\\r)",
                    Before::Previous(_) => "(\\])",
                    Before::Line => "([\\r\\n][\\t ]*)",
                    Before::Digits => "([\\r\\n][\\t ]*[0-9]+)",
                };
                let after = match rule.after {
                    After::Any => "".to_string(),
                    After::Set(set) => {
                        format!("[{}]", regex::escape(core::str::from_utf8(set).unwrap()))
                    }
                    After::LineEnd => "(?:[\\r\\n]|$)".into(),
                    After::SpaceOrEnd => "(?:[ \\t\\r\\n]|$)".into(),
                    After::Entity => "[#A-Za-z]".into(),
                    After::Html => "[!/?A-Za-z]".into(),
                    After::Email => "[#-'*+\\-.0-9=^-`{-~]".into(),
                    After::Math if single => "".into(),
                    After::Math => "\\$".into(),
                    After::Closing => "".into(),
                };
                let expression = regex::Regex::new(&format!(
                    "{before}{}{after}",
                    regex::escape(&char::from(rule.byte).to_string())
                ))
                .unwrap();
                for left in contexts {
                    for right in contexts {
                        for byte in 0..=127u8 {
                            let value = format!(
                                "{left}{}{}{right}{}",
                                char::from(rule.byte),
                                char::from(byte),
                                char::from(rule.byte)
                            );
                            let actual: Vec<_> = rule
                                .matches(Context([b"", b"", value.as_bytes(), b""]), single)
                                .collect();
                            let expected: Vec<_> = if matches!(rule.after, After::Closing) {
                                value
                                    .bytes()
                                    .enumerate()
                                    .filter(|&(at, byte)| {
                                        byte == rule.byte && closing.is_match(&value[at + 1..])
                                    })
                                    .map(|(at, _)| (at..at + 1, at))
                                    .collect()
                            } else {
                                expression
                                    .captures_iter(&value)
                                    .map(|capture| {
                                        let found = capture.get(0).unwrap();
                                        let position = found.start()
                                            + capture.get(1).map_or(0, |group| group.len());
                                        (found.range(), position)
                                    })
                                    .collect()
                            };
                            assert_eq!(actual, expected, "{value:?}");
                        }
                    }
                }
            }
        }
    }
}
