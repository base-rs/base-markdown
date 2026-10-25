// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

// Reuse the preceding study's calibration, allocation counter and original workloads.
mod established {
    include!("established.rs");
    pub fn dispatch() {
        main();
    }
    pub fn measure(action: &str, name: &str, mode: &str, operation: impl FnMut() -> usize) {
        run(action, name, mode, operation);
    }
}
use core_markdown::{
    stream::{EventKind, Text},
    Document, LineWrapping, MarkdownOptions, Options,
};
use serde_json::{json, Value};
use std::{env, fs, hint::black_box};

fn main() {
    let args: Vec<_> = env::args().collect();
    let (action, mode, path) = (&args[1], &args[2], &args[3]);
    if !matches!(mode.as_str(), "join" | "unwrap" | "remove") {
        return established::dispatch();
    }
    let cases: Vec<Value> = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    for case in cases {
        let name = case["name"].as_str().unwrap();
        let source = case["markdown"].as_str().unwrap();
        let mut options = if case["profile"] == "gfm" {
            Options::gfm()
        } else {
            Options::default()
        };
        options.compile.allow_dangerous_html = true;
        options.compile.allow_dangerous_protocol = true;
        if case["profile"] == "gfm" {
            options.parse.constructs.footnote_definition = false;
            options.parse.constructs.label_start_footnote = false;
        }
        let mut document = Document::parse(
            if case["synthetic"] == true {
                ""
            } else {
                source
            },
            &options.parse,
        )
        .unwrap();
        if case["synthetic"] == true {
            document
                .append_child(
                    document.root(),
                    EventKind::Text(Text::from(source.to_owned())),
                )
                .unwrap();
        }
        if mode == "remove" {
            let index = document
                .children(document.root())
                .position(|id| document.text_content(id) == "Replace")
                .unwrap();
            let replacement = Document::parse("", &options.parse).unwrap();
            if action == "check" {
                document
                    .replace_heading_section(document.root(), index, &replacement, true, true)
                    .unwrap();
                let stored = serde_json::to_value(&document).unwrap();
                let restored: Document = serde_json::from_value(stored.clone()).unwrap();
                let mut html = String::new();
                document.render_html(&options.compile, &mut html).unwrap();
                let mut other = String::new();
                restored.render_html(&options.compile, &mut other).unwrap();
                assert_eq!(html, other);
                println!(
                    "{}",
                    json!({"name":name,"document":stored,"html":html,"markdown":core_markdown::to_markdown(&document).unwrap()})
                );
            } else {
                established::measure(action, name, mode, || {
                    let mut target = black_box(&document).clone();
                    target
                        .replace_heading_section(target.root(), index, &replacement, true, true)
                        .unwrap();
                    black_box(&target).nodes().len()
                });
            }
        } else {
            let format = MarkdownOptions {
                line_wrapping: if mode == "unwrap" {
                    LineWrapping::Unwrap
                } else {
                    LineWrapping::Preserve
                },
                ..MarkdownOptions::default()
            };
            if action == "check" {
                println!(
                    "{}",
                    json!({"name":name,"markdown":core_markdown::to_markdown_with_options(&document,&format).unwrap()})
                );
            } else {
                established::measure(action, name, mode, || {
                    core_markdown::to_markdown_with_options(black_box(&document), &format)
                        .unwrap()
                        .len()
                });
            }
        }
    }
}
