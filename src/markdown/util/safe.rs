// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

//! JS equivalent <https://github.com/syntax-tree/mdast-util-to-markdown/blob/fd6a508/lib/util/safe.js>
use alloc::string::String;
use core::fmt::Write as _;

/// How a character that would otherwise start or end syntax is written so that it reads back as
/// text, as the document's options allow: with a backslash, which `Constructs::character_escape`
/// reads before ASCII punctuation, or as a character reference, which
/// `Constructs::character_reference` decodes. Where they turn both off, nothing does, and the
/// writer writes the character as it is and returns `unwritable-node` for it (contract 3.3).
#[derive(Clone, Copy)]
pub struct Literal {
    pub backslash: bool,
    pub reference: bool,
}

impl Literal {
    /// Write `character` so that it reads back as text, preferring a backslash where `backslash`
    /// allows one. Returns `false` where the options allow neither, after writing the character
    /// as it is.
    pub fn write(self, output: &mut String, character: char, backslash: bool) -> bool {
        if backslash && self.backslash && character.is_ascii_punctuation() {
            output.push('\\');
            output.push(character);
        } else if self.reference {
            write!(output, "&#x{:X};", u32::from(character)).expect("String writing is infallible");
        } else {
            output.push(character);
            return false;
        }
        true
    }

    /// Escape the character at `position` of `output` in place, as `write` does.
    pub fn escape_at(self, output: &mut String, position: usize) -> bool {
        let character = output[position..]
            .chars()
            .next()
            .expect("an escaped position is a character boundary");
        let mut written = String::new();
        let ok = self.write(&mut written, character, true);
        output.replace_range(position..position + character.len_utf8(), &written);
        ok
    }

    /// Whether `text` ends with an odd number of backslashes, which escape the next character
    /// where backslash escapes are read.
    pub fn escapes_next(self, text: &str) -> bool {
        self.backslash && text.bytes().rev().take_while(|&byte| byte == b'\\').count() % 2 == 1
    }
}

pub struct EscapeInfos {
    pub after: bool,
    pub before: bool,
    /// Apply only if the escaped `<` would otherwise start an email autolink in the written text.
    pub email: bool,
}

/// JS: <https://github.com/syntax-tree/mdast-util-to-markdown/blob/fd6a508/lib/util/safe.js#L148>.
///
/// Without `escapes`, where no backslash escape is read, a backslash is always literal, so none
/// is doubled.
pub fn write_backslashes(value: &str, after: &str, escapes: bool, results: &mut String) {
    if !escapes {
        results.push_str(value);
        return;
    }
    let mut start = 0;
    let bytes = value.as_bytes();
    let mut at = 0;
    while let Some(offset) = memchr::memchr(b'\\', &bytes[at..]) {
        let position = at + offset;
        let next = bytes.get(position + 1).or_else(|| after.as_bytes().first());
        if next.is_some_and(u8::is_ascii_punctuation) {
            results.push_str(&value[start..position]);
            results.push('\\');
            start = position;
        }
        // Upstream matches with a lookahead, so the next backslash is checked as well.
        at = position + 1;
    }
    results.push_str(&value[start..]);
}

#[cfg(test)]
pub fn escape_backslashes(value: &str, after: &str) -> String {
    let mut output = String::new();
    write_backslashes(value, after, true, &mut output);
    output
}
