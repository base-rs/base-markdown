// Released under the MIT License.
// Copyright, 2022-2025, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

//! HTML (text) occurs in the [text][] content type.
//!
//! ## Grammar
//!
//! HTML (text) forms with the following BNF
//! (<small>see [construct][crate::construct] for character groups</small>):
//!
//! ```bnf
//! html_text ::= comment | instruction | declaration | cdata | tag_close | tag_open
//!
//! ; Restriction: the text is not allowed to start with `>`, `->`, or to contain `--`.
//! comment ::= '<!--' *byte '-->'
//! instruction ::= '<?' *byte '?>'
//! declaration ::= '<!' ascii_alphabetic *byte '>'
//! ; Restriction: the text is not allowed to contain `]]`.
//! cdata ::= '<![CDATA[' *byte ']]>'
//! tag_close ::= '</' tag_name [space_or_tab_eol] '>'
//! opening_tag ::= '<' tag_name *(space_or_tab_eol attribute) [[space_or_tab_eol] '/'] [space_or_tab_eol] '>'
//!
//! tag_name ::= ascii_alphabetic *( '-' | ascii_alphanumeric | [':' when html_tag_namespaces] )
//! attribute ::= attribute_name [[space_or_tab_eol] '=' [space_or_tab_eol] attribute_value]
//! attribute_name ::= (':' | '_' | ascii_alphabetic) *('-' | '.' | ':' | '_' | ascii_alphanumeric)
//! attribute_value ::= '"' *(byte - '"') '"' | "'" *(byte - "'")  "'" | 1*(text - '"' - "'" - '/' - '<' - '=' - '>' - '`')
//! ```
//!
//! The grammar for HTML in markdown does not follow the rules of parsing
//! HTML according to the [*§ 13.2 Parsing HTML documents* in the HTML
//! spec][html_parsing].
//! See the related flow construct [HTML (flow)][html_flow] for more info.
//!
//! Because the **tag open** and **tag close** productions in the grammar form
//! with just tags instead of complete elements, it is possible to interleave
//! (a word for switching between languages) markdown and HTML together.
//! For example:
//!
//! ```markdown
//! This is equivalent to <code>*emphasised* code</code>.
//! ```
//!
//! ## Tokens
//!
//! * [`HtmlText`][Name::HtmlText]
//! * [`HtmlTextData`][Name::HtmlTextData]
//!
//! ## References
//!
//! * [`html-text.js` in `micromark`](https://github.com/micromark/micromark/blob/main/packages/micromark-core-commonmark/dev/lib/html-text.js)
//! * [*§ 6.6 Raw HTML* in `CommonMark`](https://spec.commonmark.org/0.31/#raw-html)
//!
//! [text]: crate::construct::text
//! [html_flow]: crate::construct::html_flow
//! [html_parsing]: https://html.spec.whatwg.org/multipage/parsing.html#parsing
use crate::construct::partial_space_or_tab::space_or_tab;
use crate::event::Name;
use crate::state::{Name as StateName, State};
use crate::tokenizer::Tokenizer;
use crate::util::html_text::HtmlText;

pub fn start(tokenizer: &mut Tokenizer) -> State {
    if tokenizer.current != Some(b'<') || !tokenizer.parse_state.options.constructs.html_text {
        return State::Nok;
    }
    tokenizer.tokenize_state.html_inline = HtmlText::default();
    tokenizer.enter(Name::HtmlText);
    tokenizer.enter(Name::HtmlTextData);
    tokenizer.consume();
    State::Next(StateName::HtmlTextScan)
}

pub fn scan(tokenizer: &mut Tokenizer) -> State {
    let Some(byte) = tokenizer.current else {
        return State::Nok;
    };
    match tokenizer
        .tokenize_state
        .html_inline
        .advance(byte, tokenizer.parse_state.options.html_tag_namespaces)
    {
        None => State::Nok,
        Some(true) => {
            tokenizer.consume();
            tokenizer.exit(Name::HtmlTextData);
            tokenizer.exit(Name::HtmlText);
            State::Ok
        }
        Some(false) if byte == b'\n' => {
            tokenizer.attempt(State::Next(StateName::HtmlTextScan), State::Nok);
            State::Retry(StateName::HtmlTextLineEndingBefore)
        }
        Some(false) => {
            tokenizer.consume();
            State::Next(StateName::HtmlTextScan)
        }
    }
}

pub fn line_ending_before(tokenizer: &mut Tokenizer) -> State {
    match tokenizer.current {
        Some(b'\n') => {
            tokenizer.exit(Name::HtmlTextData);
            tokenizer.enter(Name::LineEnding);
            tokenizer.consume();
            tokenizer.exit(Name::LineEnding);
            State::Next(StateName::HtmlTextLineEndingAfter)
        }
        _ => unreachable!("expected eol"),
    }
}

/// After eol, at optional whitespace.
///
/// > 👉 **Note**: we can’t have blank lines in text, so no need to worry about
/// > empty tokens.
///
/// ```markdown
///   | a <!--a
/// > | b-->
///     ^
/// ```
pub fn line_ending_after(tokenizer: &mut Tokenizer) -> State {
    if matches!(tokenizer.current, Some(b'\t' | b' ')) {
        tokenizer.attempt(
            State::Next(StateName::HtmlTextLineEndingAfterPrefix),
            State::Nok,
        );
        State::Retry(space_or_tab(tokenizer))
    } else {
        State::Retry(StateName::HtmlTextLineEndingAfterPrefix)
    }
}

/// After eol, after optional whitespace.
///
/// > 👉 **Note**: we can’t have blank lines in text, so no need to worry about
/// > empty tokens.
///
/// ```markdown
///   | a <!--a
/// > | b-->
///     ^
/// ```
pub fn line_ending_after_prefix(tokenizer: &mut Tokenizer) -> State {
    tokenizer.enter(Name::HtmlTextData);
    State::Ok
}
