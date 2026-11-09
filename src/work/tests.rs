// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Counting work, and asserting that it grows linearly with the input.

extern crate std;

use core::alloc::{GlobalAlloc, Layout};
use core::cell::Cell;
use core::fmt::{self, Debug};
use core::ops::RangeFull;
use std::alloc::System;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

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

/// A `VecDeque` that counts as steps the elements each of its operations visits or moves, which [`super::Deque`] is in tests. It has only the operations the crate uses, so that one that shifts the queue, such as `make_contiguous`, is added here with its count before it can be used.
#[derive(Clone, Debug)]
pub(crate) struct Deque<T>(VecDeque<T>);

impl<T> Deque<T> {
    pub(crate) fn new() -> Self {
        Self(VecDeque::new())
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(crate) fn push_back(&mut self, value: T) {
        add(Kind::Steps, 1);
        self.0.push_back(value);
    }

    pub(crate) fn pop_front(&mut self) -> Option<T> {
        add(Kind::Steps, 1);
        self.0.pop_front()
    }

    /// The elements in order, counting each one visited.
    pub(crate) fn iter(&self) -> impl Iterator<Item = &T> {
        self.0.iter().inspect(|_| add(Kind::Steps, 1))
    }

    /// Remove the element at `at`, counting the elements moved to close the gap, those on the shorter side, as `VecDeque::remove` moves, and the one removed.
    pub(crate) fn remove(&mut self, at: usize) -> Option<T> {
        let length = self.0.len();
        add(Kind::Steps, at.min(length.saturating_sub(at + 1)) + 1);
        self.0.remove(at)
    }

    /// Remove all the elements, counting each one.
    pub(crate) fn drain(&mut self, range: RangeFull) -> impl Iterator<Item = T> + '_ {
        add(Kind::Steps, self.0.len());
        self.0.drain(range)
    }
}

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

/// The least wall-clock time that `run` takes in five runs.
pub(crate) fn timed(mut run: impl FnMut()) -> Duration {
    (0..5)
        .map(|_| {
            let start = Instant::now();
            run();
            start.elapsed()
        })
        .fold(Duration::MAX, Duration::min)
}

/// Assert that `subject` takes no more than twice as much longer from `size` to `SCALE * size` as `control` does, in wall-clock time, in one of three tries, for work that no count sees.
///
/// Only ignored tests use this, which are run by hand on an idle host, as a loaded machine can stall any run: `subject` and `control` return the time that the part being checked took, with [`timed`], and a try over the bound is measured again, as noise makes linear work look slower only now and then, while quadratic work fails every time. The times are written to standard error, which `--nocapture` shows.
pub(crate) fn assert_time_against_control(
    label: impl Debug,
    size: usize,
    mut subject: impl FnMut(usize) -> Duration,
    mut control: impl FnMut(usize) -> Duration,
) {
    let mut ratios = std::vec::Vec::new();
    for _ in 0..3 {
        let times = [
            subject(size),
            subject(SCALE * size),
            control(size),
            control(SCALE * size),
        ]
        .map(|time| time.as_secs_f64());
        let ratio = (times[1] / times[0]) / (times[3] / times[2]);
        std::eprintln!(
            "time against control {:?}: {} -> {}: {:?} s, {:.2}x the control's growth",
            label,
            size,
            SCALE * size,
            times,
            ratio
        );
        if ratio <= 2.0 {
            return;
        }
        ratios.push(ratio);
    }
    panic!(
        "{:?}: grew {:.2?} times the control's growth in wall-clock time",
        label, ratios
    );
}

#[test]
fn counts_work_on_this_thread() {
    let counts = counted(|| {
        super::read();
        super::step!(3);
        drop(std::vec![0_u8; 5]);
    });
    assert_eq!(counts, Counts([1, 3, 5]));
    assert_eq!(std::format!("{counts:?}"), "1 reads, 3 steps, 5 bytes");
    // Spawning the thread allocates on this one, but counts none of the thread's work.
    let counts = counted(|| {
        std::thread::spawn(|| super::step!(3)).join().unwrap();
    });
    assert_eq!(counts.0[Kind::Steps as usize], 0);
}

#[test]
fn linear_work_passes() {
    assert_linear("linear", 16, |size| counted(|| super::step!(size + 4)));
    assert_against_control(
        "linear",
        16,
        |size| {
            counted(|| {
                super::step!(2 * size);
                super::read();
            })
        },
        |size| counted(|| super::step!(size)),
    );
}

#[test]
#[should_panic(expected = "8x the input counted")]
fn quadratic_work_fails() {
    assert_linear("quadratic", 16, |size| {
        counted(|| super::step!(size * size / 8 + size))
    });
}

#[test]
#[should_panic(expected = "against the control's")]
fn quadratic_work_fails_against_a_control() {
    assert_against_control(
        "quadratic",
        16,
        |size| counted(|| super::step!(size * size)),
        |size| counted(|| super::step!(size)),
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
        |size| counted(|| super::step!(size)),
    );
}

#[test]
fn searches_count_the_bytes_they_read() {
    let steps = |search: &dyn Fn()| counted(search).0[Kind::Steps as usize];
    assert_eq!(
        steps(&|| assert_eq!(super::searched!(Some(2), 8), Some(2))),
        3
    );
    assert_eq!(steps(&|| assert_eq!(super::searched!(None, 8), None)), 8);
    assert_eq!(
        steps(&|| assert_eq!(super::searched_back!(Some(2), 8), Some(2))),
        6
    );
    assert_eq!(
        steps(&|| assert_eq!(super::searched_back!(None, 8), None)),
        8
    );
}

#[test]
fn a_deque_counts_the_elements_it_visits_or_moves() {
    let steps = |run: &mut dyn FnMut()| counted(run).0[Kind::Steps as usize];
    let mut deque = super::Deque::new();
    assert!(deque.is_empty());
    assert_eq!(
        steps(&mut || (0..8).for_each(|value| deque.push_back(value))),
        8
    );
    assert_eq!(steps(&mut || assert_eq!(deque.pop_front(), Some(0))), 1);
    assert_eq!(
        steps(&mut || assert_eq!(deque.iter().position(|&value| value == 3), Some(2))),
        3
    );
    // Removing the third of seven moves the two before it.
    assert_eq!(steps(&mut || assert_eq!(deque.remove(2), Some(3))), 3);
    // Removing the second to last of six moves the last.
    assert_eq!(steps(&mut || assert_eq!(deque.remove(4), Some(6))), 2);
    let mut drained = std::vec::Vec::new();
    assert_eq!(steps(&mut || drained.extend(deque.drain(..))), 5);
    assert_eq!(drained, [1, 2, 4, 5, 7]);
    assert!(deque.is_empty());
    assert_eq!(std::format!("{:?}", deque.clone()), "Deque([])");
}
