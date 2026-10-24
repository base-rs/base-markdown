// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use core_markdown::{
    stream::{EventKind, Tag},
    Document, Options,
};
use serde_json::{json, Value};
use std::{
    env, fs,
    hint::black_box,
    time::{Duration, Instant},
};

#[cfg(feature = "allocations")]
mod allocations {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering::Relaxed};
    pub static ACTIVE: AtomicBool = AtomicBool::new(false);
    pub static ALLOC: AtomicUsize = AtomicUsize::new(0);
    pub static REALLOC: AtomicUsize = AtomicUsize::new(0);
    pub static BYTES: AtomicUsize = AtomicUsize::new(0);
    pub struct Counter;
    unsafe impl GlobalAlloc for Counter {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            if ACTIVE.load(Relaxed) {
                ALLOC.fetch_add(1, Relaxed);
                BYTES.fetch_add(layout.size(), Relaxed);
            }
            unsafe { System.alloc(layout) }
        }
        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            unsafe { System.dealloc(ptr, layout) }
        }
        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
            if ACTIVE.load(Relaxed) {
                REALLOC.fetch_add(1, Relaxed);
                BYTES.fetch_add(size, Relaxed);
            }
            unsafe { System.realloc(ptr, layout, size) }
        }
    }
    #[global_allocator]
    static GLOBAL: Counter = Counter;
    pub fn measure(mut action: impl FnMut() -> usize) -> serde_json::Value {
        // Initialize any caller-owned reusable storage before counting one operation.
        action();
        ALLOC.store(0, Relaxed);
        REALLOC.store(0, Relaxed);
        BYTES.store(0, Relaxed);
        ACTIVE.store(true, Relaxed);
        let result = action();
        ACTIVE.store(false, Relaxed);
        serde_json::json!({"allocations":ALLOC.load(Relaxed), "reallocations":REALLOC.load(Relaxed),
            "requested_bytes":BYTES.load(Relaxed), "result":result})
    }
}

fn run(action: &str, name: &str, mode: &str, mut operation: impl FnMut() -> usize) {
    if action == "alloc" {
        #[cfg(feature = "allocations")]
        {
            println!(
                "{}",
                json!({"document":name,"mode":mode,"counts":allocations::measure(operation)})
            );
            return;
        }
        #[cfg(not(feature = "allocations"))]
        panic!("build with allocations for diagnostics");
    }
    assert_eq!(action, "bench");
    let batch = |operation: &mut dyn FnMut() -> usize, count: u64| {
        for _ in 0..count {
            black_box(operation());
        }
    };
    let warmup = Instant::now();
    let mut warm = 0;
    while warmup.elapsed() < Duration::from_millis(200) {
        batch(&mut operation, 1);
        warm += 1;
    }
    let count = ((warm as f64 / warmup.elapsed().as_secs_f64()) * 0.15)
        .ceil()
        .max(1.) as u64;
    let mut samples = Vec::new();
    for _ in 0..7 {
        let start = Instant::now();
        batch(&mut operation, count);
        samples.push(start.elapsed().as_secs_f64() / count as f64);
    }
    let mut ordered = samples.clone();
    ordered.sort_by(f64::total_cmp);
    println!(
        "{}",
        json!({"document":name,"mode":mode,"iterations_per_sample":count,
        "samples_seconds":samples,"median_seconds":ordered[3]})
    );
}

fn options(profile: &str) -> Options {
    let mut options = if profile == "gfm" {
        Options::gfm()
    } else {
        Options::default()
    };
    options.compile.allow_dangerous_html = true;
    options.compile.allow_dangerous_protocol = true;
    if profile == "gfm" {
        options.parse.constructs.footnote_definition = false;
        options.parse.constructs.label_start_footnote = false;
    }
    options
}

fn main() {
    let args: Vec<_> = env::args().collect();
    let (action, mode, path) = (&args[1], &args[2], &args[3]);
    let cases: Vec<Value> = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    for case in cases {
        let name = case["name"].as_str().unwrap();
        let source = case["markdown"].as_str().unwrap();
        let options = options(case["profile"].as_str().unwrap_or("commonmark"));
        if mode == "html" {
            run(action, name, mode, || {
                core_markdown::to_html_with_options(black_box(source), &options)
                    .unwrap()
                    .len()
            });
            continue;
        }
        let document = Document::parse(source, &options.parse).unwrap();
        if mode == "markdown" {
            run(action, name, mode, || {
                core_markdown::to_markdown(black_box(&document))
                    .unwrap()
                    .len()
            });
        } else if mode == "replace" {
            let target = Document::parse(case["target"].as_str().unwrap(), &options.parse).unwrap();
            let index = target
                .children(target.root())
                .position(|id| target.text_content(id) == "Replace")
                .unwrap();
            if action == "check" {
                let mut target = target.clone();
                target
                    .replace_heading_section(target.root(), index, &document, true, true)
                    .unwrap();
                let mut html = String::new();
                target.render_html(&options.compile, &mut html).unwrap();
                // Serialize the complete arena, including detached original nodes and syntax overrides.
                let stored = serde_json::to_value(&target).unwrap();
                let restored: Document = serde_json::from_value(stored.clone()).unwrap();
                let mut restored_html = String::new();
                restored
                    .render_html(&options.compile, &mut restored_html)
                    .unwrap();
                assert_eq!(html, restored_html);
                println!(
                    "{}",
                    json!({"name":name,"document":stored,"html":html,
                    "markdown":core_markdown::to_markdown(&target).unwrap()})
                );
            } else {
                run(action, name, mode, || {
                    let mut target = black_box(&target).clone();
                    target
                        .replace_heading_section(
                            target.root(),
                            index,
                            black_box(&document),
                            true,
                            true,
                        )
                        .unwrap();
                    black_box(&target).nodes().len()
                });
            }
        } else if mode == "move" {
            let mut document = document;
            let first = document.children(document.root()).next().unwrap();
            let subtree = document.children(first).next().unwrap();
            let second = document
                .append_child(document.root(), EventKind::Start(Tag::BlockQuote))
                .unwrap();
            run(action, name, mode, || {
                document.attach(second, subtree, None).unwrap();
                document.attach(first, subtree, None).unwrap();
                black_box(&document).nodes().len()
            });
        } else {
            panic!("unknown mode")
        }
    }
}
