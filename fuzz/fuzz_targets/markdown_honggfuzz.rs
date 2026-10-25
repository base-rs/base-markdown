// Released under the MIT License.
// Copyright, 2023, by Christian Murphy.
// Copyright, 2026, by Samuel Williams.

use honggfuzz::fuzz;

fn main() {
    loop {
        fuzz!(|data: &[u8]| {
            if let Ok(s) = std::str::from_utf8(data) {
                let _ = core_markdown::to_html(s);
                let _ = core_markdown::to_html_with_options(s, &core_markdown::Options::gfm());
                let _ = core_markdown::Document::parse(s, &core_markdown::ParseOptions::default());
                let _ = core_markdown::Document::parse(s, &core_markdown::ParseOptions::gfm());
            }
        });
    }
}
