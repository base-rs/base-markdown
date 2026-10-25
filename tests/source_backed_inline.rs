// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use base_markdown::{
    render_events,
    stream::{EventKind, Text},
    to_html_with_options, Document, EventParser, Options,
};

fn assert_paths(source: &str, options: &Options, expected: &str) {
    assert_eq!(to_html_with_options(source, options), expected);
    let document = Document::parse(source, &options.parse).unwrap();
    let mut output = String::new();
    render_events(source, document.events(), &options.compile, &mut output).unwrap();
    assert_eq!(output, expected);
}

#[test]
fn multiline_raw_values_keep_the_surrounding_text_source_backed() {
    let source = "> before `first\n> second` after\n> plain **bold** line";
    let options = Options::default();
    let events = EventParser::new(source, &options.parse)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let mut raw = 0;
    for event in events {
        match event.kind {
            EventKind::Text(value) => {
                let Text::Source(range) = value else {
                    panic!("unchanged text must retain its original buffer")
                };
                assert_eq!(event.span, range);
                assert!(!source[range].contains('>'));
            }
            EventKind::Code { value, .. } => {
                assert_eq!(value.resolve(source).unwrap(), "first second");
                assert_eq!(&source[event.span], "`first\n> second`");
                raw += 1;
            }
            _ => {}
        }
    }
    assert_eq!(raw, 1);
    assert_paths(
        source,
        &options,
        "<blockquote>\n<p>before <code>first second</code> after\nplain <strong>bold</strong> line</p>\n</blockquote>",
    );
}

#[test]
fn multiline_resources_and_html_share_continuation_padding_rules() {
    let source = "> [a](b \"\n>   title\") <a\n>   title='x'>z</a>";
    let mut options = Options::default();
    options.compile.allow_dangerous_html = true;
    assert_paths(
        source,
        &options,
        "<blockquote>\n<p><a href=\"b\" title=\"\ntitle\">a</a> <a\ntitle='x'>z</a></p>\n</blockquote>",
    );
}

#[test]
fn optional_code_metadata_survives_literal_link_candidate_splitting() {
    let mut options = Options::gfm();
    options.parse.inline_code_info = true;
    assert_paths(
        "Use ruby:`Object.new` and rust:`Vec::new()`.",
        &options,
        "<p>Use <code class=\"language-ruby\">Object.new</code> and <code class=\"language-rust\">Vec::new()</code>.</p>",
    );
}

#[test]
fn escaped_punctuation_bounds_email_postprocessing_without_splitting_normal_text() {
    assert_paths(
        r"<foo\+@bar.example.com> \~foo@example.org",
        &Options::gfm(),
        "<p>&lt;foo+@bar.example.com&gt; ~<a href=\"mailto:foo@example.org\">foo@example.org</a></p>",
    );
}

#[test]
fn footnote_calls_obey_the_independent_label_end_switch() {
    let source = "note[^x]\n\n[^x]: body";
    let mut options = Options::gfm();
    options.parse.constructs.label_end = false;
    assert!(EventParser::new(source, &options.parse)
        .all(|event| !matches!(event.unwrap().kind, EventKind::FootnoteReference(_))));
}

#[test]
fn unresolved_footnote_openers_keep_link_fallback_when_ordinary_links_are_disabled() {
    let mut options = Options::gfm();
    options.parse.constructs.label_start_link = false;
    options.parse.constructs.footnote_definition = false;
    for (source, expected) in [
        ("[^n](url)", "<p><a href=\"url\">^n</a></p>"),
        ("![^n](url)", "<p>!<a href=\"url\">^n</a></p>"),
        ("[^n]\n\n[^n]: /note", "<p><a href=\"/note\">^n</a></p>\n"),
        ("![^n]\n\n[^n]: /note", "<p>!<a href=\"/note\">^n</a></p>\n"),
        ("[n](url)", "<p>[n](url)</p>"),
    ] {
        assert_paths(source, &options, expected);
    }
}

#[test]
fn normalized_titles_decode_entities_and_escapes_after_indentation_is_removed() {
    assert_paths(
        "[a](url \"\n  a&amp;b\\*c\")",
        &Options::default(),
        "<p><a href=\"url\" title=\"\na&amp;b*c\">a</a></p>",
    );
}

#[test]
fn raw_values_replace_nulls_while_preserving_literal_markdown() {
    assert_paths(
        "before `\\*\0&copy;` after",
        &Options::default(),
        "<p>before <code>\\*�&amp;copy;</code> after</p>",
    );
}

#[test]
fn reference_labels_handle_continuation_padding_tabs_and_escaped_brackets() {
    for (source, expected) in [
        (
            "[x][a\n  b]\n\n[a b]: /url",
            "<p><a href=\"/url\">x</a></p>\n",
        ),
        (
            "a\t[x][b\tc]\n\n[b c]: /url",
            "<p>a\t<a href=\"/url\">x</a></p>\n",
        ),
        (
            "[x][a\t\\]]\n\n[a \\]]: /url",
            "<p><a href=\"/url\">x</a></p>\n",
        ),
        (
            "a\t[x][b\t\\]\tc]\n\n[b \\] c]: /url",
            "<p>a\t<a href=\"/url\">x</a></p>\n",
        ),
    ] {
        assert_paths(source, &Options::default(), expected);
    }
}

#[test]
fn task_markers_accept_authored_crlf_and_incomplete_markers_remain_literal() {
    for (source, expected) in [
        ("- [", "<ul>\n<li>[</li>\n</ul>"),
        (
            "- [\r\n] done",
            "<ul>\r\n<li><input type=\"checkbox\" disabled=\"\" /> done</li>\r\n</ul>",
        ),
        (
            "- [\r] done",
            "<ul>\r<li><input type=\"checkbox\" disabled=\"\" /> done</li>\r</ul>",
        ),
    ] {
        assert_paths(source, &Options::gfm(), expected);
    }
}

#[test]
fn table_pipe_quoting_remains_active_when_character_escapes_are_disabled() {
    let mut options = Options::gfm();
    options.parse.constructs.character_escape = false;
    assert_paths(
        "| a\\|b |\n| --- |",
        &options,
        "<table>\n<thead>\n<tr>\n<th>a|b</th>\n</tr>\n</thead>\n</table>",
    );
}
