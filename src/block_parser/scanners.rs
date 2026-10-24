// Copyright 2015 Google Inc. All rights reserved.
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
// THE SOFTWARE.

//! Block syntax scanners adapted from pulldown-cmark 0.13.4, revision 38e4d08f14ec4bd9783270e9623db7681ebed968.
use core::convert::TryFrom;
use memchr::memchr2;
// Keep the line state together on the stack. Arm64 preparation benchmarks showed large layout-dependent stalls with the natural eight-byte alignment.
#[derive(Clone)]
#[repr(align(64))]
pub(super) struct LineStart<'a> {
    bytes: &'a [u8],
    ix: usize,
    tab_start: usize,
    spaces_remaining: usize,
    min_hrule_offset: usize,
}
impl LineStart<'_> {
    pub(crate) fn new(bytes: &[u8]) -> LineStart<'_> {
        LineStart {
            bytes,
            tab_start: 0,
            ix: 0,
            spaces_remaining: 0,
            min_hrule_offset: 0,
        }
    }

    pub(crate) fn scan_space(&mut self, n_space: usize) -> bool {
        self.scan_space_inner(n_space) == 0
    }

    pub(crate) fn scan_space_upto(&mut self, n_space: usize) -> usize {
        n_space - self.scan_space_inner(n_space)
    }

    fn scan_space_inner(&mut self, mut n_space: usize) -> usize {
        // Consume any common prefix between the number of spaces we
        // want and the number of unscanned tab-introduced spaces.
        let n_from_remaining = self.spaces_remaining.min(n_space);
        self.spaces_remaining -= n_from_remaining;
        n_space -= n_from_remaining;

        while n_space > 0 && self.ix < self.bytes.len() {
            match self.bytes[self.ix] {
                b' ' => {
                    self.ix += 1;
                    n_space -= 1;
                }
                b'\t' => {
                    let spaces = 4 - (self.ix - self.tab_start) % 4;
                    self.ix += 1;
                    self.tab_start = self.ix;
                    let n = spaces.min(n_space);
                    n_space -= n;

                    // Record the unscanned portion of the tab.
                    self.spaces_remaining = spaces - n;
                }
                _ => break,
            }
        }
        n_space
    }

    pub(crate) fn scan_all_space(&mut self) {
        self.spaces_remaining = 0;
        self.ix += self.bytes[self.ix..]
            .iter()
            .take_while(|&&b| b == b' ' || b == b'\t')
            .count();
    }

    pub(crate) fn is_at_eol(&self) -> bool {
        self.bytes
            .get(self.ix)
            .map_or(true, |&c| c == b'\r' || c == b'\n')
    }

    fn scan_ch(&mut self, c: u8) -> bool {
        if self.ix < self.bytes.len() && self.bytes[self.ix] == c {
            self.ix += 1;
            true
        } else {
            false
        }
    }

    pub(crate) fn scan_blockquote_marker(&mut self) -> bool {
        if self.spaces_remaining == 0 && self.scan_ch(b'>') {
            let _ = self.scan_space(1);
            true
        } else {
            false
        }
    }

    pub(crate) fn scan_list_marker_with_indent(
        &mut self,
        indent: usize,
        thematic: bool,
    ) -> Option<(u8, u64, usize)> {
        let save = self.clone();
        if self.ix < self.bytes.len() {
            let c = self.bytes[self.ix];
            if c == b'-' || c == b'+' || c == b'*' {
                if thematic && self.ix >= self.min_hrule_offset {
                    // there could be an hrule here
                    if let Err(min_offset) = scan_hrule(&self.bytes[self.ix..]) {
                        self.min_hrule_offset = self.ix + min_offset;
                    } else {
                        *self = save;
                        return None;
                    }
                }
                self.ix += 1;
                if self.scan_space(1) || self.is_at_eol() {
                    return Some(self.finish_list_marker(c, 0, indent + 2));
                }
            } else if c.is_ascii_digit() {
                let start_ix = self.ix;
                let mut ix = self.ix + 1;
                let mut val = u64::from(c - b'0');
                while ix < self.bytes.len() && ix - start_ix < 10 {
                    let c = self.bytes[ix];
                    ix += 1;
                    if c.is_ascii_digit() {
                        val = val * 10 + u64::from(c - b'0');
                    } else if c == b')' || c == b'.' {
                        self.ix = ix;
                        if self.scan_space(1) || self.is_at_eol() {
                            return Some(self.finish_list_marker(
                                c,
                                val,
                                indent + 1 + ix - start_ix,
                            ));
                        }
                        break;
                    } else {
                        break;
                    }
                }
            }
        }
        *self = save;
        None
    }

    fn finish_list_marker(&mut self, c: u8, start: u64, mut indent: usize) -> (u8, u64, usize) {
        let save = self.clone();

        // skip the rest of the line if it's blank
        if scan_blank_line(&self.bytes[self.ix..]).is_some() {
            return (c, start, indent);
        }

        let post_indent = self.scan_space_upto(4);
        if post_indent < 4 {
            indent += post_indent;
        } else {
            *self = save;
        }
        (c, start, indent)
    }

    pub(crate) fn advance(&mut self, bytes: usize) {
        self.ix += bytes;
    }

    pub(crate) fn bytes_scanned(&self) -> usize {
        self.ix
    }

    pub(crate) fn remaining_space(&self) -> usize {
        self.spaces_remaining
    }
}
pub(crate) fn is_ascii_whitespace(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\r' | b'\n')
}

pub(crate) fn is_ascii_whitespace_no_nl(c: u8) -> bool {
    matches!(c, b' ' | b'\t')
}

pub(crate) fn scan_while<F>(data: &[u8], mut f: F) -> usize
where
    F: FnMut(u8) -> bool,
{
    data.iter().take_while(|&&c| f(c)).count()
}

pub(crate) fn scan_ch_repeat(data: &[u8], c: u8) -> usize {
    scan_while(data, |x| x == c)
}

pub(crate) fn scan_whitespace_no_nl(data: &[u8]) -> usize {
    scan_while(data, is_ascii_whitespace_no_nl)
}

pub(crate) fn scan_eol(bytes: &[u8]) -> Option<usize> {
    match *bytes {
        [] => Some(0),
        [b'\r', b'\n', ..] => Some(2),
        [b'\n' | b'\r', ..] => Some(1),
        _ => None,
    }
}

pub(crate) fn scan_blank_line(bytes: &[u8]) -> Option<usize> {
    let i = scan_whitespace_no_nl(bytes);
    scan_eol(&bytes[i..]).map(|n| i + n)
}

pub(crate) fn scan_nextline(bytes: &[u8]) -> usize {
    memchr2(b'\r', b'\n', bytes).map_or(bytes.len(), |index| {
        index + scan_eol(&bytes[index..]).unwrap()
    })
}

pub(crate) fn scan_closing_code_fence(
    bytes: &[u8],
    fence_char: u8,
    n_fence_char: usize,
) -> Option<usize> {
    if bytes.is_empty() {
        return None;
    }
    let mut i = 0;
    let num_fence_chars_found = scan_ch_repeat(&bytes[i..], fence_char);
    if num_fence_chars_found < n_fence_char {
        return None;
    }
    i += num_fence_chars_found;
    let num_trailing_spaces = scan_while(&bytes[i..], |byte| matches!(byte, b' ' | b'\t'));
    i += num_trailing_spaces;
    scan_eol(&bytes[i..]).map(|_| i)
}

pub(crate) fn scan_hrule(bytes: &[u8]) -> Result<usize, usize> {
    if bytes.len() < 3 {
        return Err(0);
    }
    let c = bytes[0];
    if !(c == b'*' || c == b'-' || c == b'_') {
        return Err(0);
    }
    let mut n = 0;
    let mut i = 0;

    while i < bytes.len() {
        match bytes[i] {
            b'\n' | b'\r' => {
                i += scan_eol(&bytes[i..]).unwrap_or(0);
                break;
            }
            c2 if c2 == c => {
                n += 1;
            }
            b' ' | b'\t' => (),
            _ => return Err(i),
        }
        i += 1;
    }
    if n >= 3 {
        Ok(i)
    } else {
        Err(i)
    }
}

pub(crate) fn scan_atx_heading(data: &[u8]) -> Option<u8> {
    let level = scan_ch_repeat(data, b'#');
    if data.get(level).copied().map_or(true, is_ascii_whitespace) {
        (1..=6)
            .contains(&level)
            .then(|| u8::try_from(level).expect("heading levels are at most six"))
    } else {
        None
    }
}

pub(crate) fn scan_setext_heading(data: &[u8]) -> Option<(usize, u8)> {
    let c = *data.first()?;
    let level = if c == b'=' {
        1
    } else if c == b'-' {
        2
    } else {
        return None;
    };
    let mut i = 1 + scan_ch_repeat(&data[1..], c);
    i += scan_blank_line(&data[i..])?;
    Some((i, level))
}

// Callers dispatch on the opening byte: data is nonempty and starts with a backtick or tilde.
pub(crate) fn scan_code_fence(data: &[u8]) -> Option<(usize, u8)> {
    let c = data[0];
    let i = 1 + scan_ch_repeat(&data[1..], c);
    if i >= 3 {
        if c == b'`' {
            let suffix = &data[i..];
            let next_line = i + scan_nextline(suffix);
            if suffix[..(next_line - i)].contains(&b'`') {
                return None;
            }
        }
        Some((i, c))
    } else {
        None
    }
}
