// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

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

// Copyright 2026 Samuel Williams.

//! Candidate-byte scanning. The two-nibble lookup technique follows pulldown-cmark's SIMD scanner (revision 38e4d08f14ec4bd9783270e9623db7681ebed968); Markdown recognition stays in the parser.
pub(crate) struct AsciiSet {
    members: [bool; 256],
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    low: [u8; 16],
}
impl AsciiSet {
    pub(crate) const fn new(bytes: &[u8]) -> Self {
        let mut members = [false; 256];
        #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
        let mut low = [0; 16];
        let mut index = 0;
        while index < bytes.len() {
            let byte = bytes[index];
            assert!(byte < 128);
            members[byte as usize] = true;
            #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
            {
                low[(byte & 15) as usize] |= 1 << (byte >> 4);
            }
            index += 1;
        }
        Self {
            members,
            #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
            low,
        }
    }
    #[inline]
    pub(crate) fn find(&self, bytes: &[u8]) -> usize {
        #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
        let offset = 0;
        #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
        let mut offset = 0;
        #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
        {
            use core::arch::aarch64::{
                vandq_u8, vbslq_u8, vdupq_n_u8, vld1q_u8, vmaxvq_u8, vminvq_u8, vqtbl1q_u8,
                vshrq_n_u8, vtstq_u8,
            };
            // NEON is enabled by the target. Every vector load stays within its slice; the lookup tables each contain exactly sixteen bytes. UTF-8 continuation bytes map to zero in the high-nibble table.
            unsafe {
                let low = vld1q_u8(self.low.as_ptr());
                let high = vld1q_u8([1, 2, 4, 8, 16, 32, 64, 128, 0, 0, 0, 0, 0, 0, 0, 0].as_ptr());
                let indices =
                    vld1q_u8([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15].as_ptr());
                while bytes.len() - offset >= 16 {
                    let input = vld1q_u8(bytes.as_ptr().add(offset));
                    let low_bits = vqtbl1q_u8(low, vandq_u8(input, vdupq_n_u8(15)));
                    let high_bits = vqtbl1q_u8(high, vshrq_n_u8::<4>(input));
                    let matches = vtstq_u8(low_bits, high_bits);
                    if vmaxvq_u8(matches) != 0 {
                        // A matched lane retains its byte index; other lanes become 255. Compute the minimum only for a matching chunk so safe runs keep their detection-only loop.
                        let first = vminvq_u8(vbslq_u8(matches, indices, vdupq_n_u8(u8::MAX)));
                        return offset + usize::from(first);
                    }
                    offset += 16;
                }
            }
        }
        offset
            + bytes[offset..]
                .iter()
                .position(|byte| self.members[usize::from(*byte)])
                .unwrap_or(bytes.len() - offset)
    }
}

#[cfg(test)]
mod tests {
    use super::AsciiSet;
    use alloc::{vec, vec::Vec};

    /// Check `find` on `length` bytes of `a` with each of `bytes` at each index, starting the
    /// slice at each of `alignments`.
    fn check(set: &AsciiSet, markers: &[u8], alignments: &[usize], length: usize, bytes: &[u8]) {
        for &alignment in alignments {
            let mut buffer = vec![b'a'; alignment + length];
            assert_eq!(set.find(&buffer[alignment..]), length);
            for index in 0..length {
                for &byte in bytes {
                    buffer[alignment + index] = byte;
                    let expected = if markers.contains(&byte) {
                        index
                    } else {
                        length
                    };
                    assert_eq!(
                        set.find(&buffer[alignment..]),
                        expected,
                        "{byte:#x} at {index}"
                    );
                }
                buffer[alignment + index] = b'a';
            }
        }
    }

    #[test]
    fn candidates_match_scalar_at_every_alignment_and_boundary() {
        // Loads are unaligned and chunks count from the start of the slice, so a few alignments
        // stand in for all of them.
        let alignments = [0, 1, 7, 15, 16, 31];
        let every: Vec<u8> = (0..=255).collect();
        for markers in [b"\0&\"<>".as_slice(), b"\0<\\\r\n&`*_[]!", b"", b"\x7f"] {
            let set = AsciiSet::new(markers);
            // Every byte in every lane of one to four chunks, at every tail position with and
            // without a chunk before it, and first in the tail after two and four chunks.
            for length in [15, 16, 17, 31, 32, 33, 64, 65] {
                check(&set, markers, &alignments, length, &every);
            }
            // Every position at every length, with each marker, each byte that shares a nibble
            // with one, and UTF-8 continuation and lead bytes, at two alignments.
            let probes: Vec<u8> = (0..=255)
                .filter(|&byte: &u8| {
                    matches!(byte, 0x80 | 0xbf | 0xc3 | 0xff)
                        || markers
                            .iter()
                            .any(|&marker| byte & 15 == marker & 15 || byte >> 4 == marker >> 4)
                })
                .collect();
            for length in 0..65 {
                check(&set, markers, &alignments[..2], length, &probes);
            }
        }
    }

    #[test]
    fn multiple_candidates_return_the_first_lane_across_vector_and_tail_boundaries() {
        let set = AsciiSet::new(b"\0&\"<>");
        for alignment in 0..16 {
            for length in [15, 16, 17, 31, 32, 33, 64, 65] {
                let mut bytes = vec![0x80; alignment + length];
                for first in 0..length {
                    bytes[alignment + first] = b'&';
                    for second in first + 1..length {
                        bytes[alignment + second] = b'<';
                        assert_eq!(set.find(&bytes[alignment..]), first);
                        bytes[alignment + second] = 0x80;
                    }
                    bytes[alignment + first] = 0x80;
                }
            }
        }
    }
}
