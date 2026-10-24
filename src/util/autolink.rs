// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Angle autolink recognition, shared by both inline producers.
use crate::util::constant::{AUTOLINK_DOMAIN_SIZE_MAX, AUTOLINK_SCHEME_SIZE_MAX};

#[derive(Clone, Copy, Debug, Default)]
pub(crate) enum Autolink {
    #[default]
    Open,
    Scheme(u8),
    Email,
    DomainStart,
    Domain(u8, bool),
    Url,
}

impl Autolink {
    pub fn email(self) -> bool {
        matches!(self, Self::Domain(..))
    }

    /// Input starts after `<`. `Some(true)` completes a link; `None` rejects it.
    pub fn advance(&mut self, byte: u8) -> Option<bool> {
        use Autolink::{Domain, DomainStart, Email, Open, Scheme, Url};
        *self = match *self {
            Open if byte.is_ascii_alphabetic() => Scheme(1),
            Open if atext(byte) => Email,
            Scheme(size) if byte == b':' && size >= 2 => Url,
            Scheme(size)
                if usize::from(size) < AUTOLINK_SCHEME_SIZE_MAX
                    && (byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.')) =>
            {
                Scheme(size + 1)
            }
            Scheme(_) | Email => {
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
            Open | Domain(..) => return None,
            Url if byte <= b' ' || matches!(byte, b'<' | 0x7f) => return None,
            Url => Url,
        };
        Some(false)
    }
}

fn atext(byte: u8) -> bool {
    matches!(byte, b'#'..=b'\'' | b'*' | b'+' | b'-'..=b'9' | b'=' | b'?' | b'A'..=b'Z' | b'^'..=b'~')
}
