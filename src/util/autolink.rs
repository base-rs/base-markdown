// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Angle autolink recognition, shared by both inline producers.
//!
//! Unlike `CommonMark`, which accepts any scheme of 2–32 characters, only `http://` and `https://` with a host, `mailto:` with an address (schemes in any case) and the email form complete a link. Prose such as `Option<std::io::Error>` or XML such as `<xs:element>` falls through to raw HTML or text.
use crate::util::constant::AUTOLINK_DOMAIN_SIZE_MAX;

/// Each allowed scheme is a prefix of one of these names.
const HTTPS: &[u8] = b"https";
const MAILTO: &[u8] = b"mailto";

#[derive(Clone, Copy, Debug, Default)]
pub(crate) enum Autolink {
    #[default]
    Open,
    /// The first `size` bytes match `name`, case-insensitively.
    Scheme(&'static [u8], u8),
    Email,
    DomainStart,
    Domain(u8, bool),
    /// After `http:` or `https:`, with this many of the required `//` seen.
    Slashes(u8),
    /// Before the first byte of the host or address, which must not be `/`, `?` or `#`.
    Target,
    Url,
}

impl Autolink {
    pub fn email(self) -> bool {
        matches!(self, Self::Domain(..))
    }

    /// Input starts after `<`. `Some(true)` completes a link; `None` rejects it.
    pub fn advance(&mut self, byte: u8) -> Option<bool> {
        use Autolink::{Domain, DomainStart, Email, Open, Scheme, Slashes, Target, Url};
        *self = match *self {
            Open if byte.eq_ignore_ascii_case(&b'h') => Scheme(HTTPS, 1),
            Open if byte.eq_ignore_ascii_case(&b'm') => Scheme(MAILTO, 1),
            Open if atext(byte) => Email,
            Scheme(name, size)
                if byte == b':' && name == MAILTO && usize::from(size) == name.len() =>
            {
                Target
            }
            Scheme(name, size)
                if byte == b':' && name == HTTPS && usize::from(size) >= name.len() - 1 =>
            {
                Slashes(0)
            }
            Scheme(name, size)
                if usize::from(size) < name.len()
                    && byte.eq_ignore_ascii_case(&name[usize::from(size)]) =>
            {
                Scheme(name, size + 1)
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
            Domain(_, false) | Url if byte == b'>' => return Some(true),
            Domain(_, false) if byte == b'.' => DomainStart,
            Domain(size, _)
                if usize::from(size) < AUTOLINK_DOMAIN_SIZE_MAX
                    && (byte.is_ascii_alphanumeric() || byte == b'-') =>
            {
                Domain(size + 1, byte == b'-')
            }
            Slashes(0) if byte == b'/' => Slashes(1),
            Slashes(_) if byte == b'/' => Target,
            Open | Domain(..) | Slashes(_) => return None,
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
