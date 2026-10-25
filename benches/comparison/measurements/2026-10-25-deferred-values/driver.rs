#[cfg(feature = "diagnostics")]
use core_markdown::EventParser;
use core_markdown::{Document, HeadingOptions, MarkdownOptions, Options};
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
        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            if ACTIVE.load(Relaxed) {
                ALLOC.fetch_add(1, Relaxed);
                BYTES.fetch_add(layout.size(), Relaxed);
            }
            unsafe { System.alloc_zeroed(layout) }
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
        action();
        ALLOC.store(0, Relaxed);
        REALLOC.store(0, Relaxed);
        BYTES.store(0, Relaxed);
        ACTIVE.store(true, Relaxed);
        let output_bytes = action();
        ACTIVE.store(false, Relaxed);
        serde_json::json!({"allocations":ALLOC.load(Relaxed), "reallocations":REALLOC.load(Relaxed),
            "requested_bytes":BYTES.load(Relaxed), "output_bytes":output_bytes})
    }
}

fn options(case: &Value) -> Options {
    let mut options = if case["profile"] == "gfm" {
        Options::gfm()
    } else {
        Options::default()
    };
    options.compile.allow_dangerous_html = true;
    options.compile.allow_dangerous_protocol = true;
    options.compile.heading_ids = case["heading_ids"].as_bool().unwrap_or(false);
    if case["profile"] == "gfm" {
        options.parse.constructs.footnote_definition = false;
        options.parse.constructs.label_start_footnote = false;
    }
    options
}

// Compare semantic values and authored spans independently of source/owned storage.
#[cfg(feature = "diagnostics")]
fn resolved(value: Value, source: &str) -> Value {
    match value {
        Value::Object(mut map) => {
            if map.len() == 1 {
                if let Some(range) = map.get("Source") {
                    let start = range["start"].as_u64().unwrap() as usize;
                    let end = range["end"].as_u64().unwrap() as usize;
                    return json!(&source[start..end]);
                }
                if let Some(text) = map.get("Value") {
                    return text.clone();
                }
            }
            for value in map.values_mut() {
                *value = resolved(value.take(), source);
            }
            Value::Object(map)
        }
        Value::Array(values) => {
            Value::Array(values.into_iter().map(|v| resolved(v, source)).collect())
        }
        value => value,
    }
}

fn markdown_options(case: &Value) -> MarkdownOptions {
    MarkdownOptions {
        setext: case["setext"].as_bool().unwrap_or(false),
        ..MarkdownOptions::default()
    }
}

#[cfg(feature = "diagnostics")]
fn check(case: &Value) -> Value {
    let source = case["markdown"].as_str().unwrap();
    let options = options(case);
    let document = Document::parse(source, &options.parse).unwrap();
    let mut ast_html = String::new();
    document
        .render_html(&options.compile, &mut ast_html)
        .unwrap();
    let events: Vec<_> = EventParser::new(source, &options.parse)
        .map(Result::unwrap)
        .collect();
    let ast_events: Vec<_> = document.events().map(Result::unwrap).collect();
    let markdown = match core_markdown::to_markdown_with_options(&document, &markdown_options(case))
    {
        Ok(value) => json!({"ok":value}),
        Err(error) => json!({"error":format!("{error:?}")}),
    };
    let headings: Vec<_> = document
        .headings(&HeadingOptions::default())
        .into_iter()
        .map(|h| json!({"level":h.level,"text":h.text,"anchor":h.anchor}))
        .collect();
    json!({"name":case["name"],"profile":case["profile"],
        "html":core_markdown::to_html_with_options(source, &options),"ast_html":ast_html,
        "markdown":markdown,"events":resolved(serde_json::to_value(events).unwrap(),source),
        "ast_events":resolved(serde_json::to_value(ast_events).unwrap(),source),"headings":headings})
}

#[cfg(not(feature = "diagnostics"))]
fn check(_: &Value) -> Value {
    panic!("check requires the diagnostics feature");
}

fn run(action: &str, mode: &str, case: &Value, count: u64, warmup_ms: u64) -> Value {
    let source = case["markdown"].as_str().unwrap();
    let options = options(case);
    let markdown_options = markdown_options(case);
    // Document preparation is deliberately excluded from the prepared operations.
    let document = if mode == "html" {
        None
    } else {
        Some(Document::parse(source, &options.parse).unwrap())
    };
    let mut operation = || match mode {
        "html" => core_markdown::to_html_with_options(black_box(source), black_box(&options)).len(),
        "ast-html" => {
            let mut output = String::new();
            black_box(document.as_ref().unwrap())
                .render_html(black_box(&options.compile), &mut output)
                .unwrap();
            black_box(output).len()
        }
        "markdown" => core_markdown::to_markdown_with_options(
            black_box(document.as_ref().unwrap()),
            black_box(&markdown_options),
        )
        .unwrap()
        .len(),
        "headings" => black_box(document.as_ref().unwrap())
            .headings(&HeadingOptions::default())
            .iter()
            .map(|h| h.text.len() + h.anchor.len())
            .sum(),
        _ => panic!("unknown operation: {mode}"),
    };
    if action == "alloc" {
        #[cfg(feature = "allocations")]
        return json!({"name":case["name"],"mode":mode,"counts":allocations::measure(operation)});
        #[cfg(not(feature = "allocations"))]
        panic!("alloc requires the allocations feature");
    }
    assert_eq!(action, "bench");
    let warmup = Instant::now();
    let mut warm_count = 0u64;
    loop {
        black_box(operation());
        warm_count += 1;
        if warmup.elapsed() >= Duration::from_millis(warmup_ms) {
            break;
        }
    }
    let count = if count == 0 {
        ((warm_count as f64 / warmup.elapsed().as_secs_f64()) * 0.12)
            .ceil()
            .max(1.) as u64
    } else {
        count
    };
    let start = Instant::now();
    let mut output_bytes = 0;
    for _ in 0..count {
        output_bytes = black_box(operation());
    }
    let seconds = start.elapsed().as_secs_f64();
    json!({"name":case["name"],"profile":case["profile"],"mode":mode,"input_bytes":source.len(),
        "iterations":count,"seconds":seconds,"seconds_per_iteration":seconds / count as f64,
        "output_bytes":output_bytes,"allocation_instrumented":cfg!(feature="allocations"),"diagnostics_enabled":cfg!(feature="diagnostics")})
}

fn main() {
    let args: Vec<_> = env::args().collect();
    assert!(args.len() >= 3, "check CASES.json [NAME] | bench MODE CASES.json [NAME] [ITERATIONS] [WARMUP_MS] | alloc MODE CASES.json [NAME]");
    let action = args[1].as_str();
    let (mode, path, offset) = if action == "check" {
        ("", &args[2], 3)
    } else {
        (args[2].as_str(), &args[3], 4)
    };
    let filter = args.get(offset).map(String::as_str).unwrap_or("*");
    let count = args
        .get(offset + 1)
        .map(|s| s.parse().unwrap())
        .unwrap_or(0);
    let warmup = args
        .get(offset + 2)
        .map(|s| s.parse().unwrap())
        .unwrap_or(100);
    let cases: Vec<Value> = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    let mut matched = false;
    for case in cases {
        if filter != "*" && case["name"] != filter {
            continue;
        }
        matched = true;
        println!(
            "{}",
            if action == "check" {
                check(&case)
            } else {
                run(action, mode, &case, count, warmup)
            }
        );
    }
    assert!(matched, "case filter did not match");
}
