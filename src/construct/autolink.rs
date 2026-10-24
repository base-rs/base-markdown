// Released under the MIT License.
// Copyright, 2022-2025, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

//! Autolink occurs in the [text][] content type.
//!
//! ## Grammar
//!
//! Autolink forms with the following BNF
//! (<small>see [construct][crate::construct] for character groups</small>):
//!
//! ```bnf
//! autolink ::= '<' (url | email) '>'
//!
//! url ::= protocol *url_byte
//! protocol ::= ascii_alphabetic 0*31(protocol_byte) ':'
//! protocol_byte ::= '+' '-' '.' ascii_alphanumeric
//! url_byte ::= byte - ascii_control - ' '
//!
//! email ::= 1*ascii_atext '@' email_domain *('.' email_domain)
//! ; Restriction: up to (including) 63 character are allowed in each domain.
//! email_domain ::= ascii_alphanumeric *(ascii_alphanumeric | '-' ascii_alphanumeric)
//!
//! ascii_atext ::= ascii_alphanumeric | '!' | '"' | '#' | '$' | '%' | '&' | '\'' | '*' | '+' | '-' | '/' | '=' | '?' | '^' | '_' | '`' | '{' | '|' | '}' | '~'
//! ```
//!
//! The maximum allowed size of a scheme is `31` (inclusive), which is defined
//! in [`AUTOLINK_SCHEME_SIZE_MAX`][].
//! The maximum allowed size of a domain is `63` (inclusive), which is defined
//! in [`AUTOLINK_DOMAIN_SIZE_MAX`][].
//!
//! The grammar for autolinks is quite strict and prohibits the use of ASCII control
//! characters or spaces.
//! To use non-ascii characters and otherwise impossible characters in URLs,
//! you can use percent encoding:
//!
//! ```markdown
//! <https://example.com/alpha%20bravo>
//! ```
//!
//! Yields:
//!
//! ```html
//! <p><a href="https://example.com/alpha%20bravo">https://example.com/alpha%20bravo</a></p>
//! ```
//!
//! There are several cases where incorrect encoding of URLs would, in other
//! languages, result in a parse error.
//! In markdown, there are no errors, and URLs are normalized.
//! In addition, many characters are percent encoded
//! ([`sanitize_uri`][sanitize_uri]).
//! For example:
//!
//! ```markdown
//! <https://a👍b%>
//! ```
//!
//! Yields:
//!
//! ```html
//! <p><a href="https://a%F0%9F%91%8Db%25">https://a👍b%</a></p>
//! ```
//!
//! Interestingly, there are a couple of things that are valid autolinks in
//! markdown but in HTML would be valid tags, such as `<svg:rect>` and
//! `<xml:lang/>`.
//! However, because `CommonMark` employs a naïve HTML parsing algorithm, those
//! are not considered HTML.
//!
//! While `CommonMark` restricts links from occurring in other links in the
//! case of labels (see [label end][label_end]), this restriction is not in
//! place for autolinks inside labels:
//!
//! ```markdown
//! [<https://example.com>](#)
//! ```
//!
//! Yields:
//!
//! ```html
//! <p><a href="#"><a href="https://example.com">https://example.com</a></a></p>
//! ```
//!
//! The generated output, in this case, is invalid according to HTML.
//! When a browser sees that markup, it will instead parse it as:
//!
//! ```html
//! <p><a href="#"></a><a href="https://example.com">https://example.com</a></p>
//! ```
//!
//! ## HTML
//!
//! Autolinks relate to the `<a>` element in HTML.
//! See [*§ 4.5.1 The `a` element*][html_a] in the HTML spec for more info.
//! When an email autolink is used (so, without a protocol), the string
//! `mailto:` is prepended before the email, when generating the `href`
//! attribute of the hyperlink.
//!
//! ## Recommendation
//!
//! It is recommended to use labels ([label start link][label_start_link],
//! [label end][label_end]), either with a resource or a definition
//! ([definition][]), instead of autolinks, as those allow more characters in
//! URLs, and allow relative URLs and `www.` URLs.
//! They also allow for descriptive text to explain the URL in prose.
//!
//! ## Tokens
//!
//! * [`Autolink`][Name::Autolink]
//! * [`AutolinkEmail`][Name::AutolinkEmail]
//! * [`AutolinkMarker`][Name::AutolinkMarker]
//! * [`AutolinkProtocol`][Name::AutolinkProtocol]
//!
//! ## References
//!
//! * [`autolink.js` in `micromark`](https://github.com/micromark/micromark/blob/main/packages/micromark-core-commonmark/dev/lib/autolink.js)
//! * [*§ 6.4 Autolinks* in `CommonMark`](https://spec.commonmark.org/0.31/#autolinks)
//!
//! [text]: crate::construct::text
//! [definition]: crate::construct::definition
//! [label_start_link]: crate::construct::label_start_link
//! [label_end]: crate::construct::label_end
//! [autolink_scheme_size_max]: crate::util::constant::AUTOLINK_SCHEME_SIZE_MAX
//! [autolink_domain_size_max]: crate::util::constant::AUTOLINK_DOMAIN_SIZE_MAX
//! [sanitize_uri]: crate::util::sanitize_uri
//! [html_a]: https://html.spec.whatwg.org/multipage/text-level-semantics.html#the-a-element
use crate::event::Name;
use crate::state::{Name as StateName, State};
use crate::tokenizer::Tokenizer;
use crate::util::autolink::Autolink;

pub fn start(tokenizer: &mut Tokenizer) -> State {
    if !tokenizer.parse_state.options.constructs.autolink || tokenizer.current != Some(b'<') {
        return State::Nok;
    }
    tokenizer.tokenize_state.autolink = Autolink::default();
    tokenizer.enter(Name::Autolink);
    tokenizer.enter(Name::AutolinkMarker);
    tokenizer.consume();
    tokenizer.exit(Name::AutolinkMarker);
    tokenizer.enter(Name::AutolinkProtocol);
    State::Next(StateName::AutolinkScan)
}

pub fn scan(tokenizer: &mut Tokenizer) -> State {
    let Some(byte) = tokenizer.current else {
        return State::Nok;
    };
    match tokenizer.tokenize_state.autolink.advance(byte) {
        None => State::Nok,
        Some(false) => {
            tokenizer.consume();
            State::Next(StateName::AutolinkScan)
        }
        Some(true) => {
            let index = tokenizer.events.len();
            tokenizer.exit(Name::AutolinkProtocol);
            if tokenizer.tokenize_state.autolink.email() {
                tokenizer.events[index - 1].name = Name::AutolinkEmail;
                tokenizer.events[index].name = Name::AutolinkEmail;
            }
            tokenizer.enter(Name::AutolinkMarker);
            tokenizer.consume();
            tokenizer.exit(Name::AutolinkMarker);
            tokenizer.exit(Name::Autolink);
            State::Ok
        }
    }
}
