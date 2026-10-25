// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

// Reuse the preceding adapter, calibration and counting allocator unchanged.
mod previous {
    include!("previous.rs");
    pub fn dispatch() {
        main();
    }
    pub fn measure(action: &str, name: &str, mode: &str, operation: impl FnMut() -> usize) {
        established::measure(action, name, mode, operation);
    }
}
use core_markdown::{Document, HeadingOptions, Options};
use serde_json::{json, Value};
use std::{env, fs, hint::black_box};

fn main() {
    let args: Vec<_> = env::args().collect();
    if args[2] != "features" {
        return previous::dispatch();
    }
    let cases: Vec<Value> = serde_json::from_str(&fs::read_to_string(&args[3]).unwrap()).unwrap();
    if cases[0].get("operation").is_none() {
        return previous::dispatch();
    }
    for case in cases {
        let source = case["markdown"].as_str().unwrap();
        let name = case["name"].as_str().unwrap();
        let mut options = Options::default();
        options.compile.heading_ids = case["headings"].as_bool().unwrap_or(false);
        options.parse.constructs.frontmatter = case["frontmatter"].as_bool().unwrap_or(false);
        let document = Document::parse(source, &options.parse).unwrap();
        if args[1] == "check" {
            let direct = core_markdown::to_html_with_options(source, &options).unwrap();
            let mut arena = String::new();
            document.render_html(&options.compile, &mut arena).unwrap();
            assert_eq!(direct, arena);
            let headings: Vec<_> = document.headings(&HeadingOptions::default()).into_iter()
                .map(|heading| json!([heading.level, heading.text, heading.anchor])).collect();
            let syntax: Vec<_> = document.descendants(document.root())
                .map(|id| format!("{:?}", document.syntax(id))).collect();
            println!("{}", json!({"name":name,"html":direct,"headings":headings,"syntax":syntax,
                "markdown":core_markdown::to_markdown(&document).unwrap()}));
        } else {
            previous::measure(&args[1], name, "features", || match case["operation"].as_str().unwrap() {
                "html" => core_markdown::to_html_with_options(black_box(source), &options).unwrap().len(),
                "headings" => black_box(document.headings(&HeadingOptions::default())).len(),
                _ => panic!("unknown operation"),
            });
        }
    }
}
