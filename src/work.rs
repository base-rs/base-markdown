// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Work, which tests count to check that it grows linearly with the input.
//!
//! Wall-clock time depends on how loaded the machine is, so the linearity tests count work instead, of three kinds, each of which must grow linearly on its own: each read of a document node through its [`NodeId`](crate::NodeId), which [`read`] counts; each byte allocated, which the allocator the crate's tests install counts; and each step of a loop over the input that reads no node and allocates nothing, such as a scan of source or output, on a path a linearity test checks, which [`step`] counts. Work that goes quadratic repeats one of these, so its count grows with the square of the input, deterministically. Outside tests, [`read`] and [`step`] do nothing.

/// Count a read of a document node.
#[inline]
pub(crate) fn read() {
    #[cfg(test)]
    tests::add(tests::Kind::Reads, 1);
}

/// Count `steps` steps of a loop over the input.
#[inline]
pub(crate) fn step(steps: usize) {
    #[cfg(test)]
    tests::add(tests::Kind::Steps, steps);
    #[cfg(not(test))]
    let _ = steps;
}

#[cfg(test)]
pub(crate) mod tests;
