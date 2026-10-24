use core_markdown::{EventParser, Options};
use serde_json::{json, Value};
use std::io::{self, BufRead};

fn main() {
    if std::env::args().nth(1).as_deref() == Some("size") {
        println!("{}", std::mem::size_of::<EventParser<'_>>());
        return;
    }
    for line in io::stdin().lock().lines() {
        let case: Value = serde_json::from_str(&line.unwrap()).unwrap();
        let source = case["markdown"].as_str().unwrap();
        let options: Options = serde_json::from_value(case["options"].clone()).unwrap();
        let events: Vec<_> = EventParser::new(source, &options.parse)
            .unwrap()
            .map(Result::unwrap)
            .collect();
        println!("{}", json!(events));
    }
}
