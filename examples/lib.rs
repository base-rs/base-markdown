// Released under the MIT License.
// Copyright, 2022, by Bernhard Berger.
// Copyright, 2022-2025, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

fn main() -> Result<(), core_markdown::message::Message> {
    // Turn on debugging.
    // You can show it with `RUST_LOG=debug cargo run --features log --example lib`
    env_logger::init();

    // Safely turn (untrusted?) markdown into HTML.
    println!("{:?}", core_markdown::to_html("## Hello, *world*!"));

    // Turn trusted markdown into HTML.
    println!(
        "{:?}",
        core_markdown::to_html_with_options(
            "<div style=\"color: goldenrod\">\n\n# Hi, *Saturn*! 🪐\n\n</div>",
            &core_markdown::Options {
                compile: core_markdown::CompileOptions {
                    allow_dangerous_html: true,
                    allow_dangerous_protocol: true,
                    ..core_markdown::CompileOptions::default()
                },
                ..core_markdown::Options::default()
            }
        )
    );

    // Support GFM extensions.
    println!(
        "{}",
        core_markdown::to_html_with_options(
            "* [x] contact ~Mercury~Venus at hi@venus.com!",
            &core_markdown::Options::gfm()
        )?
    );

    // Access the syntax tree:
    println!(
        "{:?}",
        core_markdown::to_mdast("# Hello, *world*!", &core_markdown::ParseOptions::default())?
    );

    Ok(())
}
