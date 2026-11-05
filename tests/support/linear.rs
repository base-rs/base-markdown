// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! A wall-clock check that work grows linearly with its input, shared by the scaling tests.

use std::fmt::Debug;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

/// Held while a scaling check times, so that only one times at once in a test binary, as large allocations in parallel tests contend with each other. Other tests that load the machine heavily, such as the fuzz run, can take it too.
pub static TIMING: Mutex<()> = Mutex::new(());

/// How many times larger the large input is than the small one.
pub const SCALE: usize = 8;

/// The largest ratio of the two times that passes.
///
/// Linear time takes about 8 times as long, and quadratic time takes 64 times as long once its quadratic part dominates, which measures 40 to 58 times at these sizes. Under heavy load, linear time stayed under 19 times.
pub const BOUND: f64 = 20.0;

/// Assert that `SCALE` times the input, `size` against `SCALE * size`, takes less than `BOUND` times as long, after a warm-up.
///
/// `time` builds an input of the given size, runs the work and returns how long the part being checked took. Comparing the ratio of two sizes, rather than a time, means a slower machine or build slows both alike. Each size takes the fastest of five runs, alternating between the sizes, so that a pause or a stretch of load slows both alike and does not count. A ratio over the bound is measured again up to twice, since noise makes linear work look slower only now and then, while quadratic work fails every time.
///
/// The ratios are written to standard error, which `--nocapture` shows.
pub fn assert_linear(label: impl Debug, size: usize, mut time: impl FnMut(usize) -> Duration) {
    let _timing = TIMING.lock().unwrap_or_else(PoisonError::into_inner);
    time(size);
    let mut ratios = Vec::new();
    for _ in 0..3 {
        let mut small = Duration::MAX;
        let mut large = Duration::MAX;
        for _ in 0..5 {
            small = small.min(time(size));
            large = large.min(time(SCALE * size));
        }
        let ratio = large.as_secs_f64() / small.as_secs_f64();
        eprintln!(
            "linear {:?}: {} -> {}: {:?} -> {:?}, {:.1}x",
            label,
            size,
            SCALE * size,
            small,
            large,
            ratio
        );
        if ratio < BOUND {
            return;
        }
        ratios.push(ratio);
    }
    panic!(
        "{:?}: {}x input took {:.1?} times as long, against a bound of {}",
        label, SCALE, ratios, BOUND
    );
}

/// How long `run` takes.
pub fn timed(run: impl FnOnce()) -> Duration {
    let started = Instant::now();
    run();
    started.elapsed()
}

/// The largest ratio of the subject's growth to the control's that passes.
///
/// Work that grows as the control does gives a ratio near 1, and logarithmic factors keep it under 1.5 at these sizes, while work that grows with the square of the input gives 8 once its quadratic part dominates.
pub const CONTROL_BOUND: f64 = 2.5;

/// Assert that `subject` grows from `size` to `SCALE * size` no more than `CONTROL_BOUND` times as much as `control` does, measured in the same run.
///
/// The control does the same work on inputs that keep what is checked small, such as many short documents in place of one long one, so the ratio of the two growths shows only the work that depends on what is checked, whatever the machine, build or load. Each size of each takes the fastest of five runs, alternating, and a ratio over the bound is measured again up to twice, as in [`assert_linear`].
pub fn assert_against_control(
    label: impl Debug,
    size: usize,
    mut subject: impl FnMut(usize) -> Duration,
    mut control: impl FnMut(usize) -> Duration,
) {
    let _timing = TIMING.lock().unwrap_or_else(PoisonError::into_inner);
    subject(size);
    control(size);
    let mut ratios = Vec::new();
    for _ in 0..3 {
        let mut times = [Duration::MAX; 4];
        for _ in 0..5 {
            times[0] = times[0].min(subject(size));
            times[1] = times[1].min(subject(SCALE * size));
            times[2] = times[2].min(control(size));
            times[3] = times[3].min(control(SCALE * size));
        }
        let [small, large, control_small, control_large] = times.map(|time| time.as_secs_f64());
        let ratio = (large / small) / (control_large / control_small);
        eprintln!(
            "against control {:?}: {} -> {}: {:?}, {:.2}x the control's growth",
            label,
            size,
            SCALE * size,
            times,
            ratio
        );
        if ratio < CONTROL_BOUND {
            return;
        }
        ratios.push(ratio);
    }
    panic!(
        "{:?}: grew {:.2?} times the control's growth, against a bound of {}",
        label, ratios, CONTROL_BOUND
    );
}
