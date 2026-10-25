// Released under the MIT License.
// Copyright, 2022, by Bernhard Berger.
// Copyright, 2022-2025, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

fn main() {
    // Safely turn (untrusted?) markdown into HTML.
    println!("{:?}", base_markdown::to_html("## Hello, *world*!"));

    // Turn trusted markdown into HTML.
    println!(
        "{:?}",
        base_markdown::to_html_with_options(
            "<div style=\"color: goldenrod\">\n\n# Hi, *Saturn*! 🪐\n\n</div>",
            &base_markdown::Options {
                compile: base_markdown::CompileOptions {
                    allow_dangerous_html: true,
                    allow_dangerous_protocol: true,
                    ..base_markdown::CompileOptions::default()
                },
                ..base_markdown::Options::default()
            }
        )
    );

    // Support GFM extensions.
    println!(
        "{}",
        base_markdown::to_html_with_options(
            "* [x] contact ~Mercury~Venus at hi@venus.com!",
            &base_markdown::Options::gfm()
        )
    );

    // Access the syntax tree:
    println!(
        "{:?}",
        base_markdown::Document::parse(
            "# Hello, *world*!",
            &base_markdown::ParseOptions::default()
        )
        .unwrap()
    );
}
