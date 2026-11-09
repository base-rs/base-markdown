// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Attention sequence selection.
//!
//! The writer keeps three invariants, under which the parser pairs every opening sequence with
//! the closing sequence of the same node:
//!
//! 1. Runs never join: each emphasis, strong or strikethrough node has its own opening and
//!    closing run, and where one node's run would touch another's, the two use different
//!    markers, so emphasis in strong is written `*__a__*`, not `***a***`.
//! 2. Text writes every `*`, `_` and `~` that could form a run escaped, so the only runs are
//!    the sequences of attention.
//! 3. Nodes nest, so the nearest opening run of a marker before a closing run is its node's,
//!    and the two have the same length, which the rule of 3 never keeps apart.
//!
//! What is left is local. A run opens or closes by the kinds of the characters next to it
//! (`crate::util::attention::sides`), and an opening run that can also close closes the
//! nearest enclosing run of its marker that it pairs with (#111). Each node chooses its marker,
//! `*` or `_` for emphasis and strong, and text next to a run may have its edge character, a
//! letter, written as a character reference, which is punctuation. These choices meet only
//! through the kind of character between two pieces and the markers of the open attention, so
//! `plan` makes them for a whole scope, a paragraph, heading, table cell or link text, by
//! dynamic programming over the pieces in order, with one state per kind of boundary: it finds
//! a choice wherever one exists, or none, and the writer returns `unwritable-node`.
use super::util::safe::Literal;
use crate::util::attention::{closes_enclosing, pairs, sides, Run as AttentionRun};
use crate::util::char::Kind;
use alloc::{string::String, vec::Vec};
use core::convert::TryFrom;
use core::ops::Range;

/// Sequences for emphasis, strong and strikethrough, in order of preference.
pub(super) static EMPHASIS_ASTERISK: &[&str] = &["*", "_"];
pub(super) static EMPHASIS_UNDERSCORE: &[&str] = &["_", "*"];
pub(super) static STRONG_ASTERISK: &[&str] = &["**", "__"];
pub(super) static STRONG_UNDERSCORE: &[&str] = &["__", "**"];
pub(super) static STRIKETHROUGH: &[&str] = &["~~"];
/// Where `ParseOptions::strikethrough_single_tilde` reads one tilde and the text holds a run of
/// two tildes, which no escape keeps from closing two, and none of one.
pub(super) static STRIKETHROUGH_SINGLE: &[&str] = &["~", "~~"];

/// A phrasing node of the scope being planned, as far as the attention in it needs.
pub(super) enum Piece {
    /// Text, by the kinds of the first and last characters of its value, which is not empty,
    /// and whether that value is one character, which both edges then share.
    Text {
        first: Kind,
        last: Kind,
        single: bool,
    },
    /// Anything else, by the kinds of the characters it is written to start and end with, and
    /// whether it starts with the language of code, which `_` before it would join (#185).
    Other {
        first: Kind,
        last: Kind,
        language: bool,
    },
    /// Attention, by its sequences in order of preference and the pieces of its children.
    Attention {
        sequences: &'static [&'static str],
        children: Range<usize>,
    },
    /// Attention that writes no character, which a normalized tree does not have. It is not
    /// written, so it reads back as normalizing would leave it (`EMPTY`).
    Empty,
}

/// How a piece is written. For text, whether its first character (`ENCODE_FIRST`) and last
/// character (`ENCODE_LAST`) are written as character references; for attention, its
/// `Spelling`.
pub(super) type Choice = u16;
pub(super) const ENCODE_FIRST: Choice = 1;
pub(super) const ENCODE_LAST: Choice = 2;
/// The choice for attention that is not written (`Piece::Empty`).
pub(super) const EMPTY: Choice = Choice::MAX;
/// The most levels of attention written in one run with attention.
const LEVELS: usize = 7;
const JOINED: Choice = 1 << 7;
const CLOSED: Choice = 1 << 8;

/// How attention is written: with which of its sequences, and in one run with the attention
/// in it. Runs join only where the parser reads them back exactly, and joining costs more than
/// writing the runs apart.
///
/// - `joins` levels of emphasis or strong, each the first child of the one before, are written
///   in one opening run with this node, as in `***a**b*`. A level that is the only child of the
///   one before closes in one run with it, as in `***a***`, and the others with their own runs.
///   Each closing run pairs with the opening run where the rule of 3 between the two lets them
///   (`crate::util::attention::pairs`), and takes two markers from it, or one where it or what
///   is left of the opening run has one, the innermost level first: the levels must take as
///   many as their sequences have, so emphasis in strong closed in one run does not read back.
/// - `closes` levels of emphasis or strong, each the last child of the one before, are written
///   in one closing run with this node, as in `*a**b***`, and open with their own runs, or in
///   one run with the one before where they are its only child. The closing run pairs with each
///   opening run in turn, the innermost first, likewise.
/// - A node that is `joined` writes no opening run, and one that is `closed` no closing run,
///   as attention around it writes them in its own.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Spelling {
    pub(super) sequence: usize,
    pub(super) joins: usize,
    pub(super) closes: usize,
    pub(super) joined: bool,
    pub(super) closed: bool,
}

impl Spelling {
    pub(super) fn of(choice: Choice) -> Self {
        let field = |shift: u32| usize::from(choice >> shift & 7);
        Spelling {
            sequence: usize::from(choice & 1),
            joins: field(1),
            closes: field(4),
            joined: choice & JOINED != 0,
            closed: choice & CLOSED != 0,
        }
    }

    fn choice(self) -> Choice {
        let field = |value: usize| Choice::try_from(value).expect("few levels");
        field(self.sequence)
            | field(self.joins) << 1
            | field(self.closes) << 4
            | if self.joined { JOINED } else { 0 }
            | if self.closed { CLOSED } else { 0 }
    }
}

/// Choose how each piece of a scope is written, where the scope's `pieces` from `root` are
/// written between characters of kinds `before` and `after`, and characters can be encoded
/// where `encode` holds. `None` where no choice keeps every invariant.
///
/// Fewer encoded characters come first, then fewer levels written in one run, then fewer
/// sequences other than the preferred one; among choices that cost as little, each piece in
/// turn takes its most preferred option, so the choice depends on the tree alone.
pub(super) fn plan(
    pieces: &[Piece],
    root: Range<usize>,
    before: Kind,
    after: Kind,
    encode: bool,
) -> Option<Vec<Choice>> {
    let start = state(edge(before), NONE);
    let after = 1 + edge(after);
    let mut ends = [INFEASIBLE; STATES];
    for (end, cost) in ends.iter_mut().enumerate() {
        let required = split(end).1;
        if required == NONE || required == after {
            *cost = 0;
        }
    }
    // Pieces between the levels of a run are not assigned, and only empty ones can be there.
    let mut choices: Vec<Choice> = pieces
        .iter()
        .map(|piece| {
            if matches!(piece, Piece::Empty) {
                EMPTY
            } else {
                0
            }
        })
        .collect();
    // A choice with only preferred sequences, nothing encoded and no levels in one run costs
    // nothing, and there is at most one, as what follows each piece is then fixed; otherwise each
    // stage finds the best choice where there is one it considers.
    if preferred(pieces, root.clone(), 0, start, after - 1).is_some() {
        return Some(choices);
    }
    for stage in [Stage::Sequences, Stage::Levels, Stage::Full] {
        let mut planner = Planner {
            pieces,
            encode,
            stage,
            options: (0..pieces.len()).map(|_| Vec::new()).collect(),
            inner: (0..pieces.len()).map(|_| Vec::new()).collect(),
        };
        if planner.assign(root.clone(), 0, start, ends, &mut choices) {
            return Some(choices);
        }
    }
    None
}

/// The state after writing the pieces of `list` from state `at` with the choice that costs
/// nothing, inside the attention in `open`, before a character of kind `follow`, or `None` where
/// that choice breaks an invariant. This walks the one path `Planner::step` would cost nothing
/// along.
fn preferred(
    pieces: &[Piece],
    list: Range<usize>,
    open: u16,
    mut at: usize,
    follow: u8,
) -> Option<usize> {
    let first_kind = |list: Range<usize>| {
        list.map(|index| match &pieces[index] {
            Piece::Text { first, .. } | Piece::Other { first, .. } => Some(edge(*first)),
            Piece::Attention { .. } => Some(PUNCTUATION),
            Piece::Empty => None,
        })
        .find_map(|kind| kind)
    };
    // What each run requires of the character after it is known as it is walked, so it holds.
    for index in list.clone() {
        let last = split(at).0;
        at = match &pieces[index] {
            Piece::Empty => continue,
            Piece::Text {
                first,
                last: end,
                single,
            } => state(edge(if *single { *first } else { *end }), NONE),
            Piece::Other {
                last: end,
                language,
                ..
            } => {
                if *language && last == RUN + 1 {
                    return None;
                }
                state(edge(*end), NONE)
            }
            Piece::Attention {
                sequences,
                children,
            } => {
                let sequence = sequences[0];
                let (marker, byte) = marker(sequence);
                if last == RUN + marker {
                    return None;
                }
                let first = first_kind(children.clone())?;
                let (opens, closes) = sides(byte, kind(last), kind(first));
                if !opens || closes && closes_open(open, marker, byte, sequence.len()) {
                    return None;
                }
                let inside = open | bit(marker, sequence.len());
                let end = preferred(
                    pieces,
                    children.clone(),
                    inside,
                    state(RUN + marker, 1 + first),
                    PUNCTUATION,
                )?;
                let end = split(end).0;
                let after = first_kind(index + 1..list.end).unwrap_or(follow);
                let (reopens, closes_here) = sides(byte, kind(end), kind(after));
                if end == RUN + marker
                    || !closes_here
                    || !pairs(
                        byte,
                        AttentionRun::whole(sequence.len(), true, closes),
                        AttentionRun::whole(sequence.len(), reopens, true),
                    )
                {
                    return None;
                }
                state(RUN + marker, 1 + after)
            }
        };
    }
    Some(at)
}

/// Which choices a plan considers: any sequences, then also levels written in one run, then also
/// encoded characters. Each costs less than any choice only a later stage considers, so the first
/// stage with a choice finds the best.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    Sequences,
    Levels,
    Full,
}

/// A cost: encoded characters in the high half, then levels written in one run, then sequences
/// other than the preferred.
type Cost = u64;
const INFEASIBLE: Cost = Cost::MAX;
const ENCODING: Cost = 1 << 42;
const MERGE: Cost = 1 << 21;

/// What the output ends with after a piece: the kind of its last character, whitespace (0),
/// punctuation (1) or other (2), or a run of `*` (3), `_` (4) or `~` (5). Together with what
/// the kind of the next piece's first character must be, none (0) or a kind plus 1, as the
/// run just written was chosen to flank next to it.
const STATES: usize = 24;
const NONE: u8 = 0;
const WHITESPACE: u8 = 0;
const PUNCTUATION: u8 = 1;
const OTHER: u8 = 2;
const RUN: u8 = 3;
type Table = [Cost; STATES];

fn state(last: u8, next: u8) -> usize {
    usize::from(last) * 4 + usize::from(next)
}

fn split(state: usize) -> (u8, u8) {
    (
        u8::try_from(state / 4).expect("a state is small"),
        u8::try_from(state % 4).expect("a state is small"),
    )
}

fn edge(kind: Kind) -> u8 {
    match kind {
        Kind::Whitespace => WHITESPACE,
        Kind::Punctuation => PUNCTUATION,
        Kind::Other => OTHER,
    }
}

/// The kind of what the output ends with: a run of markers is punctuation.
fn kind(last: u8) -> Kind {
    match last {
        WHITESPACE => Kind::Whitespace,
        OTHER => Kind::Other,
        _ => Kind::Punctuation,
    }
}

/// The marker of a sequence, by its index after `RUN`, and as a byte.
fn marker(sequence: &str) -> (u8, u8) {
    let byte = sequence.as_bytes()[0];
    (
        match byte {
            b'*' => 0,
            b'_' => 1,
            _ => 2,
        },
        byte,
    )
}

/// The bit of a set of open attention that a run of `marker` and `size` takes. Whether a run
/// closes another of `*` or `_` depends on their sizes modulo 3 alone, and runs of `~` are of
/// size 1 or 2.
fn bit(marker: u8, size: usize) -> u16 {
    1 << (usize::from(marker) * 3 + size % 3)
}

/// Whether an opening run of `byte` and `size`, which can also close, would close attention in
/// `open`, the set of enclosing attention by marker and size.
fn closes_open(open: u16, index: u8, byte: u8, size: usize) -> bool {
    (1..=3).any(|enclosing| {
        open & bit(index, enclosing) != 0 && closes_enclosing(byte, enclosing, size)
    })
}

struct Planner<'p> {
    pieces: &'p [Piece],
    encode: bool,
    /// Which choices are considered.
    stage: Stage,
    /// For each attention piece, per set of open attention, the ways to write it after a
    /// character of each kind (`Planner::options`).
    options: Vec<Vec<(u16, Options)>>,
    /// For each attention piece, per set of open attention and choice, how it is written.
    inner: Vec<Vec<(u32, Inner)>>,
}

/// The ways to write attention after a character of each kind: its marker, the kind its closing
/// run was chosen to flank before, its cost and its choice.
type Options = [Vec<(u8, u8, Cost, Choice)>; 3];

/// How attention is written with one choice: its runs, what is written between them, and its
/// least cost after a character of each kind and before one of each kind.
struct Inner {
    run: Option<Run>,
    steps: Vec<Step>,
    best: [[Best; 3]; 3],
}

/// The least cost of writing attention, with the kind of its first character, the state before
/// its closing run, and whether that run was taken to be able to open, where the levels closed
/// in it (`Spelling::closes`) depend on that.
type Best = (Cost, u8, usize, bool);

/// Whether attention piece `piece` is emphasis or strong, and which.
fn strong(piece: &Piece) -> Option<bool> {
    match piece {
        Piece::Attention { sequences, .. } => {
            (!sequences[0].starts_with('~')).then_some(sequences[0].len() == 2)
        }
        _ => None,
    }
}

/// The children of attention piece `piece`.
fn children(piece: &Piece) -> Range<usize> {
    let Piece::Attention { children, .. } = piece else {
        unreachable!("only attention has children");
    };
    children.clone()
}

impl Planner<'_> {
    /// Each way to write piece `index`, inside the attention in `open`, after state `from`, in
    /// order of preference: the state it leaves, its cost and its choice, for attention shifted
    /// above the kind its closing run was chosen to flank before.
    fn step(
        &mut self,
        index: usize,
        open: u16,
        from: usize,
        visit: &mut dyn FnMut(usize, Cost, u32),
    ) {
        let (last, required) = split(from);
        match &self.pieces[index] {
            Piece::Text {
                first,
                last: end,
                single,
            } => {
                let first = edge(*first);
                let end = edge(*end);
                for encode_first in [false, true] {
                    if encode_first && !(self.stage == Stage::Full && self.encode && first == OTHER)
                    {
                        continue;
                    }
                    let kind = if encode_first { PUNCTUATION } else { first };
                    if required != NONE && required != 1 + kind {
                        continue;
                    }
                    if *single {
                        let choice = if encode_first {
                            ENCODE_FIRST | ENCODE_LAST
                        } else {
                            0
                        };
                        visit(
                            state(kind, NONE),
                            ENCODING * Cost::from(encode_first),
                            u32::from(choice),
                        );
                        continue;
                    }
                    for encode_last in [false, true] {
                        if encode_last
                            && !(self.stage == Stage::Full && self.encode && end == OTHER)
                        {
                            continue;
                        }
                        visit(
                            state(if encode_last { PUNCTUATION } else { end }, NONE),
                            ENCODING * (Cost::from(encode_first) + Cost::from(encode_last)),
                            u32::from(
                                (Choice::from(encode_first) * ENCODE_FIRST)
                                    | (Choice::from(encode_last) * ENCODE_LAST),
                            ),
                        );
                    }
                }
            }
            Piece::Other {
                first,
                last: end,
                language,
            } => {
                if !(*language && last == RUN + 1
                    || required != NONE && required != 1 + edge(*first))
                {
                    visit(state(edge(*end), NONE), 0, 0);
                }
            }
            Piece::Empty => visit(from, 0, u32::from(EMPTY)),
            // Callers pass only states that let attention start with its run.
            Piece::Attention { .. } => {
                let slot = self.options(index, open);
                let options = &self.options[index][slot].1[usize::from(edge(kind(last)))];
                for &(marker, after, cost, choice) in options {
                    // Runs never join, but as `Spelling` describes.
                    if last != RUN + marker {
                        visit(
                            state(RUN + marker, 1 + after),
                            cost,
                            u32::from(choice) << 2 | u32::from(after),
                        );
                    }
                }
            }
        }
    }

    /// Where `options` holds the ways to write attention piece `index` inside the attention in
    /// `open`, in order of preference, working them out the first time.
    fn options(&mut self, index: usize, open: u16) -> usize {
        if let Some(slot) = self.options[index]
            .iter()
            .position(|(found, _)| *found == open)
        {
            return slot;
        }
        let Piece::Attention { sequences, .. } = &self.pieces[index] else {
            unreachable!("only attention has options");
        };
        let sequences = *sequences;
        let (joins, closes) = if self.stage != Stage::Sequences {
            self.levels(index)
        } else {
            (0, 0)
        };
        // The choices in order of preference, with how each is written.
        let mut written = Vec::new();
        for (sequence, value) in sequences.iter().enumerate() {
            let (marker, _) = marker(value);
            let plain = Spelling {
                sequence,
                ..Spelling::default()
            };
            let spellings = (0..=joins)
                .map(|joins| Spelling { joins, ..plain })
                .chain((1..=closes).map(|closes| Spelling { closes, ..plain }));
            for spelling in spellings {
                let choice = spelling.choice();
                let slot = self.inner(index, open, choice);
                written.push((marker, choice, slot));
            }
        }
        let mut options: Options = Default::default();
        for (before, list) in options.iter_mut().enumerate() {
            for &(marker, choice, slot) in &written {
                for after in [WHITESPACE, PUNCTUATION, OTHER] {
                    let (cost, ..) = self.inner[index][slot].1.best[before][usize::from(after)];
                    if cost != INFEASIBLE {
                        list.push((marker, after, cost, choice));
                    }
                }
            }
        }
        self.options[index].push((open, options));
        self.options[index].len() - 1
    }

    /// The least cost of writing the pieces of `list`, inside the attention in `open`, from each
    /// state at the cost in `current`, by the state they end in.
    fn sequence(&mut self, list: Range<usize>, open: u16, mut current: Table) -> Table {
        for index in list {
            let mut next = [INFEASIBLE; STATES];
            let mut visit = |from: usize, cost: Cost, planner: &mut Self| {
                if cost != INFEASIBLE {
                    planner.step(index, open, from, &mut |to, added, _| {
                        next[to] = next[to].min(cost + added);
                    });
                }
            };
            // Text is written the same way after any states that require the same of it, and
            // attention after any that end the same way and let it start with a run.
            match self.pieces[index] {
                Piece::Empty => continue,
                Piece::Text { .. } => {
                    for required in 0..4 {
                        let cost = (0..6).map(|last| current[state(last, required)]).min();
                        visit(state(0, required), cost.unwrap_or(INFEASIBLE), self);
                    }
                }
                Piece::Attention { .. } => {
                    let slot = self.options(index, open);
                    let options = &self.options[index][slot].1;
                    for last in 0..6 {
                        let cost =
                            current[state(last, NONE)].min(current[state(last, 1 + PUNCTUATION)]);
                        if cost == INFEASIBLE {
                            continue;
                        }
                        for &(marker, after, added, _) in &options[usize::from(edge(kind(last)))] {
                            if last != RUN + marker {
                                let to = &mut next[state(RUN + marker, 1 + after)];
                                *to = (*to).min(cost + added);
                            }
                        }
                    }
                }
                Piece::Other { .. } => {
                    for (from, &cost) in current.iter().enumerate() {
                        visit(from, cost, self);
                    }
                }
            }
            current = next;
        }
        current
    }

    /// The least cost of writing the rest of `list`, inside the attention in `open`, to a state
    /// that `ends` costs other than `INFEASIBLE`, from each state before each piece, and last
    /// `ends` itself.
    fn rest(&mut self, list: Range<usize>, open: u16, ends: Table) -> Vec<Table> {
        let mut rest = alloc::vec![ends; list.len() + 1];
        for (offset, index) in list.enumerate().rev() {
            let after = rest[offset + 1];
            let mut costs = [INFEASIBLE; STATES];
            let cost_from = |from: usize, planner: &mut Self| {
                let mut cost = INFEASIBLE;
                planner.step(index, open, from, &mut |to, added, _| {
                    if after[to] != INFEASIBLE {
                        cost = cost.min(added + after[to]);
                    }
                });
                cost
            };
            // As in `sequence`, text and attention are written the same way after many states.
            match self.pieces[index] {
                Piece::Text { .. } => {
                    for required in 0..4 {
                        let cost = cost_from(state(0, required), self);
                        for last in 0..6 {
                            costs[state(last, required)] = cost;
                        }
                    }
                }
                Piece::Attention { .. } => {
                    for last in 0..6 {
                        let cost = cost_from(state(last, NONE), self);
                        costs[state(last, NONE)] = cost;
                        costs[state(last, 1 + PUNCTUATION)] = cost;
                    }
                }
                _ => {
                    for (from, cost) in costs.iter_mut().enumerate() {
                        *cost = cost_from(from, self);
                    }
                }
            }
            rest[offset] = costs;
        }
        rest
    }

    /// Record the choices that write `list` from `start` to a state that `ends` costs other than
    /// `INFEASIBLE`, at least cost, or return `false` where there are none. Where several cost
    /// the least, each piece in turn takes its most preferred option that still can.
    fn assign(
        &mut self,
        list: Range<usize>,
        open: u16,
        start: usize,
        ends: Table,
        choices: &mut [Choice],
    ) -> bool {
        let rest = self.rest(list.clone(), open, ends);
        if rest[0][start] == INFEASIBLE {
            return false;
        }
        let mut at = start;
        for (offset, index) in list.enumerate() {
            let goal = rest[offset][at];
            let after = rest[offset + 1];
            let mut taken = None;
            self.step(index, open, at, &mut |to, added, choice| {
                if taken.is_none() && after[to] != INFEASIBLE && added + after[to] == goal {
                    taken = Some((to, choice));
                }
            });
            let (to, choice) = taken.expect("a least cost is reached");
            let from = at;
            at = to;
            if !matches!(self.pieces[index], Piece::Attention { .. }) {
                choices[index] = Choice::try_from(choice).expect("a choice is small");
                continue;
            }
            let after = u8::try_from(choice & 3).expect("a kind is small");
            let choice = Choice::try_from(choice >> 2).expect("a choice is small");
            choices[index] = choice;
            self.assign_attention(index, open, choice, kind(split(from).0), after, choices);
        }
        true
    }

    /// Record the choice for one level of a run, `node`, and those for the pieces of `list`,
    /// inside the attention in `open`, which `tables` costs from each state before them by
    /// `start` and after them by `end`, to reach `target`: return the state `list` starts from.
    fn assign_level(
        &mut self,
        list: Range<usize>,
        open: u16,
        start: &Table,
        target: usize,
        choices: &mut [Choice],
    ) -> usize {
        let mut ends = [INFEASIBLE; STATES];
        ends[target] = 0;
        let rest = self.rest(list.clone(), open, ends)[0];
        let from = (0..STATES)
            .filter(|&at| start[at] != INFEASIBLE && rest[at] != INFEASIBLE)
            .min_by_key(|&at| start[at] + rest[at])
            .expect("the level was costed");
        let assigned = self.assign(list, open, from, ends, choices);
        debug_assert!(assigned, "the level was costed");
        from
    }

    /// Record the choices for the children of attention piece `index`, written with `choice`
    /// inside the attention in `open`, after a character of kind `before` and before one of
    /// kind `after`, and for the levels written in one run with it.
    fn assign_attention(
        &mut self,
        index: usize,
        open: u16,
        choice: Choice,
        before: Kind,
        after: u8,
        choices: &mut [Choice],
    ) {
        let (_, first, end, reopens) = self.attention(index, open, choice, before, after);
        let slot = self.inner(index, open, choice);
        let entry = &mut self.inner[index][slot].1;
        let run = entry.run.take().expect("the run was costed");
        let steps = core::mem::take(&mut entry.steps);
        let (_, closes) = sides(run.byte, before, kind(first));
        let mut tables = Vec::new();
        self.evaluate(&run, &steps, closes, first, reopens, Some(&mut tables));
        let mut target = end;
        for (position, step) in steps.iter().enumerate().rev() {
            let (before, after) = (tables[position], tables[position + 1]);
            target = match *step {
                Step::Pieces(ref list, open) => {
                    self.assign_level(list.clone(), open, &before, target, choices)
                }
                Step::Opening(size, open) => (0..STATES)
                    .find(|&from| {
                        before[from] == after[target]
                            && opening(&run, size, open, reopens, from).any(|to| to == target)
                    })
                    .expect("the opening run was costed"),
                Step::Closing(size) => (0..STATES)
                    .find(|&from| {
                        before[from] == after[target]
                            && closing(&run, size, closes, from).any(|to| to == target)
                    })
                    .expect("the closing run was costed"),
            };
        }
        debug_assert_eq!(target, state(RUN + run.marker, 1 + first));
        for (&node, &grouped) in run.levels.iter().zip(&run.grouped) {
            let Piece::Attention { sequences, .. } = &self.pieces[node] else {
                unreachable!("a run is of attention");
            };
            let sequence = sequences
                .iter()
                .position(|sequence| sequence.as_bytes()[0] == run.byte)
                .expect("emphasis and strong have both markers");
            choices[node] = Spelling {
                sequence,
                joined: run.joins || grouped,
                closed: !run.joins || grouped,
                ..Spelling::default()
            }
            .choice();
        }
        let entry = &mut self.inner[index][slot].1;
        entry.run = Some(run);
        entry.steps = steps;
    }

    /// The pieces of `list` that write anything.
    fn written(&self, list: Range<usize>) -> impl Iterator<Item = usize> + '_ {
        list.filter(move |&index| !matches!(self.pieces[index], Piece::Empty))
    }

    /// The first or last piece of `list` that writes anything, where it is emphasis or strong,
    /// and whether it is the only one.
    fn end(&self, list: Range<usize>, last: bool) -> Option<(usize, bool)> {
        let mut written = self.written(list);
        let first = written
            .next()
            .expect("attention holds a piece that writes something");
        let rest = written.last();
        let found = if last { rest.unwrap_or(first) } else { first };
        strong(&self.pieces[found]).map(|_| (found, rest.is_none()))
    }

    /// How many levels attention piece `index` can be written in one run with, as `joins` and
    /// `closes` (`Spelling`).
    fn levels(&self, index: usize) -> (usize, usize) {
        if strong(&self.pieces[index]).is_none() {
            return (0, 0);
        }
        let count = |last: bool| {
            let mut levels = 0;
            let mut at = index;
            while levels < LEVELS {
                let Some((found, _)) = self.end(children(&self.pieces[at]), last) else {
                    break;
                };
                at = found;
                levels += 1;
            }
            levels
        };
        (count(false), count(true))
    }

    /// The size of the sequences of emphasis or strong piece `index`.
    fn size(&self, index: usize) -> usize {
        if strong(&self.pieces[index]) == Some(true) {
            2
        } else {
            1
        }
    }

    /// The runs that attention piece `index` is written with under `choice`, or `None` where
    /// the levels in them would not each take as many markers as their sequences have.
    fn run(&self, index: usize, choice: Choice) -> Option<Run> {
        let Piece::Attention { sequences, .. } = &self.pieces[index] else {
            unreachable!("only attention has a sequence");
        };
        let spelling = Spelling::of(choice);
        let sequence = sequences[spelling.sequence];
        let (marker, byte) = marker(sequence);
        let joins = spelling.closes == 0;
        if spelling.joins == 0 && joins {
            return Some(Run {
                marker,
                byte,
                size: sequence.len(),
                closing: sequence.len(),
                joins,
                levels: Vec::new(),
                grouped: Vec::new(),
            });
        }
        let mut levels = Vec::new();
        let mut grouped = Vec::new();
        let mut at = index;
        for _ in 0..spelling.joins.max(spelling.closes) {
            let (found, only) = self
                .end(children(&self.pieces[at]), !joins)
                .expect("the levels were counted");
            levels.push(found);
            grouped.push(only);
            at = found;
        }
        // The sizes of the levels, the node first, in groups that share a run: the opening run
        // where levels close in one run, and the closing runs where they join one.
        let sizes: Vec<usize> = core::iter::once(sequence.len())
            .chain(levels.iter().map(|&node| self.size(node)))
            .collect();
        let mut groups: Vec<Vec<usize>> = Vec::new();
        for (level, &size) in sizes.iter().enumerate() {
            match groups.last_mut() {
                Some(group) if level > 0 && grouped[level - 1] => group.push(size),
                _ => groups.push(alloc::vec![size]),
            }
        }
        let total: usize = sizes.iter().sum();
        // The parser takes two markers or one from the nearest opening run with any left, the
        // innermost level first.
        let mut shared = total;
        for group in groups.iter().rev() {
            let mut left = if joins { group.iter().sum() } else { total };
            let mut each = if joins { shared } else { group.iter().sum() };
            for &size in group.iter().rev() {
                if each.min(left).min(2) != size {
                    return None;
                }
                each -= size;
                left -= size;
            }
            if joins {
                shared -= group.iter().sum::<usize>();
            } else {
                shared = left;
            }
        }
        let first: usize = groups[0].iter().sum();
        Some(Run {
            marker,
            byte,
            size: if joins { total } else { first },
            closing: if joins { first } else { total },
            joins,
            levels,
            grouped,
        })
    }

    /// What is written of attention piece `index` as `run`, inside the attention in `inside`,
    /// between its opening run and its closing run, in order.
    fn steps(&self, index: usize, run: &Run, inside: u16) -> Vec<Step> {
        let mut steps = Vec::new();
        let parents = core::iter::once(index).chain(run.levels.iter().copied());
        let around: Vec<usize> = parents.collect();
        let sizes = |from: usize| {
            let mut size = self.size(run.levels[from]);
            for level in from + 1..run.levels.len() {
                if !run.grouped[level] {
                    break;
                }
                size += self.size(run.levels[level]);
            }
            size
        };
        let innermost = *around.last().expect("the node is a level");
        if run.joins {
            // The innermost level's children, then, going out, the closing run of each level
            // that closes on its own and the rest of the children of the level around it.
            steps.push(Step::Pieces(children(&self.pieces[innermost]), inside));
            for level in (0..run.levels.len()).rev() {
                if run.grouped[level] {
                    continue;
                }
                steps.push(Step::Closing(sizes(level)));
                let node = run.levels[level];
                let list = children(&self.pieces[around[level]]);
                steps.push(Step::Pieces(node + 1..list.end, inside));
            }
        } else {
            // Going in, the children of each level up to the next, and the opening run of each
            // level that opens on its own, then the innermost level's children.
            let mut open = inside;
            for (level, &piece) in around[..=run.levels.len()].iter().enumerate() {
                if level > 0 && !run.grouped[level - 1] {
                    let size = sizes(level - 1);
                    steps.push(Step::Opening(size, open));
                    open |= bit(run.marker, size);
                }
                let list = children(&self.pieces[piece]);
                match run.levels.get(level) {
                    Some(&next) if !run.grouped[level] => {
                        steps.push(Step::Pieces(list.start..next, open));
                    }
                    Some(_) => {}
                    None => steps.push(Step::Pieces(list, open)),
                }
            }
        }
        steps
    }

    /// The kinds, as bits, that what `steps` write can start with.
    fn firsts(&self, steps: &[Step]) -> u8 {
        for step in steps {
            let Step::Pieces(list, _) = step else {
                unreachable!("a level that opens on its own follows a piece that writes something");
            };
            for index in list.clone() {
                return match &self.pieces[index] {
                    Piece::Empty => continue,
                    Piece::Text { first, .. } => {
                        let first = edge(*first);
                        let encoded = self.stage == Stage::Full && self.encode && first == OTHER;
                        1 << first | u8::from(encoded) << PUNCTUATION
                    }
                    Piece::Other { first, .. } => 1 << edge(*first),
                    Piece::Attention { .. } => 1 << PUNCTUATION,
                };
            }
        }
        unreachable!("attention holds a piece that writes something")
    }

    /// The costs of the states after each of `steps` of `run`, which starts with a character of
    /// kind `first`, and first before them, where the opening run can close if `closes` and the
    /// closing run is taken to open if `reopens`.
    fn evaluate(
        &mut self,
        run: &Run,
        steps: &[Step],
        closes: bool,
        first: u8,
        reopens: bool,
        mut tables: Option<&mut Vec<Table>>,
    ) -> Table {
        let mut current = [INFEASIBLE; STATES];
        current[state(RUN + run.marker, 1 + first)] = 0;
        if let Some(tables) = tables.as_mut() {
            tables.push(current);
        }
        for step in steps {
            // Nothing reaches the closing run once no state can be reached.
            if tables.is_none() && current.iter().all(|&cost| cost == INFEASIBLE) {
                break;
            }
            current = match *step {
                Step::Pieces(ref list, open) => self.sequence(list.clone(), open, current),
                Step::Opening(size, open) => {
                    let mut next = [INFEASIBLE; STATES];
                    for (end, &cost) in current.iter().enumerate() {
                        if cost != INFEASIBLE {
                            for to in opening(run, size, open, reopens, end) {
                                next[to] = next[to].min(cost);
                            }
                        }
                    }
                    next
                }
                Step::Closing(size) => {
                    let mut next = [INFEASIBLE; STATES];
                    for (end, &cost) in current.iter().enumerate() {
                        if cost != INFEASIBLE {
                            for to in closing(run, size, closes, end) {
                                next[to] = next[to].min(cost);
                            }
                        }
                    }
                    next
                }
            };
            if let Some(tables) = tables.as_mut() {
                tables.push(current);
            }
        }
        current
    }

    /// The least cost of writing attention piece `index` with `choice`, inside the attention in
    /// `open`, after a character of kind `before` and before one of kind `after` (an edge).
    fn attention(
        &mut self,
        index: usize,
        open: u16,
        choice: Choice,
        before: Kind,
        after: u8,
    ) -> Best {
        let slot = self.inner(index, open, choice);
        self.inner[index][slot].1.best[usize::from(edge(before))][usize::from(after)]
    }

    /// Where `inner` holds how attention piece `index` is written with `choice` inside the
    /// attention in `open`, working it out the first time: its least cost after a character of
    /// each kind and before one of each kind.
    fn inner(&mut self, index: usize, open: u16, choice: Choice) -> usize {
        let key = u32::from(open) | u32::from(choice) << 9;
        if let Some(slot) = self.inner[index]
            .iter()
            .position(|(found, _)| *found == key)
        {
            return slot;
        }
        let mut best = [[(INFEASIBLE, 0, 0, false); 3]; 3];
        let mut tables: Vec<(usize, Table)> = Vec::new();
        let run = self.run(index, choice);
        let mut steps = Vec::new();
        if let Some(run) = &run {
            steps = self.steps(index, run, open | bit(run.marker, run.size));
            // Levels in the run take its marker, which may not be their preferred one.
            let mut extra = Cost::from(choice & 1)
                + MERGE * Cost::try_from(run.levels.len()).expect("few levels");
            for &node in &run.levels {
                let Piece::Attention { sequences, .. } = &self.pieces[node] else {
                    unreachable!("a run is of attention");
                };
                extra += Cost::from(sequences[0].as_bytes()[0] != run.byte);
            }
            // Where levels open on their own, whether the closing run can open bears on
            // whether they pair with it.
            let assumptions: &[bool] = if run.joins { &[true] } else { &[false, true] };
            let firsts = self.firsts(&steps);
            for first in [WHITESPACE, PUNCTUATION, OTHER] {
                if firsts & 1 << first == 0 {
                    continue;
                }
                for closes in [false, true] {
                    // The kinds of character before that let the opening run open, and close
                    // as `closes` says, but not close enclosing attention.
                    if closes && closes_open(open, run.marker, run.byte, run.size) {
                        continue;
                    }
                    let befores = [WHITESPACE, PUNCTUATION, OTHER]
                        .map(|before| sides(run.byte, kind(before), kind(first)) == (true, closes));
                    if !befores.contains(&true) {
                        continue;
                    }
                    for &assumed in assumptions {
                        // Whether the opening run can close bears only on levels joined into
                        // it, and whether the closing run can open only on levels closed in it.
                        let at = usize::from(first) * 4
                            + usize::from(closes && run.joins && !run.levels.is_empty()) * 2
                            + usize::from(assumed && !run.joins);
                        let found = tables.iter().position(|&(key, _)| key == at);
                        let found = found.unwrap_or_else(|| {
                            let table = self.evaluate(run, &steps, closes, first, assumed, None);
                            tables.push((at, table));
                            tables.len() - 1
                        });
                        for (end, &cost) in tables[found].1.iter().enumerate() {
                            let (last, required) = split(end);
                            if cost == INFEASIBLE
                                || required != NONE && required != 1 + PUNCTUATION
                                || last == RUN + run.marker
                            {
                                continue;
                            }
                            for after in [WHITESPACE, PUNCTUATION, OTHER] {
                                let (reopens, closes_here) =
                                    sides(run.byte, kind(last), kind(after));
                                if !closes_here
                                    || reopens && !assumed
                                    || !pairs(
                                        run.byte,
                                        AttentionRun::whole(run.size, true, closes),
                                        AttentionRun::whole(run.closing, reopens, true),
                                    )
                                {
                                    continue;
                                }
                                for (before, _) in
                                    befores.iter().enumerate().filter(|(_, &can)| can)
                                {
                                    let best = &mut best[before][usize::from(after)];
                                    if cost + extra < best.0 {
                                        *best = (cost + extra, first, end, assumed);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        self.inner[index].push((key, Inner { run, steps, best }));
        self.inner[index].len() - 1
    }
}

/// What is written between the runs of attention: pieces inside the attention in a set, or a
/// run of a level of the attention, of a size, opening inside the attention in a set.
enum Step {
    Pieces(Range<usize>, u16),
    Opening(usize, u16),
    Closing(usize),
}

/// The runs of attention, as `Planner::run` works them out.
struct Run {
    marker: u8,
    byte: u8,
    /// The size of the node's opening run.
    size: usize,
    /// The size of the node's closing run.
    closing: usize,
    /// Whether the levels are joined into the opening run, or else closed in the closing run.
    joins: bool,
    /// The levels written in one run with the node, outermost first, and whether each is the
    /// only child of the one before, so that its other run is in one with that one's too.
    levels: Vec<usize>,
    grouped: Vec<bool>,
}

/// The states after a closing run of `size` of a level joined into `run`, whose opening run
/// can close if `closes`, written after state `end`, where it closes and pairs with the
/// opening run.
fn closing(run: &Run, size: usize, closes: bool, end: usize) -> impl Iterator<Item = usize> + '_ {
    let (last, required) = split(end);
    let fits = (required == NONE || required == 1 + PUNCTUATION) && last != RUN + run.marker;
    [WHITESPACE, PUNCTUATION, OTHER]
        .iter()
        .copied()
        .filter(move |&after| {
            let (opens, closes_here) = sides(run.byte, kind(last), kind(after));
            fits && closes_here
                && pairs(
                    run.byte,
                    AttentionRun::whole(run.size, true, closes),
                    AttentionRun::whole(size, opens, true),
                )
        })
        .map(move |after| state(RUN + run.marker, 1 + after))
}

/// The states after an opening run of `size` of a level closed in `run`, inside the attention
/// in `open`, written after state `end`, where it opens, closes no enclosing attention and pairs
/// with the closing run, which can open if `reopens`.
fn opening(
    run: &Run,
    size: usize,
    open: u16,
    reopens: bool,
    end: usize,
) -> impl Iterator<Item = usize> + '_ {
    let (last, required) = split(end);
    let fits = (required == NONE || required == 1 + PUNCTUATION) && last != RUN + run.marker;
    [WHITESPACE, PUNCTUATION, OTHER]
        .iter()
        .copied()
        .filter(move |&first| {
            let (opens, closes) = sides(run.byte, kind(last), kind(first));
            fits && opens
                && !(closes && closes_open(open, run.marker, run.byte, size))
                && pairs(
                    run.byte,
                    AttentionRun::whole(size, true, closes),
                    AttentionRun::whole(run.closing, reopens, true),
                )
        })
        .map(move |first| state(RUN + run.marker, 1 + first))
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
