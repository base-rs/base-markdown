// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use base_markdown::{
    render_events,
    stream::{EventKind, Text},
    Document, EventParser, Options,
};

#[test]
fn references_keep_their_meaning_across_regions_and_rendering_paths() {
    for (source, expected) in [
        (
            "&NotEqualTilde; &#x1F600; &#0; &#55296; &#x110000;",
            "<p>≂̸ 😀 � � �</p>",
        ),
        (
            "&#8; &#9; &#10; &#11; &#12; &#13; &#14; &#127; &#159; &#160;",
            "<p>� \t \n � \u{c} \r � � � \u{a0}</p>",
        ),
        (
            "&amp\n; &#x1F600\n;",
            "<p>&amp;amp\n; &amp;#x1F600\n;</p>",
        ),
        (
            "[x](a&amp;b \"t&#65; &NotEqualTilde;\")",
            "<p><a href=\"a&amp;b\" title=\"tA ≂̸\">x</a></p>",
        ),
        (
            "> &amp;\n> &#65;\n\n&amp; <b>&NotEqualTilde;</b> &#x42;\n\n&#67;",
            "<blockquote>\n<p>&amp;\nA</p>\n</blockquote>\n<p>&amp; &lt;b&gt;≂̸&lt;/b&gt; B</p>\n<p>C</p>",
        ),
        (
            "[x][id]\n\n[id]: a&amp;b \"t&#65; &NotEqualTilde;\"",
            "<p><a href=\"a&amp;b\" title=\"tA ≂̸\">x</a></p>\n",
        ),
    ] {
        for mut options in [Options::default(), Options::gfm()] {
            // Unrelated code metadata options preserve the same semantic values.
            for configured in [false, true] {
                options.parse.inline_code_info = configured;
                assert_eq!(
                    base_markdown::to_html_with_options(source, &options),
                    expected,
                    "source={source:?}, configured={configured}"
                );
                let mut checked = String::new();
                render_events(
                    source,
                    EventParser::new(source, &options.parse),
                    &options.compile,
                    &mut checked,
                )
                .unwrap();
                assert_eq!(checked, expected);
                let document = Document::parse(source, &options.parse).unwrap();
                let mut arena = String::new();
                render_events(source, document.events(), &options.compile, &mut arena).unwrap();
                assert_eq!(arena, expected);
            }
        }
    }
}

#[test]
fn a_pulled_numeric_value_survives_dropping_unconsumed_entities() {
    let source = "&#65; &NotEqualTilde; &#66;";
    let options = Options::default();
    let mut parser = EventParser::new(source, &options.parse);
    let first = parser
        .find_map(|event| {
            let event = event.unwrap();
            matches!(&event.kind, EventKind::Text(_)).then_some(event)
        })
        .unwrap();
    drop(parser);
    assert_eq!(first.span, 0..5);
    let EventKind::Text(Text::Value(value)) = first.kind else {
        panic!("numeric references produce an independently owned semantic value");
    };
    assert_eq!(value, "A");
}
