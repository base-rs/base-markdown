// Released under the MIT License.
// Copyright, 2022-2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

//! Deal with positions in a file.
//!
//! * Convert between byte indices and unist points.
use crate::unist::Point;
use alloc::{vec, vec::Vec};

#[derive(Debug)]
pub struct Location {
    /// List, where each index is a line number (0-based), and each value is
    /// the byte index *after* where the line ends.
    indices: Vec<usize>,
}

impl Location {
    /// Get an index for the given `bytes`.
    ///
    /// Port of <https://github.com/vfile/vfile-location/blob/main/index.js>
    #[must_use]
    pub fn new(bytes: &[u8]) -> Self {
        let mut index = 0;
        let mut location_index = Self { indices: vec![] };

        while index < bytes.len() {
            if bytes[index] == b'\r' {
                if index + 1 < bytes.len() && bytes[index + 1] == b'\n' {
                    location_index.indices.push(index + 2);
                    index += 1;
                } else {
                    location_index.indices.push(index + 1);
                }
            } else if bytes[index] == b'\n' {
                location_index.indices.push(index + 1);
            }

            index += 1;
        }

        location_index.indices.push(index + 1);
        location_index
    }

    /// Get the line and column-based `point` for `offset` in the bound indices.
    ///
    /// Returns `None` when given out of bounds input.
    ///
    /// Port of <https://github.com/vfile/vfile-location/blob/main/index.js>
    #[must_use]
    pub fn to_point(&self, offset: usize) -> Option<Point> {
        let mut index = 0;
        let end = *self
            .indices
            .last()
            .expect("location index includes an end offset");

        if offset < end {
            while index < self.indices.len() {
                if self.indices[index] > offset {
                    break;
                }

                index += 1;
            }

            let previous = if index > 0 {
                self.indices[index - 1]
            } else {
                0
            };
            return Some(Point::new(index + 1, offset + 1 - previous, offset));
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_location_lf() {
        let location = Location::new("ab\nc".as_bytes());
        assert_eq!(
            location.to_point(0), // `a`
            Some(Point::new(1, 1, 0)),
            "should support some points (1)"
        );
        assert_eq!(
            location.to_point(1), // `b`
            Some(Point::new(1, 2, 1)),
            "should support some points (2)"
        );
        assert_eq!(
            location.to_point(2), // `\n`
            Some(Point::new(1, 3, 2)),
            "should support some points (3)"
        );
        assert_eq!(
            location.to_point(3), // `c`
            Some(Point::new(2, 1, 3)),
            "should support some points (4)"
        );
        assert_eq!(
            location.to_point(4), // EOF
            // Still gets a point, so that we can represent positions of things
            // that end at the last character, the `c` end at `2:2`.
            Some(Point::new(2, 2, 4)),
            "should support some points (5)"
        );
        assert_eq!(
            location.to_point(5), // Out of bounds
            None,
            "should support some points (6)"
        );
    }

    #[test]
    fn test_location_cr() {
        let location = Location::new("a\rb".as_bytes());
        assert_eq!(
            location.to_point(0), // `a`
            Some(Point::new(1, 1, 0)),
            "should support some points (1)"
        );
        assert_eq!(
            location.to_point(1), // `\r`
            Some(Point::new(1, 2, 1)),
            "should support some points (2)"
        );
        assert_eq!(
            location.to_point(2), // `b`
            Some(Point::new(2, 1, 2)),
            "should support some points (3)"
        );
    }

    #[test]
    fn returns_none_for_offsets_past_the_document() {
        assert_eq!(Location::new(b"abc").to_point(4), None);
    }

    #[test]
    fn test_location_cr_lf() {
        let location = Location::new("a\r\nb".as_bytes());
        assert_eq!(
            location.to_point(0), // `a`
            Some(Point::new(1, 1, 0)),
            "should support some points (1)"
        );
        assert_eq!(
            location.to_point(1), // `\r`
            Some(Point::new(1, 2, 1)),
            "should support some points (2)"
        );
        assert_eq!(
            location.to_point(2), // `\n`
            Some(Point::new(1, 3, 2)),
            "should support some points (3)"
        );
        assert_eq!(
            location.to_point(3), // `b`
            Some(Point::new(2, 1, 3)),
            "should support some points (4)"
        );
    }
    #[test]
    fn test_empty() {
        let location = Location::new("".as_bytes());
        assert_eq!(location.to_point(0), Some(Point::new(1, 1, 0)), "to_point");
    }
}
