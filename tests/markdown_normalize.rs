// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use base_markdown::{normalize_markdown, normalize_markdown_with_options, ParseOptions};

#[test]
fn unwraps_soft_source_line_breaks() {
    let normalized = normalize_markdown("A paragraph\nwith a soft break.\n").unwrap();

    assert_eq!(normalized, "A paragraph with a soft break.\n");
}

#[test]
fn preserves_hard_breaks_code_content_and_block_boundaries() {
    let source = "First paragraph\ncontinues.\n\nHard break  \ncontinues.\n\n```text\ncode line one\ncode line two\n```\n\nLast paragraph.\n";

    let normalized = normalize_markdown(source).unwrap();

    assert_eq!(
        normalized,
        "First paragraph continues.\n\nHard break\\\ncontinues.\n\n```text\ncode line one\ncode line two\n```\n\nLast paragraph.\n"
    );
}

#[test]
fn normalization_is_idempotent() {
    let first = normalize_markdown("A paragraph\nwith a soft break.\n").unwrap();
    let second = normalize_markdown(&first).unwrap();

    assert_eq!(second, first);
}

#[test]
fn normalizes_unordered_lists_to_hyphen_markers() {
    let normalized = normalize_markdown("* first\n* second\n").unwrap();

    assert_eq!(normalized, "- first\n- second\n");
}

#[test]
fn preserves_front_matter_and_unwraps_nested_phrasing() {
    let source = "---\ntitle: Example\n---\n\nA *wrapped\nemphasis* paragraph.\n";

    let normalized = normalize_markdown(source).unwrap();

    assert_eq!(
        normalized,
        "---\ntitle: Example\n---\n\nA *wrapped emphasis* paragraph.\n"
    );
}

#[test]
fn preserves_toml_front_matter() {
    let source = "+++\ntitle = \"Example\"\n+++\n\nA paragraph.\n";

    let normalized = normalize_markdown(source).unwrap();

    assert_eq!(normalized, source);
}

#[test]
fn normalizes_crlf_soft_breaks_to_lf_output() {
    let normalized = normalize_markdown("A paragraph\r\nwith a soft break.\r\n").unwrap();

    assert_eq!(normalized, "A paragraph with a soft break.\n");
}

#[test]
fn preserves_inline_code_and_math() {
    let source = "Inline `code` and inline math $x + y$.\n";

    let normalized = normalize_markdown(source).unwrap();

    assert_eq!(normalized, source);
}

#[test]
fn preserves_gfm_tables_and_normalizes_task_list_markers() {
    let source = "| key | value |\n| --- | --- |\n| name | Ada |\n\n~~removed~~ and a task:\n\n- [x] done\n- [ ] pending\n\nA claim[^note].\n\n[^note]: A note.\n";

    let normalized = normalize_markdown(source).unwrap();

    assert_eq!(
        normalized,
        "| key | value |\n| --- | --- |\n| name | Ada |\n\n~~removed~~ and a task:\n\n- [x] done\n- [ ] pending\n\nA claim[^note].\n\n[^note]: A note.\n"
    );
}

#[test]
fn honors_independently_disabled_constructs() {
    let mut options = ParseOptions::gfm();
    options.constructs.strikethrough = false;
    let normalized = normalize_markdown_with_options("~~ordinary~~\n", &options).unwrap();
    assert_eq!(
        base_markdown::to_html_with_options(
            &normalized,
            &base_markdown::Options {
                parse: options,
                ..base_markdown::Options::default()
            }
        ),
        "<p>~~ordinary~~</p>\n"
    );
}

#[test]
fn normalizes_lone_carriage_returns_and_empty_documents() {
    assert_eq!(
        normalize_markdown("A\rB\r\r```\nx\r\n```\r").unwrap(),
        "A B\n\n```\nx\n```\n"
    );
    assert_eq!(normalize_markdown("").unwrap(), "");
}

#[test]
fn normalization_is_idempotent_for_mixed_constructs() {
    let source = "---\ntitle: Example\n---\n\n* item one\n  continues\n* item <!-- marker -->\n\n> Quoted\n> text\n\n$$\nx\n$$\n\n| a |\n|---|\n| b |\n";
    let first = normalize_markdown(source).unwrap();

    assert_eq!(normalize_markdown(&first).unwrap(), first);
}

#[test]
fn normalization_reaches_a_fixed_point_in_one_pass() {
    for source in [
        "- [x] a\n  - b\n  - c\n",
        "- [ ] a\n\n  more para\n",
        "- [ ] a\n\n  ```\n  x\n  ```\n",
        "- [ ] task\n\n      code\n",
        "1. [x] a\n   b\n\n   c\n",
        "Line\nwrap\nSetext\n---\n",
        "Hard\\\nbreak\n---\n",
        "Title\n===\n\nNext\n",
        "- a\n  - b\n\n  More.\n",
        "1. a\n   - b\n\n   More.\n",
        "- a\n  1. b\n\n  More.\n",
        "> - a\n>   - b\n>\n>   More.\n",
        "- [x] a\n  - [ ] b\n\n        code\n",
        "- a\n  - b\n\n  More\n  ---\n",
        "- a\n  - b\n  ```\n  x\n  ```\n",
        "- a\n  - b\n\n  | a |\n  |---|\n  | 1 |\n",
        "- a\n  - b\n\n  [r]: https://x\n\n  [x][r]\n",
        "> 1. [x] a\n>    - b\n>\n>    | a |\n>    |---|\n>    | 1 |\n",
    ] {
        let first = normalize_markdown(source).unwrap();

        assert_eq!(normalize_markdown(&first).unwrap(), first, "{source:?}");
    }
}

#[test]
fn task_item_content_aligns_with_the_list_marker() {
    assert_eq!(
        normalize_markdown("- [x] a\n  - b\n- [ ] c\n\n  More.\n").unwrap(),
        "- [x] a\n  - b\n\n- [ ] c\n\n  More.\n"
    );
    assert_eq!(
        normalize_markdown("Wrapped\nheading\n---\n").unwrap(),
        "## Wrapped heading\n"
    );
}

#[test]
fn normalize_parse_options_enable_gfm_frontmatter_and_math() {
    let options = ParseOptions::normalize();

    assert!(options.constructs.table);
    assert!(options.constructs.frontmatter);
    assert!(options.constructs.math_flow);
    assert!(options.constructs.math_text);
    assert_eq!(
        normalize_markdown_with_options("$$\nx\n$$\n", &options).unwrap(),
        normalize_markdown("$$\nx\n$$\n").unwrap()
    );
}

#[test]
fn paragraphs_after_nested_lists_stay_separate() {
    for source in [
        "- a\n  - b\n\n  More.\n",
        "- a\n  - b\n  ```\n  x\n  ```\n",
        "- a\n  - b\n\n  | a |\n  |---|\n  | 1 |\n",
        "- a\n  - b\n\n  [r]: https://x\n\n  [x][r]\n",
    ] {
        let normalized = normalize_markdown(source).unwrap();

        let html = |markdown: &str| {
            base_markdown::to_html_with_options(markdown, &base_markdown::Options::gfm())
        };
        assert_eq!(html(&normalized), html(source), "{source:?}");
    }
}
