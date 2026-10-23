// Released under the MIT License.
// Copyright, 2022-2025, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

//! HTML (flow) occurs in the [flow][] content type.
//!
//! ## Grammar
//!
//! HTML (flow) forms with the following BNF
//! (<small>see [construct][crate::construct] for character groups</small>):
//!
//! ```bnf
//! html_flow ::= raw | comment | instruction | declaration | cdata | basic | complete
//!
//! ; Note: closing tag name does not need to match opening tag name.
//! raw ::= '<' raw_tag_name [[space_or_tab *line | '>' *line] eol] *(*line eol) ['</' raw_tag_name *line]
//! comment ::= '<!--' [*'-' '>' *line | *line *(eol *line) ['-->' *line]]
//! instruction ::= '<?' ['>' *line | *line *(eol *line) ['?>' *line]]
//! declaration ::= '<!' ascii_alphabetic *line *(eol *line) ['>' *line]
//! cdata ::= '<![CDATA[' *line *(eol *line) [']]>' *line]
//! basic ::= '< ['/'] basic_tag_name [['/'] '>' *line *(eol 1*line)]
//! complete ::= (opening_tag | closing_tag) [*space_or_tab *(eol 1*line)]
//!
//! raw_tag_name ::= 'pre' | 'script' | 'style' | 'textarea' ; Note: case-insensitive.
//! basic_tag_name ::= 'address' | 'article' | 'aside' | ... ; See `constants.rs`, and note: case-insensitive.
//! opening_tag ::= '<' tag_name *(1*space_or_tab attribute) [*space_or_tab '/'] *space_or_tab '>'
//! closing_tag ::= '</' tag_name *space_or_tab '>'
//! tag_name ::= ascii_alphabetic *('-' | ascii_alphanumeric | [':' when html_tag_namespaces])
//! attribute ::= attribute_name [*space_or_tab '=' *space_or_tab attribute_value]
//! attribute_name ::= (':' | '_' | ascii_alphabetic) *('-' | '.' | ':' | '_' | ascii_alphanumeric)
//! attribute_value ::= '"' *(line - '"') '"' | "'" *(line - "'")  "'" | 1*(text - '"' - "'" - '/' - '<' - '=' - '>' - '`')
//! ```
//!
//! As this construct occurs in flow, like all flow constructs, it must be
//! followed by an eol (line ending) or eof (end of file).
//!
//! The grammar for HTML in markdown does not follow the rules of parsing
//! HTML according to the [*§ 13.2 Parsing HTML documents* in the HTML
//! spec][html_parsing].
//! As such, HTML in markdown *resembles* HTML, but is instead a (naïve?)
//! attempt to parse an XML-like language.
//! By extension, another notable property of the grammar is that it can
//! result in invalid HTML, in that it allows things that wouldn’t work or
//! wouldn’t work well in HTML, such as mismatched tags.
//!
//! Interestingly, most of the productions above have a clear opening and
//! closing condition (raw, comment, insutrction, declaration, cdata), but the
//! closing condition does not need to be satisfied.
//! In this case, the parser never has to backtrack.
//!
//! Because the **basic** and **complete** productions in the grammar form with
//! a tag, followed by more stuff, and stop at a blank line, it is possible to
//! interleave (a word for switching between languages) markdown and HTML
//! together, by placing the opening and closing tags on their own lines,
//! with blank lines between them and markdown.
//! For example:
//!
//! ```markdown
//! <div>This is <code>code</code> but this is not *emphasis*.</div>
//!
//! <div>
//!
//! This is a paragraph in a `div` and with `code` and *emphasis*.
//!
//! </div>
//! ```
//!
//! The **complete** production of HTML (flow) is not allowed to interrupt
//! content.
//! That means that a blank line is needed between a [paragraph][] and it.
//! However, [HTML (text)][html_text] has a similar production, which will
//! typically kick-in instead.
//!
//! The list of tag names allowed in the **raw** production are defined in
//! [`HTML_RAW_NAMES`][].
//! This production exists because there are a few cases where markdown
//! *inside* some elements, and hence interleaving, does not make sense.
//!
//! The list of tag names allowed in the **basic** production are defined in
//! [`HTML_BLOCK_NAMES`][].
//! This production exists because there are a few cases where we can decide
//! early that something is going to be a flow (block) element instead of a
//! phrasing (inline) element.
//! We *can* interrupt and don’t have to care too much about it being
//! well-formed.
//!
//! ## Tokens
//!
//! * [`HtmlFlow`][Name::HtmlFlow]
//! * [`HtmlFlowData`][Name::HtmlFlowData]
//! * [`LineEnding`][Name::LineEnding]
//!
//! ## References
//!
//! * [`html-flow.js` in `micromark`](https://github.com/micromark/micromark/blob/main/packages/micromark-core-commonmark/dev/lib/html-flow.js)
//! * [*§ 4.6 HTML blocks* in `CommonMark`](https://spec.commonmark.org/0.31/#html-blocks)
//!
//! [flow]: crate::construct::flow
//! [html_text]: crate::construct::html_text
//! [paragraph]: crate::construct::paragraph
//! [html_raw_names]: crate::util::constant::HTML_RAW_NAMES
//! [html_block_names]: crate::util::constant::HTML_BLOCK_NAMES
//! [html_parsing]: https://html.spec.whatwg.org/multipage/parsing.html#parsing
use crate::construct::partial_space_or_tab::{
    space_or_tab_with_options, Options as SpaceOrTabOptions,
};
use crate::event::Name;
use crate::state::{Name as StateName, State};
use crate::tokenizer::Tokenizer;
use crate::util::{
    constant::{HTML_BLOCK_NAMES, HTML_CDATA_PREFIX, HTML_RAW_NAMES, HTML_RAW_SIZE_MAX, TAB_SIZE},
    slice::Slice,
};

/// Symbol for `<script>` (condition 1).
const RAW: u8 = 1;
/// Symbol for `<!---->` (condition 2).
const COMMENT: u8 = 2;
/// Symbol for `<?php?>` (condition 3).
const INSTRUCTION: u8 = 3;
/// Symbol for `<!doctype>` (condition 4).
const DECLARATION: u8 = 4;
/// Symbol for `<![CDATA[]]>` (condition 5).
const CDATA: u8 = 5;
/// Symbol for `<div` (condition 6).
const BASIC: u8 = 6;
/// Symbol for `<x>` (condition 7).
const COMPLETE: u8 = 7;

/// Start of HTML (flow).
///
/// ```markdown
/// > | <x />
///     ^
/// ```
pub fn start(tokenizer: &mut Tokenizer) -> State {
    if tokenizer.parse_state.options.constructs.html_flow {
        tokenizer.tokenize_state.html_flow_indent = 0;
        tokenizer.tokenize_state.html_flow_blank_lines = 0;
        tokenizer.enter(Name::HtmlFlow);

        if matches!(tokenizer.current, Some(b'\t' | b' ')) {
            tokenizer.attempt(State::Next(StateName::HtmlFlowBefore), State::Nok);
            State::Retry(space_or_tab_with_options(
                tokenizer,
                SpaceOrTabOptions {
                    kind: Name::HtmlFlowData,
                    min: 0,
                    max: if tokenizer.parse_state.options.constructs.code_indented {
                        TAB_SIZE - 1
                    } else {
                        usize::MAX
                    },
                    connect: false,
                    content: None,
                },
            ))
        } else {
            State::Retry(StateName::HtmlFlowBefore)
        }
    } else {
        State::Nok
    }
}

/// At `<`, after optional whitespace.
///
/// ```markdown
/// > | <x />
///     ^
/// ```
pub fn before(tokenizer: &mut Tokenizer) -> State {
    if Some(b'<') == tokenizer.current {
        tokenizer.enter(Name::HtmlFlowData);
        tokenizer.consume();
        State::Next(StateName::HtmlFlowOpen)
    } else {
        State::Nok
    }
}

/// After `<`, at tag name or other stuff.
///
/// ```markdown
/// > | <x />
///      ^
/// > | <!doctype>
///      ^
/// > | <!--xxx-->
///      ^
/// ```
pub fn open(tokenizer: &mut Tokenizer) -> State {
    match tokenizer.current {
        Some(b'!') => {
            tokenizer.consume();
            State::Next(StateName::HtmlFlowDeclarationOpen)
        }
        Some(b'/') => {
            tokenizer.consume();
            tokenizer.tokenize_state.seen = true;
            tokenizer.tokenize_state.start = tokenizer.point.index;
            State::Next(StateName::HtmlFlowTagCloseStart)
        }
        Some(b'?') => {
            tokenizer.consume();
            tokenizer.tokenize_state.marker = INSTRUCTION;
            // Do not form containers.
            tokenizer.concrete = true;
            // While we’re in an instruction instead of a declaration, we’re on a `?`
            // right now, so we do need to search for `>`, similar to declarations.
            State::Next(StateName::HtmlFlowContinuationDeclarationInside)
        }
        // ASCII alphabetical.
        Some(b'A'..=b'Z' | b'a'..=b'z') => {
            tokenizer.tokenize_state.start = tokenizer.point.index;
            State::Retry(StateName::HtmlFlowTagName)
        }
        _ => State::Nok,
    }
}

/// After `<!`, at declaration, comment, or CDATA.
///
/// ```markdown
/// > | <!doctype>
///       ^
/// > | <!--xxx-->
///       ^
/// > | <![CDATA[>&<]]>
///       ^
/// ```
pub fn declaration_open(tokenizer: &mut Tokenizer) -> State {
    match tokenizer.current {
        Some(b'-') => {
            tokenizer.consume();
            tokenizer.tokenize_state.marker = COMMENT;
            State::Next(StateName::HtmlFlowCommentOpenInside)
        }
        Some(b'A'..=b'Z' | b'a'..=b'z') => {
            tokenizer.consume();
            tokenizer.tokenize_state.marker = DECLARATION;
            // Do not form containers.
            tokenizer.concrete = true;
            State::Next(StateName::HtmlFlowContinuationDeclarationInside)
        }
        Some(b'[') => {
            tokenizer.consume();
            tokenizer.tokenize_state.marker = CDATA;
            State::Next(StateName::HtmlFlowCdataOpenInside)
        }
        _ => State::Nok,
    }
}

/// After `<!-`, inside a comment, at another `-`.
///
/// ```markdown
/// > | <!--xxx-->
///        ^
/// ```
pub fn comment_open_inside(tokenizer: &mut Tokenizer) -> State {
    if let Some(b'-') = tokenizer.current {
        tokenizer.consume();
        // Do not form containers.
        tokenizer.concrete = true;
        State::Next(StateName::HtmlFlowContinuationDeclarationInside)
    } else {
        tokenizer.tokenize_state.marker = 0;
        State::Nok
    }
}

/// After `<![`, inside CDATA, expecting `CDATA[`.
///
/// ```markdown
/// > | <![CDATA[>&<]]>
///        ^^^^^^
/// ```
pub fn cdata_open_inside(tokenizer: &mut Tokenizer) -> State {
    if tokenizer.current == Some(HTML_CDATA_PREFIX[tokenizer.tokenize_state.size]) {
        tokenizer.consume();
        tokenizer.tokenize_state.size += 1;

        if tokenizer.tokenize_state.size == HTML_CDATA_PREFIX.len() {
            tokenizer.tokenize_state.size = 0;
            // Do not form containers.
            tokenizer.concrete = true;
            State::Next(StateName::HtmlFlowContinuation)
        } else {
            State::Next(StateName::HtmlFlowCdataOpenInside)
        }
    } else {
        tokenizer.tokenize_state.marker = 0;
        tokenizer.tokenize_state.size = 0;
        State::Nok
    }
}

/// After `</`, in closing tag, at tag name.
///
/// ```markdown
/// > | </x>
///       ^
/// ```
pub fn tag_close_start(tokenizer: &mut Tokenizer) -> State {
    if let Some(b'A'..=b'Z' | b'a'..=b'z') = tokenizer.current {
        tokenizer.consume();
        State::Next(StateName::HtmlFlowTagName)
    } else {
        tokenizer.tokenize_state.seen = false;
        tokenizer.tokenize_state.start = 0;
        State::Nok
    }
}

/// In tag name.
///
/// ```markdown
/// > | <ab>
///      ^^
/// > | </ab>
///       ^^
/// ```
pub fn tag_name(tokenizer: &mut Tokenizer) -> State {
    match tokenizer.current {
        None | Some(b'\t' | b'\n' | b' ' | b'/' | b'>') => {
            let closing_tag = tokenizer.tokenize_state.seen;
            let slash = matches!(tokenizer.current, Some(b'/'));
            // Guaranteed to be valid ASCII bytes.
            let slice = Slice::from_indices(
                tokenizer.parse_state.bytes,
                tokenizer.tokenize_state.start,
                tokenizer.point.index,
            );
            let name = slice
                .as_str()
                // The line ending case might result in a `\r` that is already accounted for.
                .trim()
                .to_ascii_lowercase();
            tokenizer.tokenize_state.seen = false;
            tokenizer.tokenize_state.start = 0;

            if !slash && !closing_tag && HTML_RAW_NAMES.contains(&name.as_str()) {
                tokenizer.tokenize_state.marker = RAW;
                // Do not form containers.
                tokenizer.concrete = true;
                State::Retry(StateName::HtmlFlowContinuation)
            } else if HTML_BLOCK_NAMES.contains(&name.as_str()) {
                tokenizer.tokenize_state.marker = BASIC;

                if slash {
                    tokenizer.consume();
                    State::Next(StateName::HtmlFlowBasicSelfClosing)
                } else {
                    // Do not form containers.
                    tokenizer.concrete = true;
                    State::Retry(StateName::HtmlFlowContinuation)
                }
            } else {
                tokenizer.tokenize_state.marker = COMPLETE;

                // Do not support complete HTML when interrupting.
                if tokenizer.interrupt && !tokenizer.lazy {
                    tokenizer.tokenize_state.marker = 0;
                    State::Nok
                } else if closing_tag {
                    State::Retry(StateName::HtmlFlowCompleteClosingTagAfter)
                } else {
                    State::Retry(StateName::HtmlFlowCompleteAttributeNameBefore)
                }
            }
        }
        // ASCII alphanumerical and `-`.
        Some(b'-' | b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z') => {
            tokenizer.consume();
            State::Next(StateName::HtmlFlowTagName)
        }
        Some(b':') if tokenizer.parse_state.options.html_tag_namespaces => {
            tokenizer.consume();
            State::Next(StateName::HtmlFlowTagName)
        }
        Some(_) => {
            tokenizer.tokenize_state.seen = false;
            State::Nok
        }
    }
}

/// After closing slash of a basic tag name.
///
/// ```markdown
/// > | <div/>
///          ^
/// ```
pub fn basic_self_closing(tokenizer: &mut Tokenizer) -> State {
    if let Some(b'>') = tokenizer.current {
        tokenizer.consume();
        // Do not form containers.
        tokenizer.concrete = true;
        State::Next(StateName::HtmlFlowContinuation)
    } else {
        tokenizer.tokenize_state.marker = 0;
        State::Nok
    }
}

/// After closing slash of a complete tag name.
///
/// ```markdown
/// > | <x/>
///        ^
/// ```
pub fn complete_closing_tag_after(tokenizer: &mut Tokenizer) -> State {
    match tokenizer.current {
        Some(b'\t' | b' ') => {
            tokenizer.consume();
            State::Next(StateName::HtmlFlowCompleteClosingTagAfter)
        }
        _ => State::Retry(StateName::HtmlFlowCompleteEnd),
    }
}

/// At an attribute name.
///
/// At first, this state is used after a complete tag name, after whitespace,
/// where it expects optional attributes or the end of the tag.
/// It is also reused after attributes, when expecting more optional
/// attributes.
///
/// ```markdown
/// > | <a />
///        ^
/// > | <a :b>
///        ^
/// > | <a _b>
///        ^
/// > | <a b>
///        ^
/// > | <a >
///        ^
/// ```
pub fn complete_attribute_name_before(tokenizer: &mut Tokenizer) -> State {
    match tokenizer.current {
        Some(b'\t' | b' ') => {
            tokenizer.consume();
            State::Next(StateName::HtmlFlowCompleteAttributeNameBefore)
        }
        Some(b'/') => {
            tokenizer.consume();
            State::Next(StateName::HtmlFlowCompleteEnd)
        }
        // ASCII alphanumerical and `:` and `_`.
        Some(b'0'..=b'9' | b':' | b'A'..=b'Z' | b'_' | b'a'..=b'z') => {
            tokenizer.consume();
            State::Next(StateName::HtmlFlowCompleteAttributeName)
        }
        _ => State::Retry(StateName::HtmlFlowCompleteEnd),
    }
}

/// In attribute name.
///
/// ```markdown
/// > | <a :b>
///         ^
/// > | <a _b>
///         ^
/// > | <a b>
///         ^
/// ```
pub fn complete_attribute_name(tokenizer: &mut Tokenizer) -> State {
    match tokenizer.current {
        // ASCII alphanumerical and `-`, `.`, `:`, and `_`.
        Some(b'-' | b'.' | b'0'..=b'9' | b':' | b'A'..=b'Z' | b'_' | b'a'..=b'z') => {
            tokenizer.consume();
            State::Next(StateName::HtmlFlowCompleteAttributeName)
        }
        _ => State::Retry(StateName::HtmlFlowCompleteAttributeNameAfter),
    }
}

/// After attribute name, at an optional initializer, the end of the tag, or
/// whitespace.
///
/// ```markdown
/// > | <a b>
///         ^
/// > | <a b=c>
///         ^
/// ```
pub fn complete_attribute_name_after(tokenizer: &mut Tokenizer) -> State {
    match tokenizer.current {
        Some(b'\t' | b' ') => {
            tokenizer.consume();
            State::Next(StateName::HtmlFlowCompleteAttributeNameAfter)
        }
        Some(b'=') => {
            tokenizer.consume();
            State::Next(StateName::HtmlFlowCompleteAttributeValueBefore)
        }
        _ => State::Retry(StateName::HtmlFlowCompleteAttributeNameBefore),
    }
}

/// Before unquoted, double quoted, or single quoted attribute value, allowing
/// whitespace.
///
/// ```markdown
/// > | <a b=c>
///          ^
/// > | <a b="c">
///          ^
/// ```
pub fn complete_attribute_value_before(tokenizer: &mut Tokenizer) -> State {
    match tokenizer.current {
        None | Some(b'<' | b'=' | b'>' | b'`') => {
            tokenizer.tokenize_state.marker = 0;
            State::Nok
        }
        Some(b'\t' | b' ') => {
            tokenizer.consume();
            State::Next(StateName::HtmlFlowCompleteAttributeValueBefore)
        }
        Some(b'"' | b'\'') => {
            tokenizer.tokenize_state.marker_b = tokenizer.current.unwrap();
            tokenizer.consume();
            State::Next(StateName::HtmlFlowCompleteAttributeValueQuoted)
        }
        _ => State::Retry(StateName::HtmlFlowCompleteAttributeValueUnquoted),
    }
}

/// In double or single quoted attribute value.
///
/// ```markdown
/// > | <a b="c">
///           ^
/// > | <a b='c'>
///           ^
/// ```
pub fn complete_attribute_value_quoted(tokenizer: &mut Tokenizer) -> State {
    if tokenizer.current == Some(tokenizer.tokenize_state.marker_b) {
        tokenizer.consume();
        tokenizer.tokenize_state.marker_b = 0;
        State::Next(StateName::HtmlFlowCompleteAttributeValueQuotedAfter)
    } else if matches!(tokenizer.current, None | Some(b'\n')) {
        tokenizer.tokenize_state.marker = 0;
        tokenizer.tokenize_state.marker_b = 0;
        State::Nok
    } else {
        tokenizer.consume();
        State::Next(StateName::HtmlFlowCompleteAttributeValueQuoted)
    }
}

/// In unquoted attribute value.
///
/// ```markdown
/// > | <a b=c>
///          ^
/// ```
pub fn complete_attribute_value_unquoted(tokenizer: &mut Tokenizer) -> State {
    match tokenizer.current {
        None | Some(b'\t' | b'\n' | b' ' | b'"' | b'\'' | b'/' | b'<' | b'=' | b'>' | b'`') => {
            State::Retry(StateName::HtmlFlowCompleteAttributeNameAfter)
        }
        Some(_) => {
            tokenizer.consume();
            State::Next(StateName::HtmlFlowCompleteAttributeValueUnquoted)
        }
    }
}

/// After double or single quoted attribute value, before whitespace or the
/// end of the tag.
///
/// ```markdown
/// > | <a b="c">
///            ^
/// ```
pub fn complete_attribute_value_quoted_after(tokenizer: &mut Tokenizer) -> State {
    if let Some(b'\t' | b' ' | b'/' | b'>') = tokenizer.current {
        State::Retry(StateName::HtmlFlowCompleteAttributeNameBefore)
    } else {
        tokenizer.tokenize_state.marker = 0;
        State::Nok
    }
}

/// In certain circumstances of a complete tag where only an `>` is allowed.
///
/// ```markdown
/// > | <a b="c">
///             ^
/// ```
pub fn complete_end(tokenizer: &mut Tokenizer) -> State {
    if let Some(b'>') = tokenizer.current {
        tokenizer.consume();
        State::Next(StateName::HtmlFlowCompleteAfter)
    } else {
        tokenizer.tokenize_state.marker = 0;
        State::Nok
    }
}

/// After `>` in a complete tag.
///
/// ```markdown
/// > | <x>
///        ^
/// ```
pub fn complete_after(tokenizer: &mut Tokenizer) -> State {
    match tokenizer.current {
        None | Some(b'\n') => {
            // Do not form containers.
            tokenizer.concrete = true;
            State::Retry(StateName::HtmlFlowContinuation)
        }
        Some(b'\t' | b' ') => {
            tokenizer.consume();
            State::Next(StateName::HtmlFlowCompleteAfter)
        }
        Some(_) => {
            tokenizer.tokenize_state.marker = 0;
            State::Nok
        }
    }
}

/// In continuation of any HTML kind.
///
/// ```markdown
/// > | <!--xxx-->
///          ^
/// ```
pub fn continuation(tokenizer: &mut Tokenizer) -> State {
    if tokenizer.tokenize_state.marker == COMMENT && tokenizer.current == Some(b'-') {
        tokenizer.consume();
        State::Next(StateName::HtmlFlowContinuationCommentInside)
    } else if tokenizer.tokenize_state.marker == RAW && tokenizer.current == Some(b'<') {
        tokenizer.consume();
        State::Next(StateName::HtmlFlowContinuationRawTagOpen)
    } else if tokenizer.tokenize_state.marker == DECLARATION && tokenizer.current == Some(b'>') {
        tokenizer.consume();
        State::Next(StateName::HtmlFlowContinuationClose)
    } else if tokenizer.tokenize_state.marker == INSTRUCTION && tokenizer.current == Some(b'?') {
        tokenizer.consume();
        State::Next(StateName::HtmlFlowContinuationDeclarationInside)
    } else if tokenizer.tokenize_state.marker == CDATA && tokenizer.current == Some(b']') {
        tokenizer.consume();
        State::Next(StateName::HtmlFlowContinuationCdataInside)
    } else if matches!(tokenizer.tokenize_state.marker, BASIC | COMPLETE)
        && tokenizer.current == Some(b'\n')
    {
        tokenizer.exit(Name::HtmlFlowData);
        let retain_blank_lines = tokenizer.parse_state.options.html_block_blank_lines;
        tokenizer.check(
            if retain_blank_lines {
                State::Next(StateName::HtmlFlowBlankLinesEnd)
            } else {
                State::Next(StateName::HtmlFlowContinuationAfter)
            },
            State::Next(StateName::HtmlFlowContinuationStart),
        );
        State::Retry(StateName::HtmlFlowBlankLineBefore)
    } else if matches!(tokenizer.current, None | Some(b'\n')) {
        tokenizer.exit(Name::HtmlFlowData);
        State::Retry(StateName::HtmlFlowContinuationStart)
    } else {
        tokenizer.consume();
        State::Next(StateName::HtmlFlowContinuation)
    }
}

/// In continuation, at eol.
///
/// ```markdown
/// > | <x>
///        ^
///   | asd
/// ```
pub fn continuation_start(tokenizer: &mut Tokenizer) -> State {
    tokenizer.check(
        State::Next(StateName::HtmlFlowContinuationStartNonLazy),
        State::Next(StateName::HtmlFlowContinuationAfter),
    );
    State::Retry(StateName::NonLazyContinuationStart)
}

/// In continuation, at eol, before non-lazy content.
///
/// ```markdown
/// > | <x>
///        ^
///   | asd
/// ```
pub fn continuation_start_non_lazy(tokenizer: &mut Tokenizer) -> State {
    match tokenizer.current {
        Some(b'\n') => {
            tokenizer.enter(Name::LineEnding);
            tokenizer.consume();
            tokenizer.exit(Name::LineEnding);
            State::Next(StateName::HtmlFlowContinuationBefore)
        }
        _ => unreachable!("expected eol"),
    }
}

/// In continuation, before non-lazy content.
///
/// ```markdown
///   | <x>
/// > | asd
///     ^
/// ```
pub fn continuation_before(tokenizer: &mut Tokenizer) -> State {
    match tokenizer.current {
        None | Some(b'\n') => State::Retry(StateName::HtmlFlowContinuationStart),
        _ => {
            record_line_indent(tokenizer);
            tokenizer.enter(Name::HtmlFlowData);
            State::Retry(StateName::HtmlFlowContinuation)
        }
    }
}

/// In comment continuation, after one `-`, expecting another.
///
/// ```markdown
/// > | <!--xxx-->
///             ^
/// ```
pub fn continuation_comment_inside(tokenizer: &mut Tokenizer) -> State {
    match tokenizer.current {
        Some(b'-') => {
            tokenizer.consume();
            State::Next(StateName::HtmlFlowContinuationDeclarationInside)
        }
        _ => State::Retry(StateName::HtmlFlowContinuation),
    }
}

/// In raw continuation, after `<`, at `/`.
///
/// ```markdown
/// > | <script>console.log(1)</script>
///                            ^
/// ```
pub fn continuation_raw_tag_open(tokenizer: &mut Tokenizer) -> State {
    match tokenizer.current {
        Some(b'/') => {
            tokenizer.consume();
            tokenizer.tokenize_state.start = tokenizer.point.index;
            State::Next(StateName::HtmlFlowContinuationRawEndTag)
        }
        _ => State::Retry(StateName::HtmlFlowContinuation),
    }
}

/// In raw continuation, after `</`, in a raw tag name.
///
/// ```markdown
/// > | <script>console.log(1)</script>
///                             ^^^^^^
/// ```
pub fn continuation_raw_end_tag(tokenizer: &mut Tokenizer) -> State {
    match tokenizer.current {
        Some(b'>') => {
            // Guaranteed to be valid ASCII bytes.
            let slice = Slice::from_indices(
                tokenizer.parse_state.bytes,
                tokenizer.tokenize_state.start,
                tokenizer.point.index,
            );
            let name = slice.as_str().to_ascii_lowercase();

            tokenizer.tokenize_state.start = 0;

            if HTML_RAW_NAMES.contains(&name.as_str()) {
                tokenizer.consume();
                State::Next(StateName::HtmlFlowContinuationClose)
            } else {
                State::Retry(StateName::HtmlFlowContinuation)
            }
        }
        Some(b'A'..=b'Z' | b'a'..=b'z')
            if tokenizer.point.index - tokenizer.tokenize_state.start < HTML_RAW_SIZE_MAX =>
        {
            tokenizer.consume();
            State::Next(StateName::HtmlFlowContinuationRawEndTag)
        }
        _ => {
            tokenizer.tokenize_state.start = 0;
            State::Retry(StateName::HtmlFlowContinuation)
        }
    }
}

/// In cdata continuation, after `]`, expecting `]>`.
///
/// ```markdown
/// > | <![CDATA[>&<]]>
///                  ^
/// ```
pub fn continuation_cdata_inside(tokenizer: &mut Tokenizer) -> State {
    match tokenizer.current {
        Some(b']') => {
            tokenizer.consume();
            State::Next(StateName::HtmlFlowContinuationDeclarationInside)
        }
        _ => State::Retry(StateName::HtmlFlowContinuation),
    }
}

/// In declaration or instruction continuation, at `>`.
///
/// ```markdown
/// > | <!-->
///         ^
/// > | <?>
///       ^
/// > | <!q>
///        ^
/// > | <!--ab-->
///             ^
/// > | <![CDATA[>&<]]>
///                   ^
/// ```
pub fn continuation_declaration_inside(tokenizer: &mut Tokenizer) -> State {
    if tokenizer.tokenize_state.marker == COMMENT && tokenizer.current == Some(b'-') {
        tokenizer.consume();
        State::Next(StateName::HtmlFlowContinuationDeclarationInside)
    } else if tokenizer.current == Some(b'>') {
        tokenizer.consume();
        State::Next(StateName::HtmlFlowContinuationClose)
    } else {
        State::Retry(StateName::HtmlFlowContinuation)
    }
}

/// In closed continuation: everything we get until the eol/eof is part of it.
///
/// ```markdown
/// > | <!doctype>
///               ^
/// ```
pub fn continuation_close(tokenizer: &mut Tokenizer) -> State {
    match tokenizer.current {
        None | Some(b'\n') => {
            tokenizer.exit(Name::HtmlFlowData);
            State::Retry(StateName::HtmlFlowContinuationAfter)
        }
        _ => {
            tokenizer.consume();
            State::Next(StateName::HtmlFlowContinuationClose)
        }
    }
}

/// Done.
///
/// ```markdown
/// > | <!doctype>
///               ^
/// ```
pub fn continuation_after(tokenizer: &mut Tokenizer) -> State {
    tokenizer.exit(Name::HtmlFlow);
    tokenizer.tokenize_state.marker = 0;
    // Feel free to interrupt.
    tokenizer.interrupt = false;
    // No longer concrete.
    tokenizer.concrete = false;
    State::Ok
}

/// Before eol, expecting blank line.
///
/// ```markdown
/// > | <div>
///          ^
///   |
/// ```
pub fn blank_line_before(tokenizer: &mut Tokenizer) -> State {
    if tokenizer.parse_state.options.html_block_blank_lines
        && matches!(tokenizer.tokenize_state.marker, BASIC | COMPLETE)
    {
        tokenizer.tokenize_state.html_flow_blank_lines = 0;
        tokenizer.enter(Name::LineEnding);
        tokenizer.consume();
        tokenizer.exit(Name::LineEnding);
        tokenizer.tokenize_state.html_flow_blank_line_base = tokenizer.point.column;
        return State::Next(StateName::HtmlFlowBlankLinesCheck);
    }

    tokenizer.enter(Name::LineEnding);
    tokenizer.consume();
    tokenizer.exit(Name::LineEnding);
    State::Next(StateName::BlankLineStart)
}

/// Inspect blank lines and the indentation of the next content line.
pub fn blank_lines_check(tokenizer: &mut Tokenizer) -> State {
    // Lookahead must not retain lines outside the enclosing Markdown container.
    // Consume only the valid blank lines counted before this boundary, then close.
    if tokenizer.lazy {
        return State::Ok;
    }
    match tokenizer.current {
        Some(b'\t' | b' ') => {
            tokenizer.consume();
            State::Next(StateName::HtmlFlowBlankLinesCheck)
        }
        Some(b'\n') => {
            tokenizer.tokenize_state.html_flow_blank_lines += 1;
            tokenizer.enter(Name::LineEnding);
            tokenizer.consume();
            tokenizer.exit(Name::LineEnding);
            tokenizer.tokenize_state.html_flow_blank_line_base = tokenizer.point.column;
            State::Next(StateName::HtmlFlowBlankLinesCheck)
        }
        None => {
            if tokenizer.point.column > tokenizer.tokenize_state.html_flow_blank_line_base {
                tokenizer.tokenize_state.html_flow_blank_lines += 1;
            }
            if tokenizer.tokenize_state.html_flow_blank_lines > 0 {
                State::Ok
            } else {
                State::Nok
            }
        }
        Some(_) => {
            if tokenizer.tokenize_state.html_flow_blank_lines == 0 {
                return State::Nok;
            }

            let indent = tokenizer
                .point
                .column
                .saturating_sub(tokenizer.tokenize_state.html_flow_blank_line_base);
            let minimum = tokenizer.tokenize_state.html_flow_indent;
            let continues = if minimum == 0 {
                indent > 0
            } else {
                indent >= minimum
            };

            if continues {
                State::Nok
            } else {
                State::Ok
            }
        }
    }
}

/// Consume a retained run of blank lines, then end the HTML block.
pub fn blank_lines_end(tokenizer: &mut Tokenizer) -> State {
    tokenizer.enter(Name::LineEnding);
    tokenizer.consume();
    tokenizer.exit(Name::LineEnding);
    State::Next(StateName::HtmlFlowBlankLinesEndStart)
}

/// At the start of a retained blank line.
pub fn blank_lines_end_start(tokenizer: &mut Tokenizer) -> State {
    if tokenizer.tokenize_state.html_flow_blank_lines == 0 {
        return State::Retry(StateName::HtmlFlowContinuationAfter);
    }

    match tokenizer.current {
        Some(b'\t' | b' ') => {
            tokenizer.enter(Name::HtmlFlowData);
            tokenizer.consume();
            State::Next(StateName::HtmlFlowBlankLinesEndData)
        }
        Some(b'\n') => {
            tokenizer.enter(Name::LineEnding);
            tokenizer.consume();
            tokenizer.exit(Name::LineEnding);
            tokenizer.tokenize_state.html_flow_blank_lines -= 1;
            State::Next(StateName::HtmlFlowBlankLinesEndStart)
        }
        _ => {
            tokenizer.tokenize_state.html_flow_blank_lines = 0;
            State::Retry(StateName::HtmlFlowContinuationAfter)
        }
    }
}

/// In retained whitespace on a blank line.
pub fn blank_lines_end_data(tokenizer: &mut Tokenizer) -> State {
    match tokenizer.current {
        Some(b'\t' | b' ') => {
            tokenizer.consume();
            State::Next(StateName::HtmlFlowBlankLinesEndData)
        }
        Some(b'\n') => {
            tokenizer.exit(Name::HtmlFlowData);
            tokenizer.enter(Name::LineEnding);
            tokenizer.consume();
            tokenizer.exit(Name::LineEnding);
            tokenizer.tokenize_state.html_flow_blank_lines -= 1;
            State::Next(StateName::HtmlFlowBlankLinesEndStart)
        }
        _ => {
            tokenizer.exit(Name::HtmlFlowData);
            tokenizer.tokenize_state.html_flow_blank_lines = 0;
            State::Retry(StateName::HtmlFlowContinuationAfter)
        }
    }
}

/// Record the shallowest positive indentation of an HTML continuation line.
fn record_line_indent(tokenizer: &mut Tokenizer) {
    if !tokenizer.parse_state.options.html_block_blank_lines
        || !matches!(tokenizer.tokenize_state.marker, BASIC | COMPLETE)
    {
        return;
    }

    let start_column = tokenizer.point.column;
    let mut column = start_column;
    let mut index = tokenizer.point.index;

    while index < tokenizer.parse_state.bytes.len() {
        match tokenizer.parse_state.bytes[index] {
            b' ' => column += 1,
            b'\t' => column += TAB_SIZE - ((column - 1) % TAB_SIZE),
            _ => break,
        }
        index += 1;
    }

    if index >= tokenizer.parse_state.bytes.len()
        || matches!(tokenizer.parse_state.bytes[index], b'\n' | b'\r')
    {
        return;
    }

    let indent = column - start_column;
    if indent > 0
        && (tokenizer.tokenize_state.html_flow_indent == 0
            || indent < tokenizer.tokenize_state.html_flow_indent)
    {
        tokenizer.tokenize_state.html_flow_indent = indent;
    }
}

#[cfg(test)]
mod tests {
    use super::{record_line_indent, BASIC, COMMENT, COMPLETE};
    use crate::{event::Point, parser::parse, tokenizer::Tokenizer, ParseOptions};

    fn with_tokenizer(source: &str, options: &ParseOptions, run: impl FnOnce(&mut Tokenizer<'_>)) {
        let (_, parse_state) = parse(source, options).expect("parse test input");
        let mut tokenizer = Tokenizer::new(
            Point {
                line: 1,
                column: 1,
                index: 0,
                vs: 0,
            },
            &parse_state,
        );
        run(&mut tokenizer);
    }

    #[test]
    fn tracks_blank_lines_for_basic_and_complete_html() {
        let options = ParseOptions {
            html_block_blank_lines: true,
            ..ParseOptions::default()
        };

        crate::parser::parse("<div>\n\n  content\n\n</div>", &options)
            .expect("tracks blank lines in a basic HTML block");
        crate::parser::parse("<widget>\n\n  content\n\n</widget>", &options)
            .expect("tracks blank lines in a complete HTML block");
    }

    #[test]
    fn records_the_shallowest_indentation_for_html_continuations() {
        let options = ParseOptions {
            html_block_blank_lines: true,
            ..ParseOptions::default()
        };

        with_tokenizer("  content", &options, |tokenizer| {
            tokenizer.tokenize_state.marker = BASIC;
            record_line_indent(tokenizer);
            assert_eq!(tokenizer.tokenize_state.html_flow_indent, 2);

            tokenizer.tokenize_state.marker = COMPLETE;
            tokenizer.tokenize_state.html_flow_indent = 3;
            record_line_indent(tokenizer);
            assert_eq!(tokenizer.tokenize_state.html_flow_indent, 2);

            tokenizer.tokenize_state.marker = COMMENT;
            tokenizer.tokenize_state.html_flow_indent = 0;
            record_line_indent(tokenizer);
            assert_eq!(tokenizer.tokenize_state.html_flow_indent, 0);
        });
    }

    #[test]
    fn blank_lines_after_comments_use_the_general_flow_path() {
        let options = ParseOptions {
            html_block_blank_lines: true,
            ..ParseOptions::default()
        };

        parse("<!-- comment -->\n\ncontent", &options)
            .expect("parses blank lines after an HTML comment");
    }
}
