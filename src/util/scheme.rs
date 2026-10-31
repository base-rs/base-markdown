// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! The link schemes shared by angle autolinks, GFM literal autolinks and the HTML renderer's safe `href` check.

/// What must follow `scheme:` for a link to form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SchemeKind {
    /// `://` and then a host, such as `https://example.com`. Also recognized as a bare GFM literal.
    Host,
    /// One email address, `local@domain`, such as `mailto:a@example.com`. Also recognized as a bare GFM literal. In `<…>`, a `?query` may follow the address, such as `<mailto:a@example.com?subject=x>`, and so may a `/resource` when `resource` is set, as in `<xmpp:a@example.com/phone>`, which is also allowed in the bare form.
    Address { resource: bool },
    /// One or more of `+`, ASCII digits, `-`, `.`, `(` and `)`, with at least one digit, such as `tel:+1-555-0100`. Only recognized in `<…>`, never as a bare literal.
    Phone,
}

#[derive(Debug)]
pub(crate) struct Scheme {
    /// The lowercase name, matched ASCII case-insensitively.
    pub name: &'static str,
    pub kind: SchemeKind,
}

/// Every scheme that can form a link, and the only non-relative schemes the HTML renderer keeps in a safe `href`.
///
/// To add a scheme, its name must be lowercase ASCII letters and must not be an HTML or XML element or namespace prefix (such as `svg`, `math`, `xml`, `xmlns` or `xlink`), so `<svg:rect>` stays HTML. It must not read like code, as `std` does in `<std::io::Error>`, and what follows it must fit one of the [`SchemeKind`]s. Never add `javascript`, `vbscript`, `data` or `file`.
pub(crate) const SCHEMES: [Scheme; 11] = [
    Scheme {
        name: "http",
        kind: SchemeKind::Host,
    },
    Scheme {
        name: "https",
        kind: SchemeKind::Host,
    },
    Scheme {
        name: "ftp",
        kind: SchemeKind::Host,
    },
    Scheme {
        name: "sftp",
        kind: SchemeKind::Host,
    },
    Scheme {
        name: "ssh",
        kind: SchemeKind::Host,
    },
    Scheme {
        name: "irc",
        kind: SchemeKind::Host,
    },
    Scheme {
        name: "ircs",
        kind: SchemeKind::Host,
    },
    Scheme {
        name: "mailto",
        kind: SchemeKind::Address { resource: false },
    },
    Scheme {
        name: "xmpp",
        kind: SchemeKind::Address { resource: true },
    },
    Scheme {
        name: "tel",
        kind: SchemeKind::Phone,
    },
    Scheme {
        name: "sms",
        kind: SchemeKind::Phone,
    },
];

/// The longest scheme name, which bounds every scheme scan.
pub(crate) const SCHEME_SIZE_MAX: usize = {
    let mut index = 0;
    let mut size = 0;
    while index < SCHEMES.len() {
        if SCHEMES[index].name.len() > size {
            size = SCHEMES[index].name.len();
        }
        index += 1;
    }
    size
};

// A packed name holds at most eight letters.
const _: () = assert!(SCHEME_SIZE_MAX <= 8);

/// The scheme names, in table order.
pub(crate) const SCHEME_NAMES: [&str; SCHEMES.len()] = {
    let mut names = [""; SCHEMES.len()];
    let mut index = 0;
    while index < SCHEMES.len() {
        names[index] = SCHEMES[index].name;
        index += 1;
    }
    names
};

/// Append an ASCII `letter`, lowercased, to a packed scheme name. Packing a name one byte per letter, which fits because names are at most eight letters, makes matching one integer comparison per scheme.
pub(crate) const fn pack(packed: u64, letter: u8) -> u64 {
    packed << 8 | (letter | 0x20) as u64
}

/// Whether a packed name already holds [`SCHEME_SIZE_MAX`] letters.
pub(crate) const fn packed_full(packed: u64) -> bool {
    packed >> ((SCHEME_SIZE_MAX - 1) * 8) != 0
}

/// The packed scheme names, in table order.
const PACKED_NAMES: [u64; SCHEMES.len()] = {
    let mut names = [0; SCHEMES.len()];
    let mut index = 0;
    while index < SCHEMES.len() {
        let name = SCHEMES[index].name.as_bytes();
        let mut offset = 0;
        while offset < name.len() {
            names[index] = pack(names[index], name[offset]);
            offset += 1;
        }
        index += 1;
    }
    names
};

/// Find the scheme whose name, packed with [`pack`], is `packed`.
pub(crate) fn packed_scheme(packed: u64) -> Option<&'static Scheme> {
    PACKED_NAMES
        .iter()
        .position(|name| *name == packed)
        .map(|index| &SCHEMES[index])
}

/// Find the scheme named `name`, ASCII case-insensitively.
pub(crate) fn scheme(name: &[u8]) -> Option<&'static Scheme> {
    if name.len() > SCHEME_SIZE_MAX || !name.iter().all(u8::is_ascii_alphabetic) {
        return None;
    }
    packed_scheme(name.iter().fold(0, |packed, letter| pack(packed, *letter)))
}

/// Bytes allowed in a [`SchemeKind::Phone`] target.
pub(crate) fn phone(byte: u8) -> bool {
    matches!(byte, b'+' | b'0'..=b'9' | b'-' | b'.' | b'(' | b')')
}

#[cfg(test)]
mod tests {
    use super::{scheme, SchemeKind, SCHEMES, SCHEME_NAMES, SCHEME_SIZE_MAX};

    #[test]
    fn names_are_lowercase_letters_and_safe() {
        for entry in &SCHEMES {
            assert!(
                entry.name.bytes().all(|byte| byte.is_ascii_lowercase()),
                "{}",
                entry.name
            );
            assert!(entry.name.len() <= SCHEME_SIZE_MAX);
        }
        for name in [
            "javascript",
            "vbscript",
            "data",
            "file",
            "svg",
            "math",
            "xml",
            "xmlns",
            "xlink",
        ] {
            assert!(scheme(name.as_bytes()).is_none(), "{}", name);
        }
        assert_eq!(SCHEME_SIZE_MAX, 6);
        assert!(scheme(b"ssh1").is_none());
        assert!(scheme(b"mailtoo").is_none());
        assert_eq!(SCHEME_NAMES.len(), SCHEMES.len());
        assert_eq!(scheme(b"HTTPS").unwrap().kind, SchemeKind::Host);
        assert_eq!(
            scheme(b"Mailto").unwrap().kind,
            SchemeKind::Address { resource: false }
        );
        assert_eq!(scheme(b"tel").unwrap().kind, SchemeKind::Phone);
    }
}
