// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Angle autolink recognition, shared by both inline producers.
//!
//! Unlike `CommonMark`, which accepts any scheme of 2–32 characters, only the schemes in [`SCHEMES`][crate::util::scheme::SCHEMES], in any case, followed by what their [`SchemeKind`] requires, and the email form complete a link. Prose such as `Option<std::io::Error>` or XML such as `<xs:element>` falls through to raw HTML or text.
use crate::util::{
    constant::AUTOLINK_DOMAIN_SIZE_MAX,
    scheme::{pack, packed_full, packed_scheme, phone, SchemeKind},
};

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Autolink {
    state: State,
    /// Whether an address scheme such as `mailto:` precedes the email, so the text is already the destination.
    prefixed: bool,
}

#[derive(Clone, Copy, Debug, Default)]
enum State {
    #[default]
    Open,
    /// The leading ASCII letters, packed with [`pack`], which can be a scheme name or an email local part.
    Scheme(u64),
    /// After an address scheme's `:`, before the local part.
    Local,
    Email,
    DomainStart,
    Domain(u8, bool),
    /// After a host scheme's `:`, with this many of the required `//` seen.
    Slashes(u8),
    /// Before the first byte of the host, which must not be `/`, `?` or `#`.
    Target,
    Url,
    /// After a phone scheme's `:`, and whether a digit has been seen.
    Phone(bool),
}

impl Autolink {
    pub fn email(self) -> bool {
        matches!(self.state, State::Domain(..)) && !self.prefixed
    }

    /// Input starts after `<`. `Some(true)` completes a link; `None` rejects it.
    pub fn advance(&mut self, byte: u8) -> Option<bool> {
        use State::{Domain, DomainStart, Email, Local, Open, Phone, Scheme, Slashes, Target, Url};
        self.state = match self.state {
            Open if byte.is_ascii_alphabetic() => Scheme(pack(0, byte)),
            Open | Local if atext(byte) => Email,
            Scheme(name) if byte == b':' => match packed_scheme(name)?.kind {
                SchemeKind::Host => Slashes(0),
                SchemeKind::Address => {
                    self.prefixed = true;
                    Local
                }
                SchemeKind::Phone => Phone(false),
            },
            Scheme(name) if byte.is_ascii_alphabetic() && !packed_full(name) => {
                Scheme(pack(name, byte))
            }
            Scheme(..) | Email => {
                if byte == b'@' {
                    DomainStart
                } else if atext(byte) {
                    Email
                } else {
                    return None;
                }
            }
            DomainStart => {
                if !byte.is_ascii_alphanumeric() {
                    return None;
                }
                Domain(1, false)
            }
            Domain(_, false) | Url | Phone(true) if byte == b'>' => return Some(true),
            Domain(_, false) if byte == b'.' => DomainStart,
            Domain(size, _)
                if usize::from(size) < AUTOLINK_DOMAIN_SIZE_MAX
                    && (byte.is_ascii_alphanumeric() || byte == b'-') =>
            {
                Domain(size + 1, byte == b'-')
            }
            Phone(digit) if phone(byte) => Phone(digit || byte.is_ascii_digit()),
            Slashes(0) if byte == b'/' => Slashes(1),
            Slashes(_) if byte == b'/' => Target,
            Open | Local | Domain(..) | Slashes(_) | Phone(_) => return None,
            Target | Url if byte <= b' ' || matches!(byte, b'<' | 0x7f) => return None,
            Target if matches!(byte, b'/' | b'?' | b'#' | b'>') => return None,
            Target | Url => Url,
        };
        Some(false)
    }
}

/// Recognize an autolink in `bytes`, which start after `<`. Returns the length including `>`, and whether it is an email autolink.
pub(crate) fn recognize(bytes: impl IntoIterator<Item = u8>) -> Option<(usize, bool)> {
    let mut link = Autolink::default();
    for (offset, byte) in bytes.into_iter().enumerate() {
        if link.advance(byte)? {
            return Some((offset + 1, link.email()));
        }
    }
    None
}

/// Email local-part characters, which can also follow `<` at the start of an autolink.
pub(crate) fn atext(byte: u8) -> bool {
    matches!(byte, b'#'..=b'\'' | b'*' | b'+' | b'-'..=b'9' | b'=' | b'?' | b'A'..=b'Z' | b'^'..=b'~')
}
