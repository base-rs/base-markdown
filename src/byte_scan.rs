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
                vandq_u8, vdupq_n_u8, vld1q_u8, vmaxvq_u8, vqtbl1q_u8, vshrq_n_u8, vtstq_u8,
            };
            // NEON is enabled by the target. Every vector load stays within its slice; the lookup tables each contain exactly sixteen bytes. UTF-8 continuation bytes map to zero in the high-nibble table.
            unsafe {
                let low = vld1q_u8(self.low.as_ptr());
                let high = vld1q_u8([1, 2, 4, 8, 16, 32, 64, 128, 0, 0, 0, 0, 0, 0, 0, 0].as_ptr());
                while bytes.len() - offset >= 16 {
                    let input = vld1q_u8(bytes.as_ptr().add(offset));
                    let low_bits = vqtbl1q_u8(low, vandq_u8(input, vdupq_n_u8(15)));
                    let high_bits = vqtbl1q_u8(high, vshrq_n_u8::<4>(input));
                    if vmaxvq_u8(vtstq_u8(low_bits, high_bits)) != 0 {
                        break;
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
    use alloc::vec;

    #[test]
    fn candidates_match_scalar_at_every_alignment_and_boundary() {
        for markers in [b"\0&\"<>".as_slice(), b"\0<\\\r\n&`*_[]!", b"", b"\x7f"] {
            let set = AsciiSet::new(markers);
            for alignment in 0..32 {
                for length in 0..65 {
                    let mut bytes = vec![b'a'; alignment + length];
                    assert_eq!(set.find(&bytes[alignment..]), length);
                    for index in 0..length {
                        for byte in 0..=255 {
                            bytes[alignment + index] = byte;
                            assert_eq!(
                                set.find(&bytes[alignment..]),
                                if markers.contains(&byte) {
                                    index
                                } else {
                                    length
                                }
                            );
                        }
                        bytes[alignment + index] = b'a';
                    }
                }
            }
        }
    }
}
