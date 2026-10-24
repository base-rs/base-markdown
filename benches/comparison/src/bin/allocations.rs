//! Allocation diagnostics are separate from the timing executable so its allocator remains unchanged.
use serde_json::{json, Value};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    env, fs,
    hint::black_box,
    sync::atomic::{AtomicUsize, Ordering},
};

struct Counting;
static CALLS: AtomicUsize = AtomicUsize::new(0);
static REALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static BYTES: AtomicUsize = AtomicUsize::new(0);
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        CALLS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        System.alloc(layout)
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        CALLS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        System.alloc_zeroed(layout)
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        REALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(size, Ordering::Relaxed);
        System.realloc(pointer, layout, size)
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        System.dealloc(pointer, layout);
    }
}
#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn measure<T>(f: impl FnOnce() -> T) -> (T, Value) {
    CALLS.store(0, Ordering::Relaxed);
    REALLOCATIONS.store(0, Ordering::Relaxed);
    BYTES.store(0, Ordering::Relaxed);
    let value = black_box(f());
    let calls = CALLS.load(Ordering::Relaxed);
    let reallocations = REALLOCATIONS.load(Ordering::Relaxed);
    let bytes = BYTES.load(Ordering::Relaxed);
    (
        value,
        json!({"allocations": calls, "reallocations": reallocations, "requested_bytes": bytes}),
    )
}
fn main() {
    let args: Vec<String> = env::args().collect();
    assert_eq!(
        args.len(),
        3,
        "Usage: allocations commonmark|gfm INPUT.json"
    );
    assert!(
        matches!(args[1].as_str(), "commonmark" | "gfm"),
        "unknown profile"
    );
    let mut options = if args[1] == "gfm" {
        core_markdown::Options::gfm()
    } else {
        core_markdown::Options::default()
    };
    options.compile.allow_dangerous_html = true;
    options.compile.allow_dangerous_protocol = true;
    if args[1] == "gfm" {
        options.parse.constructs.footnote_definition = false;
        options.parse.constructs.label_start_footnote = false;
    }
    let cases: Vec<Value> = serde_json::from_str(&fs::read_to_string(&args[2]).unwrap()).unwrap();
    for case in cases {
        let source = case["markdown"].as_str().unwrap();
        let (mut parser, prepare) =
            measure(|| core_markdown::EventParser::new(source, &options.parse).unwrap());
        let (event_count, pull) = measure(|| {
            parser
                .by_ref()
                .map(|event| {
                    black_box(event.unwrap());
                    1usize
                })
                .sum::<usize>()
        });
        drop(parser);
        let (document, arena) =
            measure(|| core_markdown::Document::parse(source, &options.parse).unwrap());
        let (_, traversal) = measure(|| {
            document.events().for_each(|event| {
                black_box(event.unwrap());
            })
        });
        let (html, html_cold) =
            measure(|| core_markdown::to_html_with_options(source, &options).unwrap());
        let mut output = String::with_capacity(html.len());
        let (_, html_reserved) = measure(|| {
            core_markdown::render_events(
                source,
                core_markdown::EventParser::new(source, &options.parse).unwrap(),
                &options.compile,
                &mut output,
            )
            .unwrap()
        });
        assert_eq!(html, output);
        let (_, ast_html) = measure(|| {
            let mut output=String::new();
            document.render_html(&options.compile,&mut output).unwrap();
            output
        });
        let (_, markdown) = measure(|| core_markdown::to_markdown(&document).unwrap());
        println!(
            "{}",
            json!({"document": case["name"], "bytes": source.len(), "events": event_count, "nodes": document.nodes().len(), "node_bytes": std::mem::size_of::<core_markdown::DocumentNode>(), "prepare": prepare, "pull_after_prepare": pull, "arena_total": arena, "arena_traversal": traversal, "html_total": html_cold, "html_reserved_output": html_reserved, "ast_html":ast_html,"ast_markdown":markdown})
        );
    }
}
