// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use std::io::{self, BufRead};
use core_markdown::{Options, EventParser, Document, render_events, to_html_with_options, to_markdown};
use serde_json::{json, Value};
fn main() {
    std::panic::set_hook(Box::new(|_| {}));
    for line in io::stdin().lock().lines() {
        let case: Value = serde_json::from_str(&line.unwrap()).unwrap();
        let source = case["markdown"].as_str().unwrap();
        let options: Options = serde_json::from_value(case["options"].clone()).unwrap_or_else(|e| { eprintln!("{e}: {}",case["options"]); std::process::exit(2) });
        let result = std::panic::catch_unwind(|| {
            let html = to_html_with_options(source, &options).unwrap();
            let mut checked = String::new();
            render_events(source, EventParser::new(source, &options.parse).unwrap(), &options.compile, &mut checked).unwrap();
            let doc = Document::parse(source, &options.parse).unwrap();
            let mut arena = String::new();
            render_events(source, doc.events(), &options.compile, &mut arena).unwrap();
            let markdown = to_markdown(&doc).map_err(|e| e.to_string());
            json!({"html":html,"checked":checked,"arena":arena,"markdown":markdown})
        });
        println!("{}", result.unwrap_or_else(|_| json!({"panic":true})));
    }
}
