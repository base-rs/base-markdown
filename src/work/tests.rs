// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Counting work, and asserting that it grows linearly with the input.

extern crate std;

use core::alloc::{GlobalAlloc, Layout};
use core::cell::Cell;
use core::fmt::{self, Debug};
use std::alloc::System;

/// A kind of work, counted on its own, so that a lot of one kind does not hide another kind growing faster.
#[derive(Clone, Copy)]
pub(crate) enum Kind {
    /// Reads of document nodes.
    Reads,
    /// Steps of loops over the input.
    Steps,
    /// Bytes allocated.
    Bytes,
}

std::thread_local! {
    /// The work this thread has counted, so that tests running in parallel count only their own.
    static COUNTS: [Cell<u64>; 3] = const { [Cell::new(0), Cell::new(0), Cell::new(0)] };
}

pub(crate) fn add(kind: Kind, count: usize) {
    // The allocator counts too, which can run as a thread exits, once the counts are gone.
    let _ = COUNTS.try_with(|counts| {
        let counter = &counts[kind as usize];
        counter.set(counter.get() + count as u64);
    });
}

fn now() -> [u64; 3] {
    COUNTS.with(|counts| counts.each_ref().map(Cell::get))
}

/// The system allocator, counting each byte allocated, so that copying grows the count as much as it does the work: copying all that was built so far at each step allocates a number of bytes that grows with the square of the input, while a buffer that grows by doubling allocates at most about twice its final size.
struct Counting;

// SAFETY: each call is passed on to the system allocator unchanged.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        add(Kind::Bytes, layout.size());
        System.alloc(layout)
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        add(Kind::Bytes, layout.size());
        System.alloc_zeroed(layout)
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        add(Kind::Bytes, size);
        System.realloc(pointer, layout, size)
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        System.dealloc(pointer, layout);
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// The work of each kind that something counted.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct Counts([u64; 3]);

impl Debug for Counts {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [reads, steps, bytes] = self.0;
        write!(formatter, "{reads} reads, {steps} steps, {bytes} bytes")
    }
}

/// The work `run` counts.
pub(crate) fn counted(run: impl FnOnce()) -> Counts {
    let before = now();
    run();
    let after = now();
    Counts([0, 1, 2].map(|kind| after[kind] - before[kind]))
}

/// How many times larger the large input is than the small one.
pub(crate) const SCALE: usize = 8;

/// Whether `large`, the work at `SCALE` times the input of `small`, grew no more than twice as much as `growth`, the work of something that grew linearly from `growth.0` to `growth.1`: `(1, SCALE)` for work that should be linear, or a control's work at each size.
///
/// Linear work counts at most about `SCALE` times as much, or less where part of it does not grow with the input, and work with a logarithmic factor, such as a search of what was built so far at each step, about 1.4 times that at the sizes the tests use. Work that grows with the square of the input counts up to `SCALE * SCALE` times as much, and more than this bound once its quadratic part is a few percent of the work at the small size.
fn within(small: u64, large: u64, growth: (u64, u64)) -> bool {
    u128::from(large) * u128::from(growth.0) <= 2 * u128::from(growth.1) * u128::from(small)
}

/// Assert that `SCALE` times the input, `size` against `SCALE * size`, counts no more than twice `SCALE` times the work of each kind.
///
/// `counts` builds an input of the given size, runs the work and returns the work that the part being checked counted, with [`counted`]. The counts are written to standard error, which `--nocapture` shows.
pub(crate) fn assert_linear(
    label: impl Debug,
    size: usize,
    mut counts: impl FnMut(usize) -> Counts,
) {
    let small = counts(size);
    let large = counts(SCALE * size);
    std::eprintln!(
        "linear {:?}: {} -> {}: {:?} -> {:?}",
        label,
        size,
        SCALE * size,
        small,
        large
    );
    assert!(small != Counts([0; 3]), "{:?}: counted no work", label);
    for kind in 0..3 {
        assert!(
            within(small.0[kind], large.0[kind], (1, SCALE as u64)),
            "{:?}: {}x the input counted {:?} against {:?}",
            label,
            SCALE,
            large,
            small
        );
    }
}

/// Assert that `subject` grows from `size` to `SCALE * size` no more than twice as much as `control` does, in each kind of work.
///
/// The control does the same work on inputs that keep what is checked small, such as many short documents in place of one long one, so the ratio of the two growths shows only the work that depends on what is checked. A kind of work that the control does less than once for each unit of its input, such as none, must grow linearly.
pub(crate) fn assert_against_control(
    label: impl Debug,
    size: usize,
    mut subject: impl FnMut(usize) -> Counts,
    mut control: impl FnMut(usize) -> Counts,
) {
    let [small, large, control_small, control_large] = [
        subject(size),
        subject(SCALE * size),
        control(size),
        control(SCALE * size),
    ];
    std::eprintln!(
        "against control {:?}: {} -> {}: {:?} -> {:?}, control {:?} -> {:?}",
        label,
        size,
        SCALE * size,
        small,
        large,
        control_small,
        control_large
    );
    assert!(small != Counts([0; 3]), "{:?}: counted no work", label);
    for kind in 0..3 {
        // A control that does a kind of work less than once for each unit of its input is no
        // control for that kind, which is held to linear growth instead.
        let growth = if control_small.0[kind] < size as u64 {
            (1, SCALE as u64)
        } else {
            (control_small.0[kind], control_large.0[kind])
        };
        assert!(
            within(small.0[kind], large.0[kind], growth),
            "{:?}: grew from {:?} to {:?}, against the control's {:?} to {:?}",
            label,
            small,
            large,
            control_small,
            control_large
        );
    }
}

#[test]
fn counts_work_on_this_thread() {
    let counts = counted(|| {
        super::read();
        super::step(3);
        drop(std::vec![0_u8; 5]);
    });
    assert_eq!(counts, Counts([1, 3, 5]));
    assert_eq!(std::format!("{counts:?}"), "1 reads, 3 steps, 5 bytes");
    // Spawning the thread allocates on this one, but counts none of the thread's work.
    let counts = counted(|| {
        std::thread::spawn(|| super::step(3)).join().unwrap();
    });
    assert_eq!(counts.0[Kind::Steps as usize], 0);
}

#[test]
fn linear_work_passes() {
    assert_linear("linear", 16, |size| counted(|| super::step(size + 4)));
    assert_against_control(
        "linear",
        16,
        |size| {
            counted(|| {
                super::step(2 * size);
                super::read();
            })
        },
        |size| counted(|| super::step(size)),
    );
}

#[test]
#[should_panic(expected = "8x the input counted")]
fn quadratic_work_fails() {
    assert_linear("quadratic", 16, |size| {
        counted(|| super::step(size * size / 8 + size))
    });
}

#[test]
#[should_panic(expected = "against the control's")]
fn quadratic_work_fails_against_a_control() {
    assert_against_control(
        "quadratic",
        16,
        |size| counted(|| super::step(size * size)),
        |size| counted(|| super::step(size)),
    );
}

#[test]
#[should_panic(expected = "counted no work")]
fn no_work_fails() {
    assert_linear("none", 16, |_| counted(|| {}));
}

#[test]
#[should_panic(expected = "counted no work")]
fn no_work_fails_against_a_control() {
    assert_against_control(
        "none",
        16,
        |_| counted(|| {}),
        |size| counted(|| super::step(size)),
    );
}
