// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Attention sequence selection.
//!
//! The writer keeps three invariants, under which the parser pairs every opening sequence with
//! the closing sequence of the same node:
//!
//! 1. Runs are apart, with two exceptions. Each emphasis, strong or strikethrough node has its
//!    own opening and closing run, and where one node's run would touch another's, the two use
//!    different markers, so emphasis in strong is written `*__a__*`. The exceptions are runs
//!    of emphasis and strong shared with a parent of the same marker: a first child may write
//!    its opening sequence in its parent's opening run, as in `***a* b**`, and a last child
//!    its closing sequence in its parent's closing run, as in `**a *b***`, where the parser
//!    takes the levels from the shared run in turn (`Spelling`).
//! 2. Text writes every `*`, `_` and `~` that could form a run escaped, so the only runs are
//!    the sequences of attention.
//! 3. Nodes nest, so the nearest opening run of a marker before a closing run is its node's.
//!
//! What is left is local. A run opens or closes by the kinds of the characters next to it
//! (`crate::util::attention::sides`), an opening run that can also close closes the nearest
//! enclosing run of its marker that it pairs with (#111), and a closing run pairs with an
//! opening run by the rule of 3, which for a shared run counts the whole run. Each node chooses
//! its marker, `*` or `_` for emphasis and strong, and whether it shares a run with its
//! parent, and text next to a run may have its edge character, a letter, written as a
//! character reference, which is punctuation. These choices meet through the kind of character
//! between two pieces, the markers and sizes of the open attention, and the sizes and markers
//! of the shared runs, so `plan` makes them for a whole scope, a paragraph, heading, table
//! cell or link text, by dynamic programming over the tree of pieces: for each attention node
//! and what its parent asks of it, the least cost of each way to write it, by the kind of
//! character before it and after its closing run. Where no choice keeps the invariants, the
//! writer returns `unwritable-node`; emphasis nested four deep with nothing between, or
//! emphasis and strong alternating six deep around emphasis, each the only child of the next,
//! have none.
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
pub(super) type Choice = u32;
pub(super) const ENCODE_FIRST: Choice = 1;
pub(super) const ENCODE_LAST: Choice = 2;
/// The choice for attention that is not written (`Piece::Empty`).
pub(super) const EMPTY: Choice = Choice::MAX;
const JOINED: Choice = 1 << 1;
const CLOSED: Choice = 1 << 2;

/// How attention is written: with which of its sequences, and whether its runs are written in
/// one with its parent's. A node that is `joined`, its parent's first child, writes no opening
/// run, as its parent's opening run holds its sequence after the parent's own, as in `***a**b*`;
/// one that is `closed`, its parent's last child, writes no closing run, as its parent's
/// closing run holds its sequence before the parent's own, as in `*a**b***`. Both can hold, as
/// in `***a***`, and chains of them make longer runs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Spelling {
    pub(super) sequence: usize,
    pub(super) joined: bool,
    pub(super) closed: bool,
}

impl Spelling {
    pub(super) fn of(choice: Choice) -> Self {
        Spelling {
            sequence: usize::from(choice & 1 != 0),
            joined: choice & JOINED != 0,
            closed: choice & CLOSED != 0,
        }
    }

    fn choice(self) -> Choice {
        Choice::from(self.sequence != 0)
            | if self.joined { JOINED } else { 0 }
            | if self.closed { CLOSED } else { 0 }
    }
}

/// Choose how each piece of a scope is written, where the scope's `pieces` from `root` are
/// written between characters of kinds `before` and `after`, and characters can be encoded
/// where `encode` holds. `None` where no choice keeps every invariant.
///
/// Fewer encoded characters come first, then fewer runs written in one, then fewer sequences
/// other than the preferred one; among choices that cost as little, each piece in turn takes
/// its most preferred option, so the choice depends on the tree alone.
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
    // Pieces not assigned are empty attention, which only empty pieces hold.
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
    let scope = List {
        open: 0,
        first: None,
        last: None,
    };
    let mut planner = Planner {
        pieces,
        encode,
        stage: Stage::Preferred,
        costs: alloc::vec![Vec::new(); pieces.len()],
        written: Vec::new(),
        tables: alloc::vec![Vec::new(); (pieces.len() + 1) * STATES],
    };
    for stage in [
        Stage::Preferred,
        Stage::Sequences,
        Stage::Shared,
        Stage::Full,
    ] {
        // What one stage works out holds for its choices alone.
        planner.stage = stage;
        planner.costs.iter_mut().for_each(Vec::clear);
        planner.written.clear();
        planner.tables.iter_mut().for_each(Vec::clear);
        if planner.assign(root.clone(), &scope, start, &ends, &mut choices) {
            return Some(choices);
        }
    }
    None
}

/// Which choices a plan considers: preferred sequences alone, then any sequences, then also runs
/// written in one, then also encoded characters. Each costs less than any choice only a later
/// stage considers, so the first stage with a choice finds the best, and the first, which most
/// writing needs alone, considers one sequence for each node.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Stage {
    Preferred,
    Sequences,
    Shared,
    Full,
}

/// A cost: encoded characters in the high half, then runs written in one, then sequences other
/// than the preferred.
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
const KINDS: [u8; 3] = [WHITESPACE, PUNCTUATION, OTHER];
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

/// The bits of a set of open attention that an open run of `marker` and `size` sets: one for each
/// size modulo 3 of an opening run of that marker, which can also close, that would close it.
/// Runs of `~` are of size 1 or 2. A set holds what its runs do to a run opening inside them
/// rather than what they are, so attention inside runs that act the same is planned once.
fn bit(marker: u8, size: usize) -> u16 {
    let byte = *b"*_~".get(usize::from(marker)).expect("three markers");
    (1..=3)
        .filter(|&opening| closes_enclosing(byte, size, opening))
        .fold(0, |bits, opening| {
            bits | 1 << (usize::from(marker) * 3 + opening % 3)
        })
}

/// Whether an opening run of `marker` and `size`, which can also close, would close attention in
/// `open` (`bit`).
fn closes_open(open: u16, marker: u8, size: usize) -> bool {
    open & 1 << (usize::from(marker) * 3 + size % 3) != 0
}

/// An opening run that attention is joined into, as its parent chose it: its marker, its size
/// modulo 3, whether it can close, and the sizes of the levels in it so far, the parent's and
/// those around it, modulo 3. The level that joins no child into it ends it, and checks that
/// the sizes add up. The kind of the character after the run is in the state it leaves.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Join {
    marker: u8,
    size: u8,
    closes: bool,
    sizes: u8,
}

/// A closing run that attention is closed in, as its parent chose it, by its marker. The
/// level that closes no child in it writes it, after its children; going out, each level adds
/// its size and checks that its opening run pairs with the closing run (`Tag`), and the
/// outermost checks the whole.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Close {
    marker: u8,
}

/// What attention closed in its parent's closing run tells the parent of that run: the sizes
/// of the levels in it so far modulo 3, in the low bits, whether it can open (`OPENS`), and
/// which sizes of the whole run modulo 3 every level so far pairs with, as bits from `PAIRS`.
/// Zero for attention that is not closed in its parent's run.
type Tag = u8;
const OPENS: Tag = 1 << 2;
const PAIRS: u32 = 3;

/// The sizes modulo 3, as bits from `PAIRS`, of a closing run that can open if `opens` that
/// attention written with `way` pairs its opening run with.
fn pairing(way: &Way, opens: bool) -> Tag {
    (0..3usize)
        .filter(|&size| {
            pairs(
                way.byte,
                AttentionRun::whole(way.opening, true, way.closes),
                AttentionRun::whole(size, opens, true),
            )
        })
        .fold(0, |bits, size| bits | 1 << (PAIRS as usize + size))
}

/// What attention is planned inside: the open attention around it (`bit`), and the runs of its
/// parent it is written in, if any.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Context {
    open: u16,
    join: Option<Join>,
    close: Option<Close>,
}

/// The pieces of a list, as the attention around them chose: inside the attention in `open`,
/// with the first piece that writes anything joined into a run, and the last closed in one.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct List {
    open: u16,
    first: Option<(usize, Join)>,
    last: Option<(usize, Close)>,
}

impl List {
    fn context(&self, index: usize) -> Context {
        Context {
            open: self.open,
            join: self
                .first
                .filter(|&(at, _)| at == index)
                .map(|(_, join)| join),
            close: self
                .last
                .filter(|&(at, _)| at == index)
                .map(|(_, close)| close),
        }
    }
}

/// The ways to write attention at least cost, as the kind of the character before it, or for
/// attention joined into its parent's run the kind of the character after that run; its
/// marker; the kind of the character its closing run was chosen to flank before; its `Tag`;
/// and the cost.
type Costs = Vec<(u8, u8, u8, Tag, Cost)>;

/// One way to write attention: its sequence, whether its first child is joined into its opening
/// run and its last child closed in its closing run, its opening run's size modulo 3 (or its
/// size, unshared), whether that run can close and the kind of the character after it.
#[derive(Clone, Copy)]
struct Way {
    sequence: usize,
    marker: u8,
    byte: u8,
    size: usize,
    join: bool,
    close: bool,
    opening: usize,
    closes: bool,
    first: u8,
    cost: Cost,
}

/// How attention written one way reaches a result (`Planner::outcomes`): with its closing run
/// after its children's state, or with its last child, closed in its run, written from a state
/// with a result of its own.
#[derive(Clone, Copy)]
enum Detail {
    End(usize),
    Closed(usize, (u8, u8, u8, Tag, Cost)),
}

struct Planner<'p> {
    pieces: &'p [Piece],
    encode: bool,
    /// Which choices are considered.
    stage: Stage,
    /// For each attention piece and context, where `written` holds the least cost of writing
    /// it.
    costs: Vec<Vec<(Context, usize)>>,
    written: Vec<Costs>,
    /// For each range of pieces, by its start and the state before it, then its end and list,
    /// the least cost of writing them by the state they end in.
    tables: Vec<Vec<((usize, List), Table)>>,
}

/// Whether attention piece `piece` is emphasis or strong, which runs can hold together.
fn shares(piece: &Piece) -> bool {
    matches!(piece, Piece::Attention { sequences, .. } if !sequences[0].starts_with('~'))
}

/// The children of attention piece `piece`.
fn children(piece: &Piece) -> Range<usize> {
    let Piece::Attention { children, .. } = piece else {
        unreachable!("only attention has children");
    };
    children.clone()
}

impl Planner<'_> {
    /// The first or last piece of `list` that writes anything.
    fn end(&self, mut list: Range<usize>, last: bool) -> usize {
        let writes = |index: &usize| !matches!(self.pieces[*index], Piece::Empty);
        if last {
            list.rfind(writes)
        } else {
            list.find(writes)
        }
        .expect("attention holds a piece that writes something")
    }

    /// The kinds, as bits, that the first character written of `list` can be of, where
    /// attention it starts with writes its opening run, or where `joined` may also not, as it
    /// is joined into the one before.
    fn firsts(&self, list: Range<usize>, joined: bool) -> u8 {
        let index = self.end(list, false);
        match &self.pieces[index] {
            Piece::Text { first, .. } => {
                let first = edge(*first);
                let encoded = self.stage == Stage::Full && self.encode && first == OTHER;
                1 << first | u8::from(encoded) << PUNCTUATION
            }
            Piece::Other { first, .. } => 1 << edge(*first),
            piece => {
                1 << PUNCTUATION
                    | if joined && shares(piece) {
                        self.firsts(children(piece), true)
                    } else {
                        0
                    }
            }
        }
    }

    /// The ways to write attention piece `index` in `context`, in order of preference.
    fn ways(&self, index: usize, context: Context) -> Vec<Way> {
        let piece = &self.pieces[index];
        let Piece::Attention { sequences, .. } = piece else {
            unreachable!("only attention has ways");
        };
        let list = children(piece);
        let sharing = self.stage >= Stage::Shared && shares(piece);
        let joins = sharing && shares(&self.pieces[self.end(list.clone(), false)]);
        let closes = sharing && shares(&self.pieces[self.end(list.clone(), true)]);
        let mut ways = Vec::new();
        for (sequence, value) in sequences.iter().enumerate() {
            if sequence > 0 && self.stage == Stage::Preferred {
                break;
            }
            let (marker, byte) = marker(value);
            let size = value.len();
            if context.join.is_some_and(|join| join.marker != marker)
                || context.close.is_some_and(|close| close.marker != marker)
                // The parser takes two markers from a run that has them, so emphasis can share
                // only one of its runs with its parent's.
                || size == 1 && context.join.is_some() && context.close.is_some()
            {
                continue;
            }
            let sum = |sizes: u8| (usize::from(sizes) + size) % 3;
            for join in [false, true] {
                if join && !joins {
                    continue;
                }
                for close in [false, true] {
                    if close && !closes {
                        continue;
                    }
                    // The opening run: the parent's, or this node's, of a size that the levels
                    // joined into it make up, and before a character of a kind they start with.
                    let firsts = if join {
                        self.firsts(children(&self.pieces[self.end(list.clone(), false)]), true)
                    } else {
                        self.firsts(list.clone(), false)
                    };
                    let firsts = KINDS.iter().copied().filter(|&f| firsts & 1 << f != 0);
                    let (sizes, can_close) = match context.join {
                        Some(run) => {
                            if !join && sum(run.sizes) != usize::from(run.size) {
                                continue;
                            }
                            let size = usize::from(run.size);
                            (size..size + 1, Some(run.closes))
                        }
                        None if join => (0..3, None),
                        None => (size..size + 1, None),
                    };
                    let cost =
                        Cost::from(sequence != 0) + MERGE * (Cost::from(join) + Cost::from(close));
                    let openings = sizes.flat_map(|opening| {
                        firsts.clone().flat_map(move |first| {
                            [false, true]
                                .iter()
                                .copied()
                                .filter(move |&closes| match can_close {
                                    Some(can) => closes == can,
                                    None => !(closes && closes_open(context.open, marker, opening)),
                                })
                                .map(move |closes| (opening, closes, first))
                        })
                    });
                    for (opening, closes, first) in openings {
                        ways.push(Way {
                            sequence,
                            marker,
                            byte,
                            size,
                            join,
                            close,
                            opening,
                            closes,
                            first,
                            cost,
                        });
                    }
                }
            }
        }
        ways
    }

    /// How the children of attention piece `index`, in `context`, are written with `way`.
    fn inside(&self, index: usize, context: Context, way: &Way) -> List {
        let list = children(&self.pieces[index]);
        let size = u8::try_from(way.size).expect("a sequence is short");
        let open = if context.join.is_some() {
            context.open
        } else {
            context.open | bit(way.marker, way.opening)
        };
        let small = |value: usize| u8::try_from(value).expect("a size is small");
        List {
            open,
            first: way.join.then(|| {
                let sizes = context.join.map_or(0, |run| run.sizes);
                let join = Join {
                    marker: way.marker,
                    size: small(way.opening),
                    closes: way.closes,
                    sizes: (sizes + size) % 3,
                };
                (self.end(list.clone(), false), join)
            }),
            last: way
                .close
                .then(|| (self.end(list.clone(), true), Close { marker: way.marker })),
        }
    }

    /// The least cost of writing `pieces` of `list` from state `start`, by the state they end
    /// in, working it out the first time.
    fn table(&mut self, pieces: Range<usize>, list: &List, start: usize) -> Table {
        let key = (pieces.end, *list);
        let at = pieces.start * STATES + start;
        if let Some((_, table)) = self.tables[at].iter().find(|entry| entry.0 == key) {
            return *table;
        }

        let mut current = [INFEASIBLE; STATES];
        current[start] = 0;
        let table = self.sequence(pieces, list, current);
        self.tables[at].push((key, table));
        table
    }

    /// Each result of writing attention piece `index` in `context` with `way`: the kind of the
    /// character before it (`Costs`), the kind its closing run was chosen to flank before, its
    /// `Tag` and its cost, with how it is reached.
    fn outcomes(
        &mut self,
        index: usize,
        context: Context,
        way: &Way,
        visit: &mut dyn FnMut(u8, u8, Tag, Cost, Detail),
    ) {
        let list = children(&self.pieces[index]);
        let inside = self.inside(index, context, way);
        let start = state(RUN + way.marker, 1 + way.first);
        let size = u8::try_from(way.size % 3).expect("a remainder is small");
        let pairings = [pairing(way, false), pairing(way, true)];
        // A level closed in its parent's run passes the run on; the outermost checks it.
        let mut result = |tag: Tag, cost: Cost, detail: Detail, after: u8| {
            let tag = if context.close.is_some() {
                tag
            } else if tag & 1 << (PAIRS + u32::from(tag & 3)) != 0 {
                0
            } else {
                return;
            };
            for before in Self::befores(context, way) {
                visit(before, after, tag, cost + way.cost, detail);
            }
        };
        if way.close {
            let last = self.end(list.clone(), true);
            let table = self.table(list.start..last, &inside, start);
            let child = inside.context(last);
            let slot = self.costs(last, child);
            for (from, &cost) in table.iter().enumerate() {
                if cost == INFEASIBLE {
                    continue;
                }
                let Some(before) = Self::entry(child, from, way.marker) else {
                    continue;
                };
                for &entry in &self.written[slot] {
                    let (kind, marker, after, tag, added) = entry;
                    if kind != before || marker != way.marker {
                        continue;
                    }
                    let opens = tag & OPENS != 0;
                    let sizes = ((tag & 3) + size) % 3;
                    let tag = sizes | tag & OPENS | tag & pairings[usize::from(opens)];
                    result(tag, cost + added, Detail::Closed(from, entry), after);
                }
            }
            return;
        }
        let table = self.table(list, &inside, start);
        for (end, &cost) in table.iter().enumerate() {
            let (last, required) = split(end);
            if cost == INFEASIBLE
                || !(required == NONE || required == 1 + PUNCTUATION)
                || last == RUN + way.marker
            {
                continue;
            }
            for after in KINDS {
                let (opens, closes) = sides(way.byte, kind(last), kind(after));
                if closes {
                    let tag = size | if opens { OPENS } else { 0 } | pairings[usize::from(opens)];
                    result(tag, cost, Detail::End(end), after);
                }
            }
        }
    }

    /// The kind of character before attention in `context` with `marker` after state `from`
    /// (`Costs`), where it can be written there: attention joined into its parent's run follows
    /// it directly, and other attention starts with its own run, which joins no other.
    fn entry(context: Context, from: usize, marker: u8) -> Option<u8> {
        let (last, required) = split(from);
        match context.join {
            Some(run) => (last == RUN + run.marker && required != NONE).then(|| required - 1),
            None => ((required == NONE || required == 1 + PUNCTUATION) && last != RUN + marker)
                .then(|| edge(kind(last))),
        }
    }

    /// The kinds of character before attention written with `way` in `context` that let its
    /// opening run open, and close as `way` says, or where it is joined into its parent's run,
    /// the kind of the character after that run (`Costs`).
    fn befores(context: Context, way: &Way) -> impl Iterator<Item = u8> + '_ {
        let joined = context.join.is_some();
        KINDS.iter().copied().filter(move |&before| {
            if joined {
                before == way.first
            } else {
                sides(way.byte, kind(before), kind(way.first)) == (true, way.closes)
            }
        })
    }

    /// The least cost of writing attention piece `index` in `context`, working it out the first
    /// time.
    fn costs(&mut self, index: usize, context: Context) -> usize {
        if let Some(&(_, slot)) = self.costs[index].iter().find(|entry| entry.0 == context) {
            return slot;
        }

        let mut costs: Costs = Vec::new();
        for way in self.ways(index, context) {
            let marker = way.marker;
            self.outcomes(index, context, &way, &mut |before, after, tag, cost, _| {
                let key = (before, marker, after, tag);
                match costs
                    .iter_mut()
                    .find(|entry| (entry.0, entry.1, entry.2, entry.3) == key)
                {
                    Some(entry) => entry.4 = entry.4.min(cost),
                    None => costs.push((before, marker, after, tag, cost)),
                }
            });
        }
        self.written.push(costs);
        self.costs[index].push((context, self.written.len() - 1));
        self.written.len() - 1
    }

    /// Each way to write piece `index` of `list` after state `from`, in order of preference: the
    /// state it leaves, its cost and its choice, for attention its marker and the kind its
    /// closing run was chosen to flank before.
    fn step(
        &mut self,
        index: usize,
        list: &List,
        from: usize,
        visit: &mut dyn FnMut(usize, Cost, Choice),
    ) {
        let slot = self.slot(index, list);
        self.step_with(index, list, slot, from, visit);
    }

    /// Where `written` holds the costs of piece `index` of `list` if it is attention.
    fn slot(&mut self, index: usize, list: &List) -> usize {
        if matches!(self.pieces[index], Piece::Attention { .. }) {
            self.costs(index, list.context(index))
        } else {
            usize::MAX
        }
    }

    /// `step`, with the `slot` of the piece.
    fn step_with(
        &self,
        index: usize,
        list: &List,
        slot: usize,
        from: usize,
        visit: &mut dyn FnMut(usize, Cost, Choice),
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
                let encodes = |kind: u8| self.stage == Stage::Full && self.encode && kind == OTHER;
                for encode_first in [false, true] {
                    if encode_first && !encodes(first) {
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
                            choice,
                        );
                        continue;
                    }
                    for encode_last in [false, true] {
                        if encode_last && !encodes(end) {
                            continue;
                        }
                        visit(
                            state(if encode_last { PUNCTUATION } else { end }, NONE),
                            ENCODING * (Cost::from(encode_first) + Cost::from(encode_last)),
                            (Choice::from(encode_first) * ENCODE_FIRST)
                                | (Choice::from(encode_last) * ENCODE_LAST),
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
            Piece::Empty => visit(from, 0, EMPTY),
            Piece::Attention { sequences, .. } => {
                let context = list.context(index);
                for sequence in sequences.iter() {
                    let (marker, _) = marker(sequence);
                    let Some(before) = Self::entry(context, from, marker) else {
                        continue;
                    };
                    for &(kind, found, after, _, cost) in &self.written[slot] {
                        if kind == before && found == marker {
                            visit(
                                state(RUN + marker, 1 + after),
                                cost,
                                Choice::from(marker) << 2 | Choice::from(after),
                            );
                        }
                    }
                }
            }
        }
    }

    /// The least cost of writing the pieces of `list`, from each state at the cost in
    /// `current`, by the state they end in.
    fn sequence(&mut self, pieces: Range<usize>, list: &List, mut current: Table) -> Table {
        for index in pieces {
            if matches!(self.pieces[index], Piece::Empty) {
                continue;
            }
            let slot = self.slot(index, list);
            let mut next = [INFEASIBLE; STATES];
            let mut visit = |from: usize, cost: Cost| {
                if cost != INFEASIBLE {
                    self.step_with(index, list, slot, from, &mut |to, added, _| {
                        next[to] = next[to].min(cost + added);
                    });
                }
            };
            // Text is written the same way after any states that require the same of it.
            if matches!(self.pieces[index], Piece::Text { .. }) {
                for required in 0..4 {
                    let cost = (0..6).map(|last| current[state(last, required)]).min();
                    visit(state(0, required), cost.unwrap_or(INFEASIBLE));
                }
            } else {
                for (from, &cost) in current.iter().enumerate() {
                    visit(from, cost);
                }
            }
            current = next;
        }
        current
    }

    /// The least cost of writing the rest of `pieces`, to a state that `ends` costs other than
    /// `INFEASIBLE`, from each state before each piece, and last `ends` itself.
    fn rest(&mut self, pieces: Range<usize>, list: &List, ends: &Table) -> Vec<Table> {
        let mut rest = alloc::vec![*ends; pieces.len() + 1];
        for (offset, index) in pieces.enumerate().rev() {
            let after = rest[offset + 1];
            let slot = self.slot(index, list);
            let mut costs = [INFEASIBLE; STATES];
            let cost_from = |from: usize| {
                let mut cost = INFEASIBLE;
                self.step_with(index, list, slot, from, &mut |to, added, _| {
                    if after[to] != INFEASIBLE {
                        cost = cost.min(added + after[to]);
                    }
                });
                cost
            };
            // As in `sequence`, text is written the same way after many states, and so is
            // attention that writes its own opening run.
            match self.pieces[index] {
                Piece::Text { .. } => {
                    for required in 0..4 {
                        let cost = cost_from(state(0, required));
                        for last in 0..6 {
                            costs[state(last, required)] = cost;
                        }
                    }
                }
                Piece::Attention { .. } if list.context(index).join.is_none() => {
                    for last in 0..6 {
                        let cost = cost_from(state(last, NONE));
                        costs[state(last, NONE)] = cost;
                        costs[state(last, 1 + PUNCTUATION)] = cost;
                    }
                }
                _ => {
                    for (from, cost) in costs.iter_mut().enumerate() {
                        *cost = cost_from(from);
                    }
                }
            }
            rest[offset] = costs;
        }
        rest
    }

    /// Record the choices that write `pieces` of `list` from `start` to a state that `ends`
    /// costs other than `INFEASIBLE`, at least cost, or return `false` where there are none.
    /// Where several cost the least, each piece in turn takes its most preferred option that
    /// still can.
    fn assign(
        &mut self,
        pieces: Range<usize>,
        list: &List,
        start: usize,
        ends: &Table,
        choices: &mut [Choice],
    ) -> bool {
        let rest = self.rest(pieces.clone(), list, ends);
        if rest[0][start] == INFEASIBLE {
            return false;
        }
        let mut at = start;
        for (offset, index) in pieces.enumerate() {
            let goal = rest[offset][at];
            let after = rest[offset + 1];
            let mut taken = None;
            self.step(index, list, at, &mut |to, added, choice| {
                if taken.is_none() && after[to] != INFEASIBLE && added + after[to] == goal {
                    taken = Some((to, added, choice));
                }
            });
            let (to, cost, choice) = taken.expect("a least cost is reached");
            let from = at;
            at = to;
            if matches!(self.pieces[index], Piece::Attention { .. }) {
                let context = list.context(index);
                let marker = u8::try_from(choice >> 2).expect("a marker is small");
                let before = Self::entry(context, from, marker).expect("the attention was costed");
                let after = u8::try_from(choice & 3).expect("a kind is small");
                let result = (before, marker, after, 0, cost);
                self.assign_attention(index, context, result, choices);
            } else {
                choices[index] = choice;
            }
        }
        true
    }

    /// Record the choices for attention piece `index` in `context` that reach `result`
    /// (`Costs`), and for its children: the first way in order of preference that does.
    fn assign_attention(
        &mut self,
        index: usize,
        context: Context,
        result: (u8, u8, u8, Tag, Cost),
        choices: &mut [Choice],
    ) {
        let (before, marker, after, tag, cost) = result;
        let list = children(&self.pieces[index]);
        for way in self.ways(index, context) {
            if way.marker != marker || way.cost > cost {
                continue;
            }
            let mut found = None;
            self.outcomes(
                index,
                context,
                &way,
                &mut |kind, end, written, total, detail| {
                    if found.is_none() && (kind, end, written, total) == (before, after, tag, cost)
                    {
                        found = Some(detail);
                    }
                },
            );
            let Some(detail) = found else {
                continue;
            };
            choices[index] = Spelling {
                sequence: way.sequence,
                joined: context.join.is_some(),
                closed: context.close.is_some(),
            }
            .choice();
            let inside = self.inside(index, context, &way);
            let start = state(RUN + way.marker, 1 + way.first);
            let mut ends = [INFEASIBLE; STATES];
            let assigned = match detail {
                Detail::End(end) => {
                    ends[end] = 0;
                    self.assign(list, &inside, start, &ends, choices)
                }
                Detail::Closed(from, entry) => {
                    ends[from] = 0;
                    let last = self.end(list.clone(), true);
                    let assigned = self.assign(list.start..last, &inside, start, &ends, choices);
                    self.assign_attention(last, inside.context(last), entry, choices);
                    assigned
                }
            };
            debug_assert!(assigned, "the way was costed");
            return;
        }
        unreachable!("the attention was costed");
    }
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
