// Released under the MIT License.
// Copyright, 2022-2025, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

//! Frontmatter occurs at the start of the document, outside containers.
//!
//! ## Grammar
//!
//! ```bnf
//! frontmatter ::= fence_open eol *( *byte eol ) fence_close
//! fence_open ::= legacy_sequence *space_or_tab [info]
//!              | code_sequence *space_or_tab info
//! fence_close ::= legacy_sequence *space_or_tab
//!               | 0*3space_or_tab code_sequence *space_or_tab
//! legacy_sequence ::= 3'+' | 3'-'
//! code_sequence ::= 3*'`' | 3*'~'
//! info ::= 1*byte
//! ```
//!
//! Opening and closing markers must match. Legacy fences have exactly three
//! markers; closing code fences must be at least as long as the opening fence.
//! Code-fence indentation is measured in columns, with at most three before a
//! closing fence. Opening fences cannot be indented. Info strings cannot contain
//! line endings, or backticks when the opening marker is a backtick.
//!
//! Frontmatter can only occur once and requires a closing fence followed by a
//! line ending or end of file. Untagged code fences remain ordinary code blocks.
//! All forms produce a format-agnostic [`NodeSyntax::Frontmatter`][crate::NodeSyntax::Frontmatter].
//! Untagged `---` and `+++` imply YAML and TOML. Tagged forms, such as `--- json`,
//! `---yaml`, `+++ toml`, or a fenced code block with a language, supply an
//! explicit format hint. The parser preserves the opaque info string and raw
//! body, following Socketry's Markly/cmarkly frontmatter model.
//!
//! ## Extension
//!
//! Frontmatter is not part of `CommonMark` and is disabled by default. Enable
//! [`Constructs::frontmatter`][crate::Constructs::frontmatter] to recognize it.
//! Frontmatter is omitted from HTML output. YAML and TOML delimiter forms are
//! more widely supported by other Markdown tools than tagged code fences.
//!
//! ## Tokens
//!
//! * [`Frontmatter`][Name::Frontmatter]
//! * [`FrontmatterFence`][Name::FrontmatterFence]
//! * [`FrontmatterFenceInfo`][Name::FrontmatterFenceInfo]
//! * [`FrontmatterSequence`][Name::FrontmatterSequence]
//! * [`FrontmatterChunk`][Name::FrontmatterChunk]
//! * [`LineEnding`][Name::LineEnding]
//! * [`SpaceOrTab`][Name::SpaceOrTab]
//!
//! ## References
//!
//! * [`micromark-extension-frontmatter`](https://github.com/micromark/micromark-extension-frontmatter)
//! * [`cmarkly` frontmatter](https://github.com/socketry/cmarkly/blob/main/src/front_matter.c)
use crate::construct::partial_space_or_tab::{space_or_tab, space_or_tab_min_max};
use crate::event::Name;
use crate::state::{Name as StateName, State};
use crate::tokenizer::Tokenizer;
use crate::util::constant::FRONTMATTER_SEQUENCE_SIZE;

/// Start of frontmatter.
///
/// ```markdown
/// > | ---
///     ^
///   | title: "Venus"
///   | ---
/// ```
pub fn start(tokenizer: &mut Tokenizer) -> State {
    // Indent not allowed.
    if tokenizer.parse_state.options.constructs.frontmatter
        && matches!(tokenizer.current, Some(b'+' | b'-' | b'`' | b'~'))
    {
        tokenizer.tokenize_state.marker = tokenizer.current.unwrap();
        tokenizer.enter(Name::Frontmatter);
        tokenizer.enter(Name::FrontmatterFence);
        tokenizer.enter(Name::FrontmatterSequence);
        State::Retry(StateName::FrontmatterOpenSequence)
    } else {
        State::Nok
    }
}

/// In open sequence.
///
/// ```markdown
/// > | ---
///     ^
///   | title: "Venus"
///   | ---
/// ```
pub fn open_sequence(tokenizer: &mut Tokenizer) -> State {
    if tokenizer.current == Some(tokenizer.tokenize_state.marker) {
        tokenizer.tokenize_state.size += 1;
        tokenizer.consume();
        State::Next(StateName::FrontmatterOpenSequence)
    } else if tokenizer.tokenize_state.size == FRONTMATTER_SEQUENCE_SIZE
        || (matches!(tokenizer.tokenize_state.marker, b'`' | b'~')
            && tokenizer.tokenize_state.size > FRONTMATTER_SEQUENCE_SIZE)
    {
        tokenizer.tokenize_state.size_b = tokenizer.tokenize_state.size;
        tokenizer.tokenize_state.size = 0;
        tokenizer.exit(Name::FrontmatterSequence);
        let next = StateName::FrontmatterInfoBefore;

        if matches!(tokenizer.current, Some(b'\t' | b' ')) {
            tokenizer.attempt(State::Next(next), State::Nok);
            State::Retry(space_or_tab(tokenizer))
        } else {
            State::Retry(next)
        }
    } else {
        tokenizer.tokenize_state.marker = 0;
        tokenizer.tokenize_state.size = 0;
        State::Nok
    }
}

/// Read an optional format hint, requiring one on code-style fences.
pub fn info_before(tokenizer: &mut Tokenizer) -> State {
    match tokenizer.current {
        None | Some(b'\n') => {
            if matches!(tokenizer.tokenize_state.marker, b'`' | b'~') {
                tokenizer.tokenize_state.marker = 0;
                tokenizer.tokenize_state.size_b = 0;
                State::Nok
            } else {
                State::Retry(StateName::FrontmatterOpenAfter)
            }
        }
        _ => {
            tokenizer.enter(Name::FrontmatterFenceInfo);
            State::Retry(StateName::FrontmatterInfo)
        }
    }
}

/// Preserve the whole info string without interpreting its named format.
pub fn info(tokenizer: &mut Tokenizer) -> State {
    match tokenizer.current {
        None | Some(b'\n') => {
            tokenizer.exit(Name::FrontmatterFenceInfo);
            State::Retry(StateName::FrontmatterOpenAfter)
        }
        Some(b'`') if tokenizer.tokenize_state.marker == b'`' => {
            tokenizer.tokenize_state.marker = 0;
            tokenizer.tokenize_state.size_b = 0;
            State::Nok
        }
        Some(_) => {
            tokenizer.consume();
            State::Next(StateName::FrontmatterInfo)
        }
    }
}

/// After open sequence.
///
/// ```markdown
/// > | ---
///        ^
///   | title: "Venus"
///   | ---
/// ```
pub fn open_after(tokenizer: &mut Tokenizer) -> State {
    if let Some(b'\n') = tokenizer.current {
        tokenizer.exit(Name::FrontmatterFence);
        tokenizer.enter(Name::LineEnding);
        tokenizer.consume();
        tokenizer.exit(Name::LineEnding);
        tokenizer.attempt(
            State::Next(StateName::FrontmatterAfter),
            State::Next(StateName::FrontmatterContentStart),
        );
        State::Next(StateName::FrontmatterCloseStart)
    } else {
        tokenizer.tokenize_state.marker = 0;
        tokenizer.tokenize_state.size_b = 0;
        State::Nok
    }
}

/// Start of close sequence.
///
/// ```markdown
///   | ---
///   | title: "Venus"
/// > | ---
///     ^
/// ```
pub fn close_start(tokenizer: &mut Tokenizer) -> State {
    if matches!(tokenizer.tokenize_state.marker, b'`' | b'~')
        && matches!(tokenizer.current, Some(b'\t' | b' '))
    {
        tokenizer.attempt(
            State::Next(StateName::FrontmatterCloseBeforeSequence),
            State::Nok,
        );
        State::Retry(space_or_tab_min_max(tokenizer, 0, 3))
    } else {
        State::Retry(StateName::FrontmatterCloseBeforeSequence)
    }
}

/// Start a closing sequence, after at most three spaces for code-style fences.
pub fn close_before_sequence(tokenizer: &mut Tokenizer) -> State {
    if tokenizer.current == Some(tokenizer.tokenize_state.marker) {
        tokenizer.enter(Name::FrontmatterFence);
        tokenizer.enter(Name::FrontmatterSequence);
        State::Retry(StateName::FrontmatterCloseSequence)
    } else {
        State::Nok
    }
}

/// In close sequence.
///
/// ```markdown
///   | ---
///   | title: "Venus"
/// > | ---
///     ^
/// ```
pub fn close_sequence(tokenizer: &mut Tokenizer) -> State {
    if tokenizer.current == Some(tokenizer.tokenize_state.marker) {
        tokenizer.tokenize_state.size += 1;
        tokenizer.consume();
        State::Next(StateName::FrontmatterCloseSequence)
    } else if tokenizer.tokenize_state.size == tokenizer.tokenize_state.size_b
        || (matches!(tokenizer.tokenize_state.marker, b'`' | b'~')
            && tokenizer.tokenize_state.size > tokenizer.tokenize_state.size_b)
    {
        tokenizer.tokenize_state.size = 0;
        tokenizer.exit(Name::FrontmatterSequence);

        if matches!(tokenizer.current, Some(b'\t' | b' ')) {
            tokenizer.attempt(State::Next(StateName::FrontmatterCloseAfter), State::Nok);
            State::Retry(space_or_tab(tokenizer))
        } else {
            State::Retry(StateName::FrontmatterCloseAfter)
        }
    } else {
        tokenizer.tokenize_state.size = 0;
        State::Nok
    }
}

/// After close sequence.
///
/// ```markdown
///   | ---
///   | title: "Venus"
/// > | ---
///        ^
/// ```
pub fn close_after(tokenizer: &mut Tokenizer) -> State {
    match tokenizer.current {
        None | Some(b'\n') => {
            tokenizer.exit(Name::FrontmatterFence);
            State::Ok
        }
        _ => State::Nok,
    }
}

/// Start of content chunk.
///
/// ```markdown
///   | ---
/// > | title: "Venus"
///     ^
///   | ---
/// ```
pub fn content_start(tokenizer: &mut Tokenizer) -> State {
    match tokenizer.current {
        None | Some(b'\n') => State::Retry(StateName::FrontmatterContentEnd),
        Some(_) => {
            tokenizer.enter(Name::FrontmatterChunk);
            State::Retry(StateName::FrontmatterContentInside)
        }
    }
}

/// In content chunk.
///
/// ```markdown
///   | ---
/// > | title: "Venus"
///     ^
///   | ---
/// ```
pub fn content_inside(tokenizer: &mut Tokenizer) -> State {
    match tokenizer.current {
        None | Some(b'\n') => {
            tokenizer.exit(Name::FrontmatterChunk);
            State::Retry(StateName::FrontmatterContentEnd)
        }
        Some(_) => {
            tokenizer.consume();
            State::Next(StateName::FrontmatterContentInside)
        }
    }
}

/// End of content chunk.
///
/// ```markdown
///   | ---
/// > | title: "Venus"
///                   ^
///   | ---
/// ```
pub fn content_end(tokenizer: &mut Tokenizer) -> State {
    match tokenizer.current {
        None => {
            tokenizer.tokenize_state.marker = 0;
            tokenizer.tokenize_state.size_b = 0;
            State::Nok
        }
        Some(b'\n') => {
            tokenizer.enter(Name::LineEnding);
            tokenizer.consume();
            tokenizer.exit(Name::LineEnding);
            tokenizer.attempt(
                State::Next(StateName::FrontmatterAfter),
                State::Next(StateName::FrontmatterContentStart),
            );
            State::Next(StateName::FrontmatterCloseStart)
        }
        Some(_) => unreachable!("expected eof/eol"),
    }
}

/// After frontmatter.
///
/// ```markdown
///   | ---
///   | title: "Venus"
/// > | ---
///        ^
/// ```
pub fn after(tokenizer: &mut Tokenizer) -> State {
    debug_assert!(
        matches!(tokenizer.current, None | Some(b'\n')),
        "expected eol/eof after closing fence"
    );
    tokenizer.exit(Name::Frontmatter);
    tokenizer.tokenize_state.marker = 0;
    tokenizer.tokenize_state.size_b = 0;
    State::Ok
}
