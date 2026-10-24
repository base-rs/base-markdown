// Released under the MIT License.
// Copyright, 2022-2025, by Titus Wormer.
// Copyright, 2023, by Ophir Lojkine.
// Copyright, 2023, by pinkforest(she/her).
// Copyright, 2026, by Samuel Williams.

//! A tokenizer glues states from the state machine together.
//!
//! It facilitates everything needed to turn bytes into events with a state
//! machine.
//! It also enables the logic needed for parsing markdown, such as an
//! [`attempt`][] to try and parse something, which can succeed or, when
//! unsuccessful, revert the attempt.
//!
//! [`attempt`]: Tokenizer::attempt
use crate::event::{Content, Event, Kind, Link, Name, Point, VOID_EVENTS};
use crate::parser::ParseState;
use crate::resolve::{call as call_resolve, Name as ResolveName};
use crate::state::{call, Name as StateName, State};
use crate::subtokenize::Subresult;

#[cfg(feature = "log")]
use crate::util::char::format_byte_opt;

use crate::util::{constant::TAB_SIZE, edit_map::EditMap};
use alloc::{boxed::Box, string::String, vec, vec::Vec};

/// Containers.
///
/// Containers are found when tokenizing
/// [document content][crate::construct::document].
/// They parse a portion at the start of one or more lines.
/// The rest of those lines is a different content type (specifically, flow),
/// which they “contain”.
#[derive(Debug, Eq, PartialEq)]
pub enum Container {
    /// [Block quote][crate::construct::block_quote].
    BlockQuote,
    /// [List item][crate::construct::list_item].
    ListItem,
    /// [GFM: Footnote definition][crate::construct::gfm_footnote_definition].
    GfmFootnoteDefinition,
}

/// Info used to tokenize a container.
///
/// Practically, these fields are only used for list items.
#[derive(Debug)]
pub struct ContainerState {
    /// Kind.
    pub kind: Container,
    /// Whether the first line was blank.
    pub blank_initial: bool,
    /// Size.
    pub size: usize,
}

/// How to handle a byte.
#[derive(Debug, PartialEq)]
enum ByteAction {
    /// This is a normal byte.
    ///
    /// Includes replaced bytes.
    Normal(u8),
    /// This byte must be ignored.
    Ignore,
    /// This is a new byte.
    Insert(u8),
}

/// Label start kind.
#[derive(Debug, PartialEq, Eq)]
pub enum LabelKind {
    /// Label (image) start.
    ///
    /// ```markdown
    /// > | a ![b] c
    ///       ^^
    /// ```
    ///
    /// Construct: [Label start (image)][crate::construct::label_start_image].
    Image,
    /// Label (image) link.
    ///
    /// ```markdown
    /// > | a [b] c
    ///       ^
    /// ```
    ///
    /// Construct: [Label start (link)][crate::construct::label_start_link].
    Link,
    /// GFM: Label (footnote) link.
    ///
    /// ```markdown
    /// > | a [^b] c
    ///       ^^
    /// ```
    ///
    /// Construct: [GFM: Label start (footnote)][crate::construct::gfm_label_start_footnote].
    GfmFootnote,
    /// GFM: Label (footnote) link, not matching a footnote definition, so
    /// handled as a label (link) start.
    ///
    /// ```markdown
    /// > | a [^b](c) d
    ///       ^^
    /// ```
    ///
    /// Construct: [Label end][crate::construct::label_end].
    GfmUndefinedFootnote,
}

/// Label start, looking for an end.
#[derive(Debug)]
pub struct LabelStart {
    /// Kind of start.
    pub kind: LabelKind,
    /// Indices of where the label starts and ends in `events`.
    pub start: (usize, usize),
    /// A boolean used internally to figure out if a (link) label start can’t
    /// be used anymore (because it would contain another link).
    /// That link start is still looking for a balanced closing bracket though,
    /// so we can’t remove it just yet.
    pub inactive: bool,
}

/// Valid label.
#[derive(Debug)]
pub struct Label {
    pub kind: LabelKind,
    /// Indices of label start.
    pub start: (usize, usize),
    /// Indices of label end.
    pub end: (usize, usize),
}

/// Different kinds of attempts.
#[derive(Debug, PartialEq)]
enum AttemptKind {
    /// Discard what was tokenized when unsuccessful.
    Attempt,
    /// Discard always.
    Check,
}

/// How to handle [`State::Ok`][] or [`State::Nok`][].
#[derive(Debug)]
struct Attempt {
    /// Where to go to when successful.
    ok: State,
    /// Where to go to when unsuccessful.
    nok: State,
    /// Kind of attempt.
    kind: AttemptKind,
    /// If needed, the progress to revert to.
    ///
    /// It is not needed to discard an [`AttemptKind::Attempt`] that has a
    /// `nok` of [`State::Nok`][], because that means it is used in *another*
    /// attempt, which will receive that `Nok`, and has to handle it.
    progress: Option<Progress>,
}

/// The internal state of a tokenizer.
///
/// Not to be confused with states from the state machine, this instead is all
/// the information on where we currently are and what’s going on.
#[derive(Clone, Debug)]
struct Progress {
    /// Length of `events`.
    ///
    /// It’s not allowed to remove events, so reverting will just pop stuff off.
    events_len: usize,
    /// Length of the stack.
    ///
    /// It’s not allowed to decrease the stack in an attempt.
    stack_len: usize,
    /// Previous code.
    previous: Option<u8>,
    /// Current code.
    current: Option<u8>,
    /// Current place in the file.
    point: Point,
    /// Physical start of the current line, needed when rewinding to just after EOL.
    line_start_index: usize,
}

/// A lot of shared fields used to tokenize things.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug)]
pub struct TokenizeState<'a> {
    // Couple complex fields used to tokenize the document.
    /// Tokenizer, used to tokenize flow in document.
    pub document_child: Option<Box<Tokenizer<'a>>>,
    /// State, used to tokenize containers.
    pub document_child_state: Option<State>,
    /// Stack of currently active containers.
    pub document_container_stack: Vec<ContainerState>,
    /// How many active containers continued.
    pub document_continued: usize,
    /// Index of last `data`.
    pub document_data_index: Option<usize>,
    /// Container exits by line number.
    pub document_exits: Vec<Option<Vec<Event>>>,
    /// Whether the previous flow was a paragraph or a definition.
    pub document_lazy_accepting_before: bool,
    /// Whether this is the first paragraph (potentially after definitions) in
    /// a list item.
    /// Used for GFM task list items.
    pub document_at_first_paragraph_of_list_item: bool,

    // Couple of very frequent settings for parsing whitespace.
    pub space_or_tab_eol_content: Option<Content>,
    pub space_or_tab_eol_connect: bool,
    pub space_or_tab_eol_ok: bool,
    pub space_or_tab_connect: bool,
    pub space_or_tab_content: Option<Content>,
    pub space_or_tab_min: usize,
    pub space_or_tab_max: usize,
    pub space_or_tab_size: usize,
    pub space_or_tab_token: Name,

    // Couple of media related fields.
    /// List of usable label starts.
    ///
    /// Used when tokenizing [text content][crate::construct::text].
    pub label_starts: Vec<LabelStart>,
    /// List of unusable label starts.
    ///
    /// Used when tokenizing [text content][crate::construct::text].
    pub label_starts_loose: Vec<LabelStart>,
    /// Stack of images and links.
    ///
    /// Used when tokenizing [text content][crate::construct::text].
    pub labels: Vec<Label>,

    /// List of defined definition identifiers.
    pub definitions: Vec<String>,
    /// List of defined GFM footnote definition identifiers.
    pub gfm_footnote_definitions: Vec<String>,

    /// Whether to connect events.
    pub connect: bool,
    /// Marker.
    pub marker: u8,
    /// Secondary marker.
    pub marker_b: u8,
    /// Several markers.
    pub markers: u128,
    /// Whether something was seen.
    pub seen: bool,
    /// Size.
    pub size: usize,
    /// Secondary size.
    pub size_b: usize,
    /// Tertiary size.
    pub size_c: usize,
    /// Index.
    pub start: usize,
    /// Index.
    pub end: usize,
    /// Shallowest positive indentation in the current type 6 or 7 HTML block.
    pub html_flow_indent: usize,
    /// Number of blank lines seen while checking HTML block continuation.
    pub html_flow_blank_lines: usize,
    /// Column at which the current candidate HTML continuation line begins.
    pub html_flow_blank_line_base: usize,
    /// Slot for an event name.
    pub token_1: Name,
    /// Slot for an event name.
    pub token_2: Name,
    /// Slot for an event name.
    pub token_3: Name,
    /// Slot for an event name.
    pub token_4: Name,
    /// Slot for an event name.
    pub token_5: Name,
    /// Slot for an event name.
    pub token_6: Name,
}

/// A tokenizer itself.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug)]
pub struct Tokenizer<'a> {
    /// Jump between line endings.
    column_start: Vec<(usize, usize)>,
    // First line where this tokenizer starts.
    first_line: usize,
    /// Current point after the last line ending (excluding jump).
    line_start: Point,
    /// Track whether the current byte is already consumed (`true`) or expected
    /// to be consumed (`false`).
    ///
    /// Tracked to make sure everything’s valid.
    consumed: bool,
    /// Stack of how to handle attempts.
    attempts: Vec<Attempt>,
    /// Current byte.
    pub current: Option<u8>,
    /// Previous byte.
    pub previous: Option<u8>,
    /// Current relative and absolute place in the file.
    pub point: Point,
    /// Semantic labels.
    pub events: Vec<Event>,
    /// Hierarchy of semantic labels.
    ///
    /// Tracked to make sure everything’s valid.
    pub stack: Vec<Name>,
    /// Edit map, to batch changes.
    pub map: EditMap,
    /// List of resolvers.
    pub resolvers: Vec<ResolveName>,
    /// Shared parsing state across tokenizers.
    pub parse_state: &'a ParseState<'a>,
    /// A lot of shared fields used to tokenize things.
    pub tokenize_state: TokenizeState<'a>,
    /// Whether we would be interrupting something.
    ///
    /// Used when tokenizing [flow content][crate::construct::flow].
    pub interrupt: bool,
    /// Whether containers cannot “pierce” into the current construct.
    ///
    /// Used when tokenizing [document content][crate::construct::document].
    pub concrete: bool,
    /// Whether this row is piercing into the current construct with more
    /// containers.
    ///
    /// Used when tokenizing [document content][crate::construct::document].
    pub pierce: bool,
    /// Whether this line is lazy: there are less containers than before.
    pub lazy: bool,
}

/// Scratch allocations that can outlive a borrow of `ParseState`. No source or parser references escape here.
#[derive(Debug)]
pub(crate) struct TokenizerBuffers {
    pub events: Vec<Event>,
    stack: Vec<Name>,
    attempts: Vec<Attempt>,
    column_start: Vec<(usize, usize)>,
    resolvers: Vec<ResolveName>,
    label_starts: Vec<LabelStart>,
    label_starts_loose: Vec<LabelStart>,
    labels: Vec<Label>,
    definitions: Vec<String>,
    footnotes: Vec<String>,
    map: EditMap,
}

impl Default for TokenizerBuffers {
    fn default() -> Self {
        Self {
            events: Vec::new(),
            stack: Vec::new(),
            attempts: Vec::new(),
            column_start: Vec::new(),
            resolvers: Vec::new(),
            label_starts: Vec::new(),
            label_starts_loose: Vec::new(),
            labels: Vec::new(),
            definitions: Vec::new(),
            footnotes: Vec::new(),
            map: EditMap::new(),
        }
    }
}

impl<'a> Tokenizer<'a> {
    /// Create a new tokenizer.
    pub fn new(point: Point, parse_state: &'a ParseState) -> Tokenizer<'a> {
        Tokenizer {
            previous: None,
            current: None,
            // To do: reserve size when feeding?
            column_start: vec![],
            first_line: point.line,
            line_start: point.clone(),
            consumed: true,
            attempts: vec![],
            point,
            stack: vec![],
            events: vec![],
            parse_state,
            tokenize_state: TokenizeState {
                connect: false,
                document_container_stack: vec![],
                document_exits: vec![],
                document_continued: 0,
                document_lazy_accepting_before: false,
                document_data_index: None,
                document_child_state: None,
                document_child: None,
                document_at_first_paragraph_of_list_item: false,
                definitions: vec![],
                gfm_footnote_definitions: vec![],

                end: 0,
                html_flow_indent: 0,
                html_flow_blank_lines: 0,
                html_flow_blank_line_base: 0,
                label_starts: vec![],
                label_starts_loose: vec![],
                marker: 0,
                marker_b: 0,
                markers: 0,
                labels: vec![],
                seen: false,
                size: 0,
                size_b: 0,
                size_c: 0,
                space_or_tab_eol_content: None,
                space_or_tab_eol_connect: false,
                space_or_tab_eol_ok: false,
                space_or_tab_connect: false,
                space_or_tab_content: None,
                space_or_tab_min: 0,
                space_or_tab_max: 0,
                space_or_tab_size: 0,
                space_or_tab_token: Name::SpaceOrTab,
                start: 0,
                token_1: Name::Data,
                token_2: Name::Data,
                token_3: Name::Data,
                token_4: Name::Data,
                token_5: Name::Data,
                token_6: Name::Data,
            },
            map: EditMap::new(),
            interrupt: false,
            pierce: false,
            concrete: false,
            lazy: false,
            resolvers: vec![],
        }
    }

    /// Reborrow parse state while retaining the previous inline region's scratch allocations.
    pub(crate) fn with_buffers(
        point: Point,
        parse_state: &'a ParseState,
        mut buffers: TokenizerBuffers,
    ) -> Self {
        let mut tokenizer = Self::new(point, parse_state);
        macro_rules! restore {
            ($buffer:ident, $($field:ident).+) => {
                buffers.$buffer.clear();
                core::mem::swap(&mut tokenizer.$($field).+, &mut buffers.$buffer);
            };
        }
        restore!(events, events);
        restore!(stack, stack);
        restore!(attempts, attempts);
        restore!(column_start, column_start);
        restore!(resolvers, resolvers);
        restore!(label_starts, tokenize_state.label_starts);
        restore!(label_starts_loose, tokenize_state.label_starts_loose);
        restore!(labels, tokenize_state.labels);
        restore!(definitions, tokenize_state.definitions);
        restore!(footnotes, tokenize_state.gfm_footnote_definitions);
        tokenizer.map = buffers.map;
        tokenizer
    }

    /// Release source borrows while preserving reusable inline scratch storage.
    pub(crate) fn into_buffers(self) -> TokenizerBuffers {
        TokenizerBuffers {
            events: self.events,
            stack: self.stack,
            attempts: self.attempts,
            column_start: self.column_start,
            resolvers: self.resolvers,
            label_starts: self.tokenize_state.label_starts,
            label_starts_loose: self.tokenize_state.label_starts_loose,
            labels: self.tokenize_state.labels,
            definitions: self.tokenize_state.definitions,
            footnotes: self.tokenize_state.gfm_footnote_definitions,
            map: self.map,
        }
    }

    /// Start another subcontent chunk while retaining scratch allocations from the previous one.
    pub fn reset(&mut self, point: Point) {
        let mut next = Self::new(point, self.parse_state);
        macro_rules! reuse {
            ($($field:ident).+) => {
                self.$($field).+.clear();
                core::mem::swap(&mut self.$($field).+, &mut next.$($field).+);
            };
        }
        reuse!(events);
        reuse!(stack);
        reuse!(attempts);
        reuse!(column_start);
        reuse!(resolvers);
        reuse!(tokenize_state.label_starts);
        reuse!(tokenize_state.label_starts_loose);
        reuse!(tokenize_state.labels);
        reuse!(tokenize_state.definitions);
        reuse!(tokenize_state.gfm_footnote_definitions);
        // A flushed tokenizer has consumed all pending edits.
        core::mem::swap(&mut self.map, &mut next.map);
        *self = next;
    }

    /// Test the ASCII delimiter bitset without searching a marker slice.
    pub fn is_data_marker(&self, byte: u8) -> bool {
        byte < 128 && self.tokenize_state.markers & (1 << byte) != 0
    }

    /// Register a resolver.
    pub fn register_resolver(&mut self, name: ResolveName) {
        if !self.resolvers.contains(&name) {
            self.resolvers.push(name);
        }
    }

    /// Register a resolver, before others.
    pub fn register_resolver_before(&mut self, name: ResolveName) {
        if !self.resolvers.contains(&name) {
            self.resolvers.insert(0, name);
        }
    }

    /// Define a jump between two places.
    ///
    /// This defines to which future index we move after a line ending.
    pub fn define_skip(&mut self, mut point: Point) {
        move_point_back(self, &mut point);

        let info = (point.index, point.vs);

        #[cfg(feature = "log")]
        log::trace!("position: define skip: {:?} -> ({:?})", point.line, info);

        let at = point.line - self.first_line;

        if at >= self.column_start.len() {
            self.column_start.push(info);
        } else {
            self.column_start[at] = info;
        }

        self.account_for_potential_skip();
    }

    /// Increment the current positional info if we’re right after a line
    /// ending, which has a skip defined.
    fn account_for_potential_skip(&mut self) {
        let at = self.point.line - self.first_line;

        if self.point.column == 1 && at != self.column_start.len() {
            self.move_to(self.column_start[at]);
        }
    }

    /// Prepare for a next byte to get consumed.
    fn expect(&mut self, byte: Option<u8>) {
        debug_assert!(self.consumed, "expected previous byte to be consumed");
        self.consumed = false;
        self.current = byte;
    }

    /// Consume the current byte.
    /// Each state function is expected to call this to signal that this code is
    /// used, or call a next function.
    pub fn consume(&mut self) {
        debug_assert!(!self.consumed, "expected code to *not* have been consumed: this might be because `State::Retry(x)` instead of `State::Next(x)` was returned");
        self.move_one();

        self.previous = self.current;
        // While we’re not at eof, it is at least better to not have the
        // same current code as `previous` *and* `current`.
        self.current = None;
        // Mark as consumed.
        self.consumed = true;
    }

    /// Move to the next (virtual) byte.
    fn move_one(&mut self) {
        match byte_action(self.parse_state.bytes, &self.point) {
            ByteAction::Ignore => {
                self.point.index += 1;
            }
            ByteAction::Insert(byte) => {
                self.previous = Some(byte);
                self.point.column += 1;
                self.point.vs += 1;
            }
            ByteAction::Normal(byte) => {
                self.previous = Some(byte);
                self.point.vs = 0;
                self.point.index += 1;

                if byte == b'\n' {
                    // Root tokenizers have no caller-defined first-line skip. Seed its
                    // slot before caching the next line, so speculative lookahead can
                    // rewind without treating line two as the start of line one.
                    if self.column_start.is_empty() {
                        self.column_start
                            .push((self.line_start.index, self.line_start.vs));
                    }
                    self.point.line += 1;
                    self.point.column = 1;

                    if self.point.line - self.first_line + 1 > self.column_start.len() {
                        self.column_start.push((self.point.index, self.point.vs));
                    }

                    self.line_start = self.point.clone();

                    self.account_for_potential_skip();

                    #[cfg(feature = "log")]
                    log::trace!("position: after eol: `{:?}`", self.point);
                } else {
                    self.point.column += 1;
                }
            }
        }
    }

    /// Move (virtual) bytes.
    fn move_to(&mut self, to: (usize, usize)) {
        let (to_index, to_vs) = to;
        while self.point.index < to_index || self.point.index == to_index && self.point.vs < to_vs {
            self.move_one();
        }
    }

    /// Mark the start of a semantic label.
    pub fn enter(&mut self, name: Name) {
        enter_impl(self, name, None);
    }

    /// Enter with a link.
    pub fn enter_link(&mut self, name: Name, link: Link) {
        enter_impl(self, name, Some(link));
    }

    /// Mark the end of a semantic label.
    pub fn exit(&mut self, name: Name) {
        let current = self.stack.pop().expect("cannot close w/o open tokens");

        debug_assert_eq!(current, name, "expected exit event to match current event");

        let previous = self.events.last().expect("cannot close w/o open event");
        let mut point = self.point.clone();

        debug_assert!(
            current != previous.name
                || previous.point.index != point.index
                || previous.point.vs != point.vs,
            "expected non-empty event"
        );

        if VOID_EVENTS.iter().any(|d| d == &name) {
            debug_assert_eq!(
                current, previous.name,
                "expected event to be void, instead of including something"
            );
        }

        // A bit weird, but if we exit right after a line ending, we *don’t* want to consider
        // potential skips.
        if matches!(self.previous, Some(b'\n')) {
            point = self.line_start.clone();
        } else {
            move_point_back(self, &mut point);
        }

        #[cfg(feature = "log")]
        log::debug!("exit:    `{name:?}`");

        let event = Event {
            kind: Kind::Exit,
            name,
            point,
            link: None,
        };
        self.events.push(event);
    }

    /// Capture the tokenizer progress.
    fn capture(&mut self) -> Progress {
        Progress {
            previous: self.previous,
            current: self.current,
            point: self.point.clone(),
            line_start_index: self.line_start.index,
            events_len: self.events.len(),
            stack_len: self.stack.len(),
        }
    }

    /// Apply tokenizer progress.
    fn free(&mut self, previous: Progress) {
        self.previous = previous.previous;
        self.current = previous.current;
        self.point = previous.point;
        // Only an exit immediately after EOL reads line_start. Other exits
        // use point directly; the next EOL refreshes line_start before reuse.
        if self.previous == Some(b'\n') {
            self.line_start = Point {
                line: self.point.line,
                column: 1,
                index: previous.line_start_index,
                vs: 0,
            };
        }
        debug_assert!(
            self.events.len() >= previous.events_len,
            "expected to restore less events than before"
        );
        self.events.truncate(previous.events_len);
        debug_assert!(
            self.stack.len() >= previous.stack_len,
            "expected to restore less stack items than before"
        );
        self.stack.truncate(previous.stack_len);
    }

    /// Stack an attempt, moving to `ok` on [`State::Ok`][] and `nok` on
    /// [`State::Nok`][], reverting in both cases.
    pub fn check(&mut self, ok: State, nok: State) {
        // Always capture (and restore) when checking.
        // No need to capture (and restore) when `nok` is `State::Nok`, because the
        // parent attempt will do it.
        let progress = Some(self.capture());
        let attempt = Attempt {
            kind: AttemptKind::Check,
            progress,
            ok,
            nok,
        };
        self.attempts.push(attempt);
    }

    /// Stack an attempt, moving to `ok` on [`State::Ok`][] and `nok` on
    /// [`State::Nok`][], reverting in the latter case.
    pub fn attempt(&mut self, ok: State, nok: State) {
        // Always capture (and restore) when checking.
        // No need to capture (and restore) when `nok` is `State::Nok`, because the
        // parent attempt will do it.
        let progress = if matches!(nok, State::Nok) {
            None
        } else {
            Some(self.capture())
        };

        let attempt = Attempt {
            kind: AttemptKind::Attempt,
            progress,
            ok,
            nok,
        };
        self.attempts.push(attempt);
    }

    /// Tokenize.
    pub fn push(&mut self, from: (usize, usize), to: (usize, usize), state: State) -> State {
        push_impl(self, from, to, state, false)
    }

    /// Flush.
    pub fn flush(&mut self, state: State, resolve: bool) -> Subresult {
        let to = (self.point.index, self.point.vs);
        let state = push_impl(self, to, to, state, true);

        state.assert_ok();

        let mut value = Subresult {
            done: false,
            gfm_footnote_definitions: self.tokenize_state.gfm_footnote_definitions.split_off(0),
            definitions: self.tokenize_state.definitions.split_off(0),
        };

        if resolve {
            let resolvers = self.resolvers.split_off(0);
            let mut index = 0;
            let defs = &mut value.definitions;
            let fn_defs = &mut value.gfm_footnote_definitions;
            while index < resolvers.len() {
                if let Some(mut result) = call_resolve(self, resolvers[index]) {
                    fn_defs.append(&mut result.gfm_footnote_definitions);
                    defs.append(&mut result.definitions);
                }
                index += 1;
            }

            self.map.consume(&mut self.events);
        }

        value
    }
}

/// Move back past ignored bytes.
fn move_point_back(tokenizer: &mut Tokenizer, point: &mut Point) {
    // Only the CR of a CRLF pair is ignored. Inspect it directly instead of running tab expansion and byte normalization at every event boundary.
    let bytes = tokenizer.parse_state.bytes;
    if point.index > 0
        && point.index < bytes.len()
        && bytes[point.index - 1] == b'\r'
        && bytes[point.index] == b'\n'
    {
        point.index -= 1;
    }
}

/// Enter.
fn enter_impl(tokenizer: &mut Tokenizer, name: Name, link: Option<Link>) {
    let mut point = tokenizer.point.clone();
    move_point_back(tokenizer, &mut point);

    #[cfg(feature = "log")]
    log::debug!("enter:   `{name:?}`");

    tokenizer.stack.push(name.clone());
    tokenizer.events.push(Event {
        kind: Kind::Enter,
        name,
        point,
        link,
    });
}

/// Run the tokenizer.
fn push_impl(
    tokenizer: &mut Tokenizer,
    from: (usize, usize),
    to: (usize, usize),
    mut state: State,
    flush: bool,
) -> State {
    debug_assert!(
        from.0 > tokenizer.point.index
            || (from.0 == tokenizer.point.index && from.1 >= tokenizer.point.vs),
        "cannot move backwards"
    );

    tokenizer.move_to(from);

    loop {
        match state {
            State::Ok | State::Nok => {
                if let Some(attempt) = tokenizer.attempts.pop() {
                    if attempt.kind == AttemptKind::Check || matches!(state, State::Nok) {
                        if let Some(progress) = attempt.progress {
                            tokenizer.free(progress);
                        }
                    }

                    tokenizer.consumed = true;

                    let next = if matches!(state, State::Ok) {
                        attempt.ok
                    } else {
                        attempt.nok
                    };

                    #[cfg(feature = "log")]
                    log::trace!("attempt: `{state:?}` -> `{next:?}`");

                    state = next;
                } else {
                    break;
                }
            }
            State::Next(name) => {
                // These states only consume ordinary bytes until a delimiter. Scan the run without redispatching the state machine for every byte. Tabs and line endings still use the virtual-byte machinery below, and `to` remains a hard boundary between linked chunks.
                if matches!(
                    name,
                    StateName::DataInside
                        | StateName::ContentChunkInside
                        | StateName::DocumentFlowInside
                        | StateName::ParagraphInside
                        | StateName::RawFlowContentChunk
                        | StateName::RawTextData
                ) {
                    let start = tokenizer.point.index;
                    let bytes = tokenizer.parse_state.bytes;
                    let mut end = start;
                    if name == StateName::DataInside {
                        while end < to.0
                            && !matches!(bytes[end], b'\r' | b'\n' | b'\t')
                            && !tokenizer.is_data_marker(bytes[end])
                        {
                            end += 1;
                        }
                    } else {
                        // memchr uses SIMD where available, with a no_std scalar fallback.
                        let remaining = &bytes[start..to.0];
                        end += memchr::memchr3(b'\r', b'\n', b'\t', remaining)
                            .unwrap_or(remaining.len());
                        if name == StateName::RawTextData {
                            end =
                                memchr::memchr(tokenizer.tokenize_state.marker, &bytes[start..end])
                                    .map_or(end, |offset| start + offset);
                        }
                    }
                    if end > start {
                        tokenizer.point.index = end;
                        tokenizer.point.column += end - start;
                        tokenizer.point.vs = 0;
                        tokenizer.previous = Some(bytes[end - 1]);
                        tokenizer.current = None;
                        tokenizer.consumed = true;
                        continue;
                    }
                }
                let action = if tokenizer.point.index < to.0
                    || (tokenizer.point.index == to.0 && tokenizer.point.vs < to.1)
                {
                    Some(byte_action(tokenizer.parse_state.bytes, &tokenizer.point))
                } else if flush {
                    None
                } else {
                    break;
                };

                if let Some(ByteAction::Ignore) = action {
                    tokenizer.move_one();
                } else {
                    let byte =
                        if let Some(ByteAction::Insert(byte) | ByteAction::Normal(byte)) = action {
                            Some(byte)
                        } else {
                            None
                        };

                    #[cfg(feature = "log")]
                    log::trace!("feed:    {} to {:?}", format_byte_opt(byte), name);

                    tokenizer.expect(byte);
                    state = call(tokenizer, name);
                }
            }
            State::Retry(name) => {
                #[cfg(feature = "log")]
                log::trace!("retry:   `{name:?}`");

                state = call(tokenizer, name);
            }
        }
    }

    tokenizer.consumed = true;

    if flush {
        debug_assert!(
            match state {
                State::Ok => true,
                _ => unreachable!("flushing a tokenizer must produce a terminal state"),
            },
            "must be ok"
        );
    } else {
        debug_assert!(
            match state {
                State::Next(_) => true,
                _ => unreachable!("continuing a tokenizer must produce a next state"),
            },
            "must have a next state"
        );
    }

    state
}

/// Figure out how to handle a byte.
fn byte_action(bytes: &[u8], point: &Point) -> ByteAction {
    if point.index < bytes.len() {
        let byte = bytes[point.index];

        if byte == b'\r' {
            // CRLF.
            if point.index < bytes.len() - 1 && bytes[point.index + 1] == b'\n' {
                ByteAction::Ignore
            }
            // CR.
            else {
                ByteAction::Normal(b'\n')
            }
        } else if byte == b'\t' {
            let remainder = point.column % TAB_SIZE;
            let vs = if remainder == 0 {
                0
            } else {
                TAB_SIZE - remainder
            };

            // On the tab itself, first send it.
            if point.vs == 0 {
                if vs == 0 {
                    ByteAction::Normal(byte)
                } else {
                    ByteAction::Insert(byte)
                }
            } else if vs == 0 {
                ByteAction::Normal(b' ')
            } else {
                ByteAction::Insert(b' ')
            }
        } else {
            ByteAction::Normal(byte)
        }
    } else {
        unreachable!("out of bounds")
    }
}

#[cfg(test)]
mod tests {
    use super::{byte_action, Tokenizer};
    use crate::{
        construct::frontmatter,
        construct::html_flow::{blank_line_before, blank_lines_end_data, blank_lines_end_start},
        event::{Name, Point},
        parser::{parse, ParseState},
        state::{Name as StateName, State},
        ParseOptions,
    };
    fn make_tokenizer<'a>(parse_state: &'a ParseState<'a>) -> Tokenizer<'a> {
        Tokenizer::new(
            Point {
                line: 1,
                column: 1,
                index: 0,
                vs: 0,
            },
            parse_state,
        )
    }

    #[test]
    fn scans_data_runs_within_chunk_and_virtual_space_boundaries() {
        let options = ParseOptions::default();
        let (_events, parse_state) = parse("ab\tdef\r\nxyz", &options);
        let mut tokenizer = make_tokenizer(&parse_state);
        tokenizer.tokenize_state.markers = 1 << b'*';
        tokenizer.enter(Name::Data);
        let state = tokenizer.push((0, 0), (1, 0), State::Next(StateName::DataInside));
        assert_eq!(tokenizer.point.index, 1);
        assert_eq!(tokenizer.point.column, 2);
        assert_eq!(tokenizer.previous, Some(b'a'));
        let state = tokenizer.push((1, 0), (2, 1), state);
        assert_eq!(tokenizer.point.index, 2);
        assert_eq!(tokenizer.point.vs, 1);
        assert_eq!(tokenizer.point.column, 4);
        let state = tokenizer.push((2, 1), (parse_state.bytes.len(), 0), state);
        tokenizer.flush(state, true);
        assert_eq!(tokenizer.point.line, 2);
        assert_eq!(tokenizer.point.column, 4);
        assert_eq!(tokenizer.previous, Some(b'z'));
    }

    #[cfg(feature = "log")]
    #[test]
    fn emits_trace_records_for_tokenizer_transitions() {
        env_logger::Builder::new()
            .filter_level(log::LevelFilter::Trace)
            .is_test(true)
            .try_init()
            .expect("install test logger");

        let options = ParseOptions::default();
        let (events, parse_state) = parse("first paragraph\nsecond line", &options);

        assert!(!events.is_empty());
        assert_eq!(parse_state.bytes, b"first paragraph\nsecond line");
    }

    #[test]
    fn blank_line_end_states_consume_whitespace_and_finish_retained_lines() {
        let options = ParseOptions {
            html_block_blank_lines: true,
            ..ParseOptions::default()
        };

        let (_events, parse_state) = parse(" \n", &options);
        let mut tokenizer = make_tokenizer(&parse_state);
        tokenizer.enter(Name::HtmlFlowData);
        tokenizer.current = Some(b' ');
        tokenizer.consumed = false;
        tokenizer.consume();
        tokenizer.current = Some(b'\n');
        tokenizer.consumed = false;
        tokenizer.tokenize_state.html_flow_blank_lines = 1;

        assert_eq!(
            blank_lines_end_data(&mut tokenizer),
            State::Next(StateName::HtmlFlowBlankLinesEndStart)
        );
        assert_eq!(tokenizer.tokenize_state.html_flow_blank_lines, 0);

        let (_events, parse_state) = parse(" x", &options);
        let mut tokenizer = make_tokenizer(&parse_state);
        tokenizer.enter(Name::HtmlFlowData);
        tokenizer.current = Some(b' ');
        tokenizer.consumed = false;
        tokenizer.consume();
        tokenizer.current = Some(b'x');
        tokenizer.consumed = false;
        tokenizer.tokenize_state.html_flow_blank_lines = 1;

        assert_eq!(
            blank_lines_end_data(&mut tokenizer),
            State::Retry(StateName::HtmlFlowContinuationAfter)
        );
        assert_eq!(tokenizer.tokenize_state.html_flow_blank_lines, 0);

        let (_events, parse_state) = parse("x", &options);
        let mut tokenizer = make_tokenizer(&parse_state);
        tokenizer.current = Some(b'x');
        tokenizer.tokenize_state.html_flow_blank_lines = 1;

        assert_eq!(
            blank_lines_end_start(&mut tokenizer),
            State::Retry(StateName::HtmlFlowContinuationAfter)
        );
        assert_eq!(tokenizer.tokenize_state.html_flow_blank_lines, 0);
    }

    #[test]
    fn blank_line_before_handles_html_marker_kinds() {
        const COMMENT: u8 = 2;
        const BASIC: u8 = 6;
        const COMPLETE: u8 = 7;

        let options = ParseOptions {
            html_block_blank_lines: true,
            ..ParseOptions::default()
        };

        for (marker, expected) in [
            (BASIC, State::Next(StateName::HtmlFlowBlankLinesCheck)),
            (COMPLETE, State::Next(StateName::HtmlFlowBlankLinesCheck)),
            (COMMENT, State::Next(StateName::BlankLineStart)),
        ] {
            let (_events, parse_state) = parse("\n", &options);
            let mut tokenizer = make_tokenizer(&parse_state);
            tokenizer.current = Some(b'\n');
            tokenizer.consumed = false;
            tokenizer.tokenize_state.marker = marker;

            assert_eq!(blank_line_before(&mut tokenizer), expected);
        }
    }

    #[test]
    fn frontmatter_after_accepts_eof() {
        let options = ParseOptions::default();
        let (_events, parse_state) = parse("\n", &options);
        let mut tokenizer = make_tokenizer(&parse_state);
        tokenizer.current = Some(b'\n');
        tokenizer.expect(Some(b'\n'));
        tokenizer.enter(Name::Frontmatter);
        tokenizer.consume();

        assert_eq!(frontmatter::after(&mut tokenizer), State::Ok);
    }

    #[test]
    #[should_panic(expected = "out of bounds")]
    fn byte_action_rejects_points_past_the_input() {
        byte_action(
            b"",
            &Point {
                line: 1,
                column: 1,
                index: 0,
                vs: 0,
            },
        );
    }
}
