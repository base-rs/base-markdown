// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use super::*;
use alloc::format;

#[test]
fn constructs_presets_enable_their_extensions() {
    let commonmark = Constructs::commonmark();
    assert!(commonmark.attention);
    assert!(!commonmark.autolink_literal);
    assert!(!commonmark.frontmatter);
    let gfm = Constructs::gfm();
    assert!(gfm.attention);
    assert!(gfm.autolink_literal);
    assert!(!gfm.frontmatter);
    let default = Constructs::default();
    assert_eq!(
        default,
        Constructs {
            frontmatter: true,
            ..Constructs::gfm()
        }
    );
}

#[test]
fn parse_presets_and_debug_retain_markdown_options() {
    let default = ParseOptions::default();
    assert_eq!(default.constructs, Constructs::default());
    assert!(!default.strikethrough_single_tilde);
    let commonmark = ParseOptions::commonmark();
    assert_eq!(commonmark.constructs, Constructs::commonmark());
    // Only the constructs differ, so an extension turned on afterward behaves as in the default.
    assert_eq!(
        format!("{commonmark:?}"),
        format!(
            "{:?}",
            ParseOptions {
                constructs: Constructs::commonmark(),
                ..ParseOptions::default()
            }
        )
    );
    let gfm = ParseOptions::gfm();
    assert_eq!(gfm.constructs, Constructs::gfm());
    assert!(gfm.strikethrough_single_tilde);
    assert!(format!("{commonmark:?}").contains("inline_code_info: false"));
}

#[test]
fn compile_presets_and_options_keep_safe_defaults() {
    let commonmark = CompileOptions::commonmark();
    assert!(!commonmark.allow_dangerous_html);
    assert!(!commonmark.tagfilter);
    assert!(!CompileOptions::default().tagfilter);
    let gfm = CompileOptions::gfm();
    assert!(!gfm.allow_dangerous_html);
    assert!(gfm.tagfilter);
    assert_eq!(Options::default().parse.constructs, Constructs::default());
    assert_eq!(
        Options::commonmark().parse.constructs,
        Constructs::commonmark()
    );
    assert!(!Options::commonmark().compile.tagfilter);
    assert_eq!(Options::gfm().parse.constructs, Constructs::gfm());
}
