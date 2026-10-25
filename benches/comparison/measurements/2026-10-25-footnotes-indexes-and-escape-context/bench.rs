// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

// Keep calibration, allocation accounting and the original operations unchanged.
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
    stream::{EventKind, Tag, Text},
    Document, LineWrapping, MarkdownOptions, Options,
};
use serde_json::{json, Value};
use std::{env, fs, hint::black_box};

fn main() {
    let args: Vec<_> = env::args().collect();
    let (action, mode, path) = (&args[1], &args[2], &args[3]);
    if mode == "features" {
        let cases: Vec<Value> = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
        for case in cases {
            let source = case["markdown"].as_str().unwrap();
            let name = case["name"].as_str().unwrap();
            let mut options = Options::gfm();
            options.parse.constructs.footnote_definition = true;
            options.parse.constructs.label_start_footnote = true;
            options.compile.allow_dangerous_html = true;
            options.compile.allow_dangerous_protocol = true;
            options.compile.footnote_clobber_prefix = case["prefix"].as_str().map(str::to_owned);
            options.compile.footnote_back_label = case["back_label"].as_str().map(str::to_owned);
            if action == "check" {
                let html = core_markdown::to_html_with_options(source, &options).unwrap();
                let document = Document::parse(source, &options.parse).unwrap();
                let mut arena_html = String::new();
                document
                    .render_html(&options.compile, &mut arena_html)
                    .unwrap();
                assert_eq!(html, arena_html);
                println!(
                    "{}",
                    json!({"name":name,"html":html,"markdown":core_markdown::to_markdown(&document).unwrap()})
                );
            } else {
                established::measure(action, name, mode, || {
                    core_markdown::to_html_with_options(black_box(source), &options)
                        .unwrap()
                        .len()
                });
            }
        }
        return;
    }
    if mode != "controls" {
        return established::dispatch();
    }
    let cases: Vec<Value> = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    for case in cases {
        let mut options = Options::gfm();
        options.parse.constructs.math_text = true;
        options.parse.constructs.math_flow = true;
        options.parse.constructs.frontmatter = true;
        let mut document =
            Document::parse(case["markdown"].as_str().unwrap_or(""), &options.parse).unwrap();
        if let Some(leaves) = case["leaves"].as_array() {
            let mut parent = document.root();
            if case["container"] == "task" {
                parent = document
                    .append_child(
                        parent,
                        EventKind::Start(Tag::List {
                            start: None,
                            tight: true,
                        }),
                    )
                    .unwrap();
                parent = document
                    .append_child(parent, EventKind::Start(Tag::Item { block_end: false }))
                    .unwrap();
            }
            let block = match case["container"].as_str() {
                Some("heading") => Tag::Heading(1),
                Some("quote") => Tag::BlockQuote,
                Some("footnote") => Tag::FootnoteDefinition(Text::Literal("note")),
                _ => Tag::Paragraph { tight: false },
            };
            if case["container"] != "root" {
                parent = document
                    .append_child(parent, EventKind::Start(block))
                    .unwrap();
            }
            if case["container"] == "link" || case["container"] == "image" {
                let tag = if case["container"] == "link" {
                    Tag::Link {
                        destination: Text::Literal("/url"),
                        title: None,
                    }
                } else {
                    Tag::Image {
                        destination: Text::Literal("/url"),
                        title: None,
                    }
                };
                parent = document
                    .append_child(parent, EventKind::Start(tag))
                    .unwrap();
            }
            if case["container"] == "task" {
                document
                    .append_child(parent, EventKind::TaskMarker(true))
                    .unwrap();
            }
            for _ in 0..case["repeat"].as_u64().unwrap_or(1) {
                for leaf in leaves {
                    let value = Text::from(leaf[1].as_str().unwrap_or("").to_owned());
                    let kind = match leaf[0].as_str().unwrap() {
                        "text" => EventKind::Text(value),
                        "soft" => EventKind::SoftBreak(value),
                        "code" => EventKind::Code {
                            value,
                            language: None,
                        },
                        "math" => EventKind::Math(value),
                        "math-flow" => EventKind::CodeBlock {
                            value,
                            language: None,
                            fence: None,
                            math: true,
                            unclosed: false,
                        },
                        "html" => EventKind::Html {
                            value,
                            block: false,
                        },
                        "hard" => EventKind::HardBreak,
                        "item" => EventKind::Start(Tag::Item { block_end: false }),
                        "emphasis" => EventKind::Start(Tag::Emphasis),
                        "strong" => EventKind::Start(Tag::Strong),
                        _ => panic!("unknown synthetic leaf"),
                    };
                    let id = document.append_child(parent, kind).unwrap();
                    if matches!(leaf[0].as_str(), Some("emphasis" | "strong" | "item")) {
                        document
                            .append_child(id, EventKind::Text(value_for(leaf)))
                            .unwrap();
                    }
                }
            }
        }
        let mut format = MarkdownOptions {
            line_wrapping: if case["unwrap"] == true {
                LineWrapping::Unwrap
            } else {
                LineWrapping::Preserve
            },
            single_dollar_text_math: case["double_math"] != true,
            ..MarkdownOptions::default()
        };
        if let Some(invalid) = case["invalid"].as_str() {
            match invalid {
                "quote" => format.quote = 'x',
                "bullet" => format.bullet = 'x',
                "emphasis" => format.emphasis = 'x',
                "strong" => format.strong = 'x',
                "fence" => format.fence = 'x',
                _ => panic!("unknown invalid option"),
            }
        }
        let name = case["name"].as_str().unwrap();
        if action == "check" {
            let result = match core_markdown::to_markdown_with_options(&document, &format) {
                Ok(markdown) => json!({"markdown":markdown}),
                Err(error) => {
                    json!({"error":{"span":error.span,"reason":error.reason,"source":error.source,"rule_id":error.rule_id}})
                }
            };
            println!("{}", json!({"name":name,"result":result}));
        } else {
            established::measure(action, name, mode, || {
                core_markdown::to_markdown_with_options(black_box(&document), &format)
                    .unwrap()
                    .len()
            });
        }
    }
}

fn value_for(leaf: &Value) -> Text<'static> {
    Text::from(leaf[1].as_str().unwrap_or("").to_owned())
}
