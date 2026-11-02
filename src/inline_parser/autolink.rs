// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Literal URL and email recognition over source bytes, without tokens or owned strings.
use super::AutolinkPrefix;
use crate::util::{
    char::{classify, Kind},
    scheme::{scheme, SchemeKind, SCHEME_SIZE_MAX},
};
use alloc::vec::Vec;

pub(super) struct Match {
    pub start: usize,
    pub end: usize,
    pub prefix: AutolinkPrefix,
}

/// Terminal punctuation depends on the rest of the word, not its URL prefix. Reuse that lookahead when an invalid domain leaves later prefixes to be scanned in the same fragment.
#[derive(Default)]
pub(super) struct Trail {
    start: usize,
    tail: usize,
    boundary: usize,
    limit: usize,
}

impl Trail {
    fn suffix(&mut self, source: &str, start: usize, limit: usize) -> (usize, usize) {
        if self.limit == limit && start >= self.start && start < self.boundary {
            return (self.tail.max(start), self.boundary);
        }
        let (tail, boundary) = terminal_suffix(source, start, limit);
        *self = Self {
            start,
            tail,
            boundary,
            limit,
        };
        (tail, boundary)
    }
}

/// The prefix the parser adds to `text` to form its destination, when all of `text`, with
/// nothing around it, is one GFM literal autolink.
pub(crate) fn literal_autolink_prefix(text: &str) -> Option<&'static str> {
    let whole = |found: Option<Match>| {
        found
            .filter(|found| found.start == 0 && found.end == text.len())
            .map(|found| found.prefix.as_str())
    };
    let bytes = text.as_bytes();
    let marker = if bytes.len() >= 4 && bytes[..4].eq_ignore_ascii_case(b"www.") {
        Some(3)
    } else {
        text.find(':')
    };
    marker
        .and_then(|marker| whole(web(text, 0, marker, text.len(), &mut Trail::default())))
        .or_else(|| {
            text.find('@')
                .and_then(|at| whole(email(text, 0, at, text.len())))
        })
}

/// Add to `markers`, in order, the offsets from `from` on in `text` of the bytes that make GFM
/// literal autolinks of `text` as it is written in phrasing: the colon after a scheme, the dot
/// after `www`, and the `@` of an email. A backslash before each of them leaves no literal, and
/// keeps the others from forming. The text before `from` is written already, and `www_start`
/// says whether what comes before `text` lets `www` start a literal.
///
/// Whether a literal forms is decided within `text`. In phrasing, what follows it is another
/// node, which is written starting with punctuation. That ends a domain, except `_` from `_`
/// attention, which a host name cannot have, so a domain that runs on into the next node is never
/// valid, and `www.` or `https://` at the end of `text` needs no escape. A literal that would run on into a following text
/// sibling, which only a tree that is not normalized has, is not found, nor in image alt one that
/// would run on into the text of a link, which is written as it is. The scan
/// is linear: domains are scanned once, except that a `www` after `_` within a scanned domain is
/// taken to form, and an email's local part is scanned back to the byte before it.
pub(crate) fn literal_markers(text: &str, from: usize, www_start: bool, markers: &mut Vec<usize>) {
    let bytes = text.as_bytes();
    let mut suffixes = Trail::default();
    let mut scanned = 0;
    // Literals whose domains are invalid, which an escape before a later `@` can make valid.
    let mut invalid = Vec::new();
    for marker in memchr::memchr2_iter(b':', b'.', &bytes[from..]).map(|at| from + at) {
        let Some((start, domain_start, _)) = web_prefix(bytes, 0, marker, bytes.len()) else {
            continue;
        };
        if start == 0 && bytes[marker] == b'.' && !www_start {
            continue;
        }
        if domain_start < scanned {
            markers.push(marker);
            continue;
        }
        let (tail, boundary) = suffixes.suffix(text, domain_start, bytes.len());
        let (end, valid) = domain(text, domain_start, tail, boundary);
        scanned = end;
        if valid {
            markers.push(marker);
        } else {
            invalid.push((marker, domain_start, end));
        }
    }
    let web = markers.len();
    let mut next = 0;
    for at in memchr::memchr_iter(b'@', &bytes[from..]).map(|at| from + at) {
        let mut local = at;
        while local > 0 && local_part(bytes[local - 1]) {
            local -= 1;
        }
        // An escaped byte ends the run of text an email is found in, and the next run starts
        // after it, as it does after the escapes written before the dots after `www`.
        let backslashes = bytes[..local]
            .iter()
            .rev()
            .take_while(|&&byte| byte == b'\\')
            .count();
        let mut min = if backslashes == 0 {
            0
        } else {
            local + backslashes % 2
        };
        while next < web && markers[next] < at {
            if markers[next] >= local {
                min = markers[next] + 1;
            }
            next += 1;
        }
        if email(text, min, at, bytes.len()).is_some() {
            markers.push(at);
        }
    }
    // An escape that a later email gets ends a literal at its `@`, which leaves out the trailing
    // punctuation before it, so `www.a..@b.cd` links `www.a` once its `@` is escaped. Such a domain
    // ends at the `@`: what is between is in the local part, and the only byte there that ends a
    // domain before it is `+`, which is not trailing. Each domain is scanned once more at most.
    let emails = markers.len();
    let mut next = web;
    for (marker, domain_start, end) in invalid {
        while next < emails && markers[next] < end {
            next += 1;
        }
        let Some(&at) = markers[web..emails].get(next - web) else {
            break;
        };
        if end == at {
            let (tail, boundary) = terminal_suffix(text, domain_start, at);
            if domain(text, domain_start, tail, boundary).1 {
                markers.push(marker);
            }
        }
    }
    // The web markers found first and the emails are each in order, and so are those found last.
    if markers.len() > emails || (emails > web && web > 0) {
        markers.sort_unstable();
    }
}

/// Recognize a protocol colon or the dot after `www`. The lower bound excludes stripped container prefixes and preceding inline constructs.
pub(super) fn web(
    source: &str,
    min: usize,
    marker: usize,
    limit: usize,
    suffix_cache: &mut Trail,
) -> Option<Match> {
    let bytes = source.as_bytes();
    let (start, domain_start, prefix) = web_prefix(bytes, min, marker, limit)?;
    let (tail, boundary) = suffix_cache.suffix(source, domain_start, limit);
    let (mut at, valid) = domain(source, domain_start, tail, boundary);
    if !valid {
        return None;
    }
    let mut opening = 0;
    let mut closing = 0;
    while at < boundary {
        let byte = bytes[at];
        if at >= tail && is_trailing(byte) && (byte != b')' || closing >= opening) {
            break;
        }
        if byte == b'(' {
            opening += 1;
        }
        if byte == b')' {
            closing += 1;
        }
        at += if byte.is_ascii() {
            1
        } else {
            source[at..limit].chars().next().unwrap().len_utf8()
        };
    }
    Some(Match {
        start,
        end: at,
        prefix,
    })
}

/// The start, the domain start and the prefix of a literal whose protocol colon or `www` dot is
/// at `marker`, judged from what comes before the domain.
fn web_prefix(
    bytes: &[u8],
    min: usize,
    marker: usize,
    limit: usize,
) -> Option<(usize, usize, AutolinkPrefix)> {
    match bytes[marker] {
        b':' => {
            if marker + 3 > limit || &bytes[marker + 1..marker + 3] != b"//" {
                return None;
            }
            let start = scheme_start(bytes, min, marker, u8::is_ascii_alphabetic);
            if !scheme(&bytes[start..marker]).is_some_and(|scheme| scheme.kind == SchemeKind::Host)
            {
                return None;
            }
            Some((start, marker + 3, AutolinkPrefix::None))
        }
        b'.' => {
            // A bare `www.` is no link, whether the text or the literal ends after it.
            if marker < min + 3
                || !bytes[marker - 3..marker].eq_ignore_ascii_case(b"www")
                || marker + 1 == limit
                || bytes[marker + 1] == b'\\'
            {
                return None;
            }
            let start = marker - 3;
            if start > min && !www_previous(bytes[start - 1]) {
                return None;
            }
            // The domain is what follows `www.`, so `www.` needs one: `www. a` is no link.
            Some((start, marker + 1, AutolinkPrefix::Http))
        }
        _ => None,
    }
}

/// Whether `www` after `byte` can start a literal.
pub(crate) fn www_previous(byte: u8) -> bool {
    matches!(
        byte,
        b'\t' | b'\r' | b'\n' | b' ' | b'(' | b'*' | b'_' | b'[' | b']' | b'~'
    )
}

/// Where the domain that starts at `at` ends, and whether it is a host name as RFC 952 and RFC
/// 1123 define it, with non-ASCII alphanumerics allowed for internationalized names (RFC 5890):
/// labels of letters, digits and `-`, separated by `.`, none empty and none starting or ending
/// with `-`. The domain runs up to the first byte that cannot be in one or the trailing
/// punctuation before the end of the literal, and an `_` in it makes it invalid rather than
/// ending it, so `www.a_b.c` is no link at all rather than a link to `www.a`.
fn domain(source: &str, mut at: usize, tail: usize, boundary: usize) -> (usize, bool) {
    let bytes = source.as_bytes();
    let mut valid = true;
    // Whether the current label is empty, and whether its last byte is `-`.
    let mut empty = true;
    let mut hyphen = false;
    while at < boundary {
        match bytes[at] {
            b'.' | b'_' if at >= tail => break,
            b'.' => {
                valid &= !empty && !hyphen;
                empty = true;
                at += 1;
            }
            b'_' => {
                valid = false;
                at += 1;
            }
            b'-' => {
                valid &= !empty;
                empty = false;
                hyphen = true;
                at += 1;
            }
            byte if byte.is_ascii() => {
                if ascii_whitespace(byte) || byte.is_ascii_punctuation() {
                    break;
                }
                empty = false;
                hyphen = false;
                at += 1;
            }
            _ => {
                let character = source[at..boundary].chars().next().unwrap();
                if classify(character) != Kind::Other {
                    break;
                }
                empty = false;
                hyphen = false;
                at += character.len_utf8();
            }
        }
    }
    (at, valid && !empty && !hyphen)
}

/// Find the terminal punctuation run once, so each URL needs linear work even with long punctuation or entity-like suffixes.
fn terminal_suffix(source: &str, start: usize, limit: usize) -> (usize, usize) {
    let bytes = source.as_bytes();
    let mut at = start;
    let mut tail = start;
    while at < limit {
        match bytes[at] {
            // A backslash is never part of a host or URI, and browsers read it as `/`, so it ends the literal like the end of the text would.
            b'<' | b'\\' => break,
            b']' if at + 1 == limit
                || matches!(bytes[at + 1], b'\t' | b'\r' | b'\n' | b' ' | b'(' | b'[') =>
            {
                break
            }
            b'&' => {
                let mut end = at + 1;
                while end < limit && bytes[end].is_ascii_alphabetic() {
                    end += 1;
                }
                if end > at + 1 && bytes.get(end) == Some(&b';') && end < limit {
                    at = end + 1;
                } else {
                    at += 1;
                    tail = at;
                }
            }
            byte if is_trailing(byte) => at += 1,
            byte if byte.is_ascii() => {
                if ascii_whitespace(byte) {
                    break;
                }
                at += 1;
                tail = at;
            }
            _ => {
                let character = source[at..limit].chars().next().unwrap();
                if classify(character) == Kind::Whitespace {
                    break;
                }
                at += character.len_utf8();
                tail = at;
            }
        }
    }
    (tail, at)
}

/// The start of the run of `word` bytes before the `:` at `colon`, scanning back at most one byte past the longest scheme so the work stays constant. A longer run is not a scheme.
fn scheme_start(bytes: &[u8], min: usize, colon: usize, word: fn(&u8) -> bool) -> usize {
    let mut start = colon;
    while start > min && colon - start <= SCHEME_SIZE_MAX && word(&bytes[start - 1]) {
        start -= 1;
    }
    start
}

fn ascii_whitespace(byte: u8) -> bool {
    // Unicode classification also treats vertical tab as whitespace, unlike u8::is_ascii_whitespace.
    matches!(byte, b'\t'..=b'\r' | b' ')
}

fn is_trailing(byte: u8) -> bool {
    matches!(
        byte,
        b'!' | b'"'
            | b'\''
            | b')'
            | b'*'
            | b','
            | b'.'
            | b':'
            | b';'
            | b'?'
            | b'_'
            | b'~'
            | b']'
            | b'&'
    )
}

/// Whether `byte` can be in the local part of an email literal.
fn local_part(byte: u8) -> bool {
    matches!(byte, b'+' | b'-' | b'.' | b'0'..=b'9' | b'A'..=b'Z' | b'_' | b'a'..=b'z')
}

/// Email expansion follows inline resolution and sees only a contiguous ordinary text run. Protocols already present in the source retain their spelling.
pub(super) fn email(source: &str, min: usize, at: usize, limit: usize) -> Option<Match> {
    let bytes = source.as_bytes();
    let mut start = at;
    while start > min && local_part(bytes[start - 1]) {
        start -= 1;
    }
    // Browsers read a backslash as `/`, so it blocks the local part as a path separator would.
    if start == at || (start > min && matches!(bytes[start - 1], b'/' | b'\\')) {
        return None;
    }
    let mut prefix = AutolinkPrefix::Mailto;
    let mut resource = false;
    if start > min && bytes[start - 1] == b':' {
        let protocol = scheme_start(bytes, min, start - 1, u8::is_ascii_alphanumeric);
        if let Some(SchemeKind::Address { resource: allowed }) =
            scheme(&bytes[protocol..start - 1]).map(|scheme| scheme.kind)
        {
            start = protocol;
            prefix = AutolinkPrefix::None;
            // GFM lets an `xmpp:` address end in a `/resource`.
            resource = allowed;
        }
    }
    let mut end = at + 1;
    let mut dot = false;
    while end < limit {
        match bytes[end] {
            b'-' | b'0'..=b'9' | b'A'..=b'Z' | b'_' | b'a'..=b'z' => {}
            b'/' if resource => {}
            b'.' if end + 1 < limit && bytes[end + 1].is_ascii_alphanumeric() => dot = true,
            _ => break,
        }
        end += 1;
    }
    if end == at + 1 || !dot || !matches!(bytes[end - 1], b'.' | b'A'..=b'Z' | b'a'..=b'z') {
        return None;
    }
    // The domain must be a host name, as for `www.` and URLs, though the local part and an `xmpp`
    // resource may have `_`.
    let host = &bytes[at + 1..end];
    let host = host.split(|&byte| byte == b'/').next().unwrap();
    if host.split(|&byte| byte == b'.').any(|label| {
        label.is_empty()
            || label.contains(&b'_')
            || label[0] == b'-'
            || label[label.len() - 1] == b'-'
    }) {
        return None;
    }
    Some(Match { start, end, prefix })
}

#[cfg(test)]
mod tests {
    use super::{email, web, Trail};
    use alloc::string::ToString;

    #[test]
    fn web_domains_paths_and_terminal_suffixes() {
        for (source, expected, prefix) in [
            ("https://example.com", "https://example.com", ""),
            (
                "(HTTPS://example.com/a(b)))",
                "HTTPS://example.com/a(b)",
                "",
            ),
            ("www.example.com&xxx;. ", "www.example.com", "http://"),
            ("www.example.com&xxx;b ", "www.example.com&xxx;b", "http://"),
            ("www.example.com&. ", "www.example.com&", "http://"),
            (
                "www.example.com/?q=a(business)\".",
                "www.example.com/?q=a(business)",
                "http://",
            ),
            (
                "www.example.com/?q=(business))+ok",
                "www.example.com/?q=(business))+ok",
                "http://",
            ),
            ("www.example.com](target)", "www.example.com", "http://"),
            ("www.example.com]x", "www.example.com]x", "http://"),
            ("www.example.com<after", "www.example.com", "http://"),
            ("www.點看.com/界\u{2003}after", "www.點看.com/界", "http://"),
            ("www.例.jp", "www.例.jp", "http://"),
            ("www.a-b.c-d", "www.a-b.c-d", "http://"),
            ("www.a.b._", "www.a.b", "http://"),
            ("http://x.org\0", "http://x.org\0", ""),
            // A backslash ends the literal like the end of the text, so trailing punctuation before it is excluded.
            ("www.a\\_b.com", "www.a", "http://"),
            ("http://a.b/x\\y", "http://a.b/x", ""),
            ("https://a.b/c.\\", "https://a.b/c", ""),
        ] {
            let marker = source
                .find(':')
                .unwrap_or_else(|| source.find('.').unwrap());
            let found = web(source, 0, marker, source.len(), &mut Trail::default())
                .expect("valid web fixture has a literal autolink");
            assert_eq!(&source[found.start..found.end], expected, "{source:?}");
            assert_eq!(found.prefix.as_str(), prefix, "{source:?}");
        }
    }

    #[test]
    fn web_prefixes_and_domain_rejections() {
        for source in [
            "www.",
            // `www.` needs a domain after it.
            "www. ",
            "www.$",
            "www._a",
            "awww.example.com",
            ":www.example.com",
            "ahttp://example.com",
            "http:/example.com",
            "gopher://example.com",
            "tel://example.com",
            "sshh://example.com",
            "http://",
            "http://---/x",
            "www.aaa_bbb.ccc",
            // Domains are host names: no `_`, no empty label, and no label that starts or ends
            // with `-`. An invalid domain is no link, rather than a shorter one.
            "www.-a..b.c",
            "www.aaa_bbb.ccc.ddd",
            "www._a.b.c",
            "www.-a.b",
            "www.a-.b",
            "www.a..b",
            "www.a./b",
            "http://_a.b.c",
            "https://a_b.com",
            "www.aaa.bbb_ccc",
            "http://a_b.c/d",
            "www.\\x",
            "http://\\x",
        ] {
            let marker = source
                .find(':')
                .unwrap_or_else(|| source.find('.').unwrap());
            assert!(
                web(source, 0, marker, source.len(), &mut Trail::default()).is_none(),
                "{:?}",
                source
            );
        }
        assert!(web("www.a", 1, 3, 5, &mut Trail::default()).is_none());
        assert!(web("http://a", 1, 4, 8, &mut Trail::default()).is_none());
        assert!(web("www.a", 0, 0, 5, &mut Trail::default()).is_none());
        let source = "xwww.a";
        assert_eq!(
            web(source, 1, 4, source.len(), &mut Trail::default())
                .unwrap()
                .start,
            1
        );
    }

    #[test]
    fn repeated_invalid_domains_share_terminal_lookahead() {
        let repeated = "http://a_b.c/".repeat(4096);
        let source = repeated.clone() + "http://good.org/a(b))).";
        let mut trail = Trail::default();
        for start in (0..repeated.len()).step_by("http://a_b.c/".len()) {
            assert!(web(&source, 0, start + 4, source.len(), &mut trail).is_none());
            // Later candidates reuse the first domain's lookahead rather than replacing it with another whole-suffix scan.
            assert_eq!(trail.start, 7);
            assert_eq!(trail.boundary, source.len());
        }
        let found = web(&source, 0, repeated.len() + 4, source.len(), &mut trail).unwrap();
        assert_eq!(&source[found.start..found.end], "http://good.org/a(b)");
        assert_eq!(trail.start, 7);
    }

    #[test]
    fn terminal_cache_respects_boundaries_limits_and_ascii_classes() {
        let source = "http://a_b.c http://good.org).";
        let mut trail = Trail::default();
        assert!(web(source, 0, 4, source.len(), &mut trail).is_none());
        let old_boundary = trail.boundary;
        let marker = source.rfind(':').unwrap();
        let found = web(source, 0, marker, source.len(), &mut trail).unwrap();
        assert_eq!(trail.start, marker + 3);
        assert!(trail.start > old_boundary);
        assert_eq!(&source[found.start..found.end], "http://good.org");
        // Shorter views cannot reuse a boundary from a previous fragment.
        assert_eq!(trail.suffix(source, 0, 4), (4, 4));
        assert_eq!(trail.limit, 4);
        // A later candidate consisting entirely of trailing punctuation starts at its own offset.
        let source = "abc.)";
        assert_eq!(trail.suffix(source, 0, source.len()), (3, 5));
        assert_eq!(trail.suffix(source, 4, source.len()), (4, 5));
        // Moving backward similarly refreshes the cache.
        assert_eq!(trail.suffix(source, 1, source.len()), (3, 5));
        assert_eq!(trail.suffix(source, 0, source.len()), (3, 5));
        for byte in 0..=0x7f {
            let mut source = "http://x/".to_string();
            source.push(char::from(byte));
            source.push('x');
            let found = web(&source, 0, 4, source.len(), &mut Trail::default()).unwrap();
            assert_eq!(
                found.end < source.len(),
                char::from(byte).is_whitespace() || byte == b'<' || byte == b'\\',
                "{byte}"
            );
        }
    }

    #[test]
    fn email_protocols_domain_rules_and_text_boundaries() {
        for (source, expected, prefix) in [
            ("a.b-c_d@a.b.", "a.b-c_d@a.b", "mailto:"),
            ("，example@example.com", "example@example.com", "mailto:"),
            ("mailto:a@example.org/path", "mailto:a@example.org", ""),
            ("XMPP:a@example.org/path.", "XMPP:a@example.org/path", ""),
            ("mmmmailto:a@example.org", "a@example.org", "mailto:"),
            ("a@b.c\\d", "a@b.c", "mailto:"),
            ("a_b@c-d.e", "a_b@c-d.e", "mailto:"),
            ("xmpp:a@b.c/x_y", "xmpp:a@b.c/x_y", ""),
        ] {
            let at = source.find('@').unwrap();
            let found = email(source, 0, at, source.len())
                .expect("valid email fixture has a literal autolink");
            assert_eq!(&source[found.start..found.end], expected, "{source:?}");
            assert_eq!(found.prefix.as_str(), prefix, "{source:?}");
        }
        for source in [
            "a@domain",
            "a@domain.",
            "a@a.b-",
            "a@a.b_",
            "a@a.b0",
            "@a.b",
            "/a@a.b",
            "\\a@a.b",
            "a@界.com",
            "a@a_b.c",
            "aaa@a.b_.c",
            "a@b_c.com",
            "a@-b.com",
            "a@b-.com",
            "a@.b.com",
        ] {
            let at = source.find('@').unwrap();
            assert!(email(source, 0, at, source.len()).is_none(), "{:?}", source);
        }
        assert!(email("aa@a.b", 2, 2, 6).is_none());
    }
}
