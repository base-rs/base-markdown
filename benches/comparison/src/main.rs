use serde_json::{json, Value};
use std::{
    env, fs,
    hint::black_box,
    time::{Duration, Instant},
};

fn renderer(parser: &str, profile: &str) -> Box<dyn Fn(&str) -> String> {
    match parser {
        "core-stream" => {
            let mut options = if profile.starts_with("gfm") {
                core_markdown::Options::gfm()
            } else {
                core_markdown::Options::default()
            };
            if profile != "default" {
                options.compile.allow_dangerous_html = true;
                options.compile.allow_dangerous_protocol = true;
            }
            if profile == "gfm" {
                options.parse.constructs.gfm_footnote_definition = false;
                options.parse.constructs.gfm_label_start_footnote = false;
            }
            Box::new(move |s| {
                let events = core_markdown::EventParser::new(s, &options.parse).unwrap();
                let mut output = String::new();
                core_markdown::render_events(s, events, &options.compile, &mut output).unwrap();
                output
            })
        }
        "core-markdown" | "core-ast" | "core-arena" | "core-prepare" | "core-events" | "core-markdown-output" => {
            let markdown = parser == "core-markdown-output";
            let prepare = parser == "core-prepare";
            let events = parser == "core-events";
            let arena = matches!(parser,"core-arena"|"core-ast");
            let mut options = if profile.starts_with("gfm") {
                core_markdown::Options::gfm()
            } else {
                core_markdown::Options::default()
            };
            if profile != "default" {
                options.compile.allow_dangerous_html = true;
                options.compile.allow_dangerous_protocol = true;
            }
            if profile == "gfm" {
                options.parse.constructs.gfm_footnote_definition = false;
                options.parse.constructs.gfm_label_start_footnote = false;
            }
            Box::new(move |s| {
                if prepare || events {
                    let parser = core_markdown::EventParser::new(s, &options.parse).unwrap();
                    if events {
                        for event in parser {
                            black_box(event.unwrap());
                        }
                    } else {
                        black_box(parser);
                    }
                    String::new()
                } else if markdown {
                    let document = core_markdown::Document::parse(s, &options.parse).unwrap();
                    core_markdown::to_markdown(&document).unwrap()
                } else if arena {
                    let document = core_markdown::Document::parse(s, &options.parse).unwrap();
                    let mut output = String::new();
                    core_markdown::render_events(
                        s,
                        document.events(),
                        &options.compile,
                        &mut output,
                    )
                    .unwrap();
                    output
                } else {
                    core_markdown::to_html_with_options(s, &options).unwrap()
                }
            })
        }
        "markdown-rs" => {
            let mut options = if profile.starts_with("gfm") {
                markdown::Options::gfm()
            } else {
                markdown::Options::default()
            };
            if profile != "default" {
                options.compile.allow_dangerous_html = true;
                options.compile.allow_dangerous_protocol = true;
            }
            if profile == "gfm" {
                options.parse.constructs.gfm_footnote_definition = false;
                options.parse.constructs.gfm_label_start_footnote = false;
            }
            Box::new(move |s| markdown::to_html_with_options(s, &options).unwrap())
        }
        "comrak" => {
            let mut options = comrak::Options::default();
            if profile != "default" {
                options.render.r#unsafe = true;
            }
            if profile.starts_with("gfm") {
                options.extension.strikethrough = true;
                options.extension.table = true;
                options.extension.autolink = true;
                options.extension.tasklist = true;
                options.extension.footnotes = profile == "gfm-footnotes";
            }
            Box::new(move |s| comrak::markdown_to_html(s, &options))
        }
        _ => panic!("unknown parser"),
    }
}

fn batch(f: &dyn Fn(&str) -> String, s: &str, n: u64) -> usize {
    let mut total = 0;
    for _ in 0..n {
        total += black_box(f(black_box(s))).len();
    }
    black_box(total)
}
fn main() {
    let a: Vec<String> = env::args().collect();
    if a.len() != 5 || !matches!(a[1].as_str(), "bench" | "render" | "profile") {
        eprintln!("Usage: markdown-comparison bench|render|profile PARSER PROFILE INPUT.json");
        std::process::exit(2);
    }
    let (action, parser, profile, path) = (&a[1], &a[2], &a[3], &a[4]);
    let cases: Vec<Value> = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    let f = renderer(parser, profile);
    if action == "profile" {
        let s = cases.last().unwrap()["markdown"].as_str().unwrap();
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(30) {
            batch(&f, s, 1);
        }
    } else if action == "render" {
        let out: Vec<_> = cases
            .iter()
            .map(|c| f(c["markdown"].as_str().unwrap()))
            .collect();
        println!("{}", serde_json::to_string(&out).unwrap());
    } else {
        for c in cases {
            let s = c["markdown"].as_str().unwrap();
            let start = Instant::now();
            let mut warm = 0;
            while start.elapsed() < Duration::from_millis(200) {
                batch(&f, s, 1);
                warm += 1;
            }
            let n = ((warm as f64 / start.elapsed().as_secs_f64()) * 0.15)
                .ceil()
                .max(1.0) as u64;
            let mut times = Vec::new();
            for _ in 0..7 {
                let start = Instant::now();
                batch(&f, s, n);
                times.push(start.elapsed().as_secs_f64() / n as f64);
            }
            times.sort_by(f64::total_cmp);
            println!(
                "{}",
                json!({"parser":parser,"profile":profile,"document":c["name"],"bytes":s.len(),"iterations_per_sample":n,"samples_seconds":times,"median_seconds":times[3],"mib_per_second":s.len() as f64/times[3]/1048576.0})
            );
        }
    }
}
