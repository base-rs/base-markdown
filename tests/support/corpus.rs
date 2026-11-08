// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Deterministic source corpora shared by the round-trip fuzz and the contract tests.

/// A xorshift generator: deterministic, portable and dependency free.
pub struct Random(pub u64);

impl Random {
    pub fn below(&mut self, bound: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % bound as u64) as usize
    }

    pub fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[self.below(items.len())]
    }

    pub fn concatenate(&mut self, items: &[&str], most: usize) -> String {
        let mut value = String::new();
        for _ in 0..1 + self.below(most) {
            value.push_str(self.pick(items));
        }
        value
    }
}

/// Prose-like pieces: words, markers, escapes, references, links, code, block starts, tables and literal autolinks.
pub const REALISTIC: &[&str] = &[
    "a",
    "foo",
    "bar",
    " ",
    " ",
    " ",
    "x",
    "5 min",
    "~",
    "~",
    "~~",
    "~5",
    "~/path",
    "\\~",
    "\\\\",
    "*",
    "_",
    "**",
    "`c~d`",
    "`",
    "[t](u)",
    "[t~](u~ \"t~\")",
    "[",
    "](u)",
    "\n",
    "\n- ",
    "\n> ",
    "\n| a | b |\n| - | - |\n| ",
    " | ",
    "&#x20;",
    "&#126;",
    "&amp;",
    "# ",
    "1. ",
    ".",
    ",",
    ":",
    "(",
    ")",
    "!",
    "$",
    "~~del~~",
    "~x~",
    "\\",
    "  \n",
    "<b>",
    "www.a.b",
    "x@y.z",
    "é",
    "\u{a0}",
];

/// Characters of attention, strikethrough, escapes, punctuation and line endings, one at a time.
pub const SOUP: &[&str] = &["*", "_", "~", "a", ".", " ", "\n", "\\"];

/// Markers next to encoded whitespace, encoded letters and escaped markers.
pub const EDGE: &[&str] = &[
    "*", "**", "_", "__", "~", "~", "~~", "\\~", "&#x20;", "&#xA;", "&#x9;", "&#x61;", "&#x62;",
    "a", "b", " ", "(", ".", "\\", "\u{a0}", "|", "x ~a ",
];

/// Block starts for the edge corpus, so edges also meet headings, table cells, items and quotes.
pub const EDGE_BLOCK: &[&str] = &["# ", "| ", "- ", "> "];

/// Tilde runs of every length among letters, digits, attention, escapes, code and links.
pub const STRIKE: &[&str] = &[
    "~", "~", "~~", "~~", "~~~", "a", "b", "5", " ", " ", "*", "_", "(", ")", ".", "\\~", "del",
    "\n", "&#x20;", "[t](u)", "`~`",
];

/// Block starts, list and task markers, definitions and footnotes, which make copies, and blank lines, for the block shapes that normalizing rewrites.
pub const BLOCKY: &[&str] = &[
    "- ",
    "* ",
    "  ",
    "    ",
    "> ",
    "1. ",
    "2) ",
    "[ ] ",
    "[x] ",
    "[^a]",
    "[^b]",
    "[^a]: ",
    "[^b]: ",
    "[a]: /u\n",
    "[b]: /v\n",
    "[a]",
    "[b][a]",
    "\n",
    "\n",
    "\n\n",
    "# ",
    "Setext\n---\n",
    "```\n",
    "~~~",
    "<div>",
    "<b>",
    "| a |\n| - |\n",
    "a",
    "b c",
    "*b*",
    "\t",
    "***",
    "<!-- x -->",
    "  \n",
    "\\",
];

/// Prose-like Markdown of one to twelve realistic pieces.
pub fn realistic(random: &mut Random) -> String {
    random.concatenate(REALISTIC, 12)
}

/// Attention, escapes and line endings, one character at a time.
pub fn soup(random: &mut Random) -> String {
    random.concatenate(SOUP, 12)
}

/// Markers next to encoded whitespace and letters, sometimes after a block start.
pub fn edge(random: &mut Random) -> String {
    let mut source = String::new();
    if random.below(4) == 0 {
        source.push_str(random.pick(EDGE_BLOCK));
    }
    source.push_str(&random.concatenate(EDGE, 12));
    if source.starts_with("| ") {
        source.push_str(" |\n| - |");
    }
    source
}

/// Tilde runs among letters, attention, escapes, code and links.
pub fn strike(random: &mut Random) -> String {
    random.concatenate(STRIKE, 10)
}

/// Block structure of one to fourteen pieces.
pub fn blocky(random: &mut Random) -> String {
    random.concatenate(BLOCKY, 14)
}
