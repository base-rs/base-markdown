#![no_main]

// Released under the MIT License.
// Copyright, 2022, by Bernhard Berger.
// Copyright, 2022, by Titus Wormer.
// Copyright, 2022-2023, by Christian Murphy.
// Copyright, 2026, by Samuel Williams.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = core_markdown::to_html(s);
        let _ = core_markdown::to_html_with_options(s, &core_markdown::Options::gfm());
        let _ = core_markdown::to_mdast(s, &core_markdown::ParseOptions::default());
        let _ = core_markdown::to_mdast(s, &core_markdown::ParseOptions::gfm());
        let _ = core_markdown::to_mdast(s, &core_markdown::ParseOptions::mdx());
    }
});
