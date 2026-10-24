// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use super::*;
use alloc::format;

#[test]
fn constructs_presets_enable_commonmark_and_gfm() {
    let commonmark = Constructs::default();
    assert!(commonmark.attention);
    assert!(!commonmark.gfm_autolink_literal);
    assert!(!commonmark.frontmatter);
    let gfm = Constructs::gfm();
    assert!(gfm.attention);
    assert!(gfm.gfm_autolink_literal);
    assert!(!gfm.frontmatter);
}

#[test]
fn parse_presets_and_debug_retain_markdown_options() {
    let commonmark = ParseOptions::default();
    assert_eq!(commonmark.constructs, Constructs::default());
    assert_eq!(ParseOptions::gfm().constructs, Constructs::gfm());
    assert!(format!("{commonmark:?}").contains("inline_code_info: false"));
}

#[test]
fn compile_presets_and_options_keep_safe_defaults() {
    let commonmark = CompileOptions::default();
    assert!(!commonmark.allow_dangerous_html);
    assert!(!commonmark.gfm_tagfilter);
    let gfm = CompileOptions::gfm();
    assert!(!gfm.allow_dangerous_html);
    assert!(gfm.gfm_tagfilter);
    assert_eq!(Options::default().parse.constructs, Constructs::default());
    assert_eq!(Options::gfm().parse.constructs, Constructs::gfm());
}
