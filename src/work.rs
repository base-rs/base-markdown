// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Work, which tests count to check that it grows linearly with the input.
//!
//! Wall-clock time depends on how loaded the machine is, so the linearity tests count work instead, of three kinds, each of which must grow linearly on its own: each read of a document node through its [`NodeId`](crate::NodeId), which [`read`] counts; each byte allocated, which the allocator the crate's tests install counts; and the steps that [`step!`] counts, for work that reads no node and allocates nothing: each byte that a scan of source or output reads, as [`searched!`] and [`searched_back!`] count for a search, each byte or element that a shift moves, and each element that a sort or filter goes through. A [`Deque`] counts the elements that each of its operations visits or moves. Outside tests, these are nothing, and evaluate no count, and a [`Deque`] is a `VecDeque`.
//!
//! The counts see only the work that is counted. Quadratic work fails a linearity test, every time, when it repeats work of one of these kinds, but a path that repeats a scan, shift or sort that the standard library does, such as `str::rfind`, `memchr`, `String::insert`, `copy_within` or `slice::sort`, without counting it, passes. So count such work where it is added on a path that a linearity test checks. Work that no count reaches, such as a scan of the serializer's output, a `String`, added without one, is checked by an ignored test that times it, as `writing_many_labels_on_a_line_takes_linear_time` does, or by a benchmark.

/// Count a read of a document node.
#[inline]
pub(crate) fn read() {
    #[cfg(test)]
    tests::add(tests::Kind::Reads, 1);
}

/// Count `$steps` steps of work that reads no node and allocates nothing. Outside tests, this is nothing: `$steps` is not evaluated.
macro_rules! step {
    ($steps:expr $(,)?) => {{
        #[cfg(test)]
        $crate::work::tests::add($crate::work::tests::Kind::Steps, $steps);
    }};
}
pub(crate) use step;

/// `$found`, what a forward search of `$length` bytes found, counting the bytes it read: up to and including the match, or all of them. Outside tests, this is `$found`, and `$length` is not evaluated.
macro_rules! searched {
    ($found:expr, $length:expr $(,)?) => {{
        let found: Option<usize> = $found;
        #[cfg(test)]
        $crate::work::step!(found.map_or($length, |at| at + 1));
        found
    }};
}
pub(crate) use searched;

/// `$found`, what a backward search of `$length` bytes found, counting the bytes it read: from the match to the end, or all of them. Outside tests, this is `$found`, and `$length` is not evaluated.
macro_rules! searched_back {
    ($found:expr, $length:expr $(,)?) => {{
        let found: Option<usize> = $found;
        #[cfg(test)]
        $crate::work::step!({
            let length: usize = $length;
            found.map_or(length, |at| length - at)
        });
        found
    }};
}
pub(crate) use searched_back;

/// A double-ended queue, which in tests counts as steps the elements each of its operations visits or moves, so that shifting or scanning it on a path a linearity test checks is counted. In tests, it has only the operations the crate uses: one added there must count the elements it can visit or move.
#[cfg(not(test))]
pub(crate) type Deque<T> = alloc::collections::VecDeque<T>;
#[cfg(test)]
pub(crate) use tests::Deque;

#[cfg(test)]
pub(crate) mod tests;
