// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use core_markdown::{
    render_events,
    stream::{Event, EventKind, StreamError, Tag, TagKind, Text},
    Document, EventHtmlRenderer, EventParser, Options,
};
use std::fmt::{self, Write};

fn event(kind: EventKind<'_>) -> Event<'_> {
    Event { kind, span: 0..0 }
}

#[test]
fn escaping_preserves_unicode_and_handles_adjacent_markers_at_scan_boundaries() {
    for (raw, encoded) in [
        ("", ""),
        ("plain é😀", "plain é😀"),
        ("\0", "�"),
        ("&\"<>", "&amp;&quot;&lt;&gt;"),
        ("\0&\"<>\0&", "�&amp;&quot;&lt;&gt;�&amp;"),
    ] {
        for length in [0, 1, 15, 16, 17, 31, 32, 33] {
            let prefix = "a".repeat(length);
            let source = format!("{prefix}{raw}é😀{raw}");
            for html in [false, true] {
                let mut options = Options::default();
                options.compile.allow_dangerous_html = html;
                let text = Text::Source(0..source.len());
                let body = if html {
                    EventKind::Html {
                        value: text,
                        block: false,
                    }
                } else {
                    EventKind::Text(text)
                };
                let events = [
                    event(EventKind::Start(Tag::Document(None))),
                    event(body),
                    event(EventKind::Text(Text::Literal(""))),
                    event(EventKind::End(TagKind::Document)),
                ];
                let mut output = String::new();
                render_events(
                    &source,
                    IntoIterator::into_iter(events).map(Ok),
                    &options.compile,
                    &mut output,
                )
                .unwrap();
                let expected = if html {
                    source.replace('\0', "�")
                } else {
                    format!("{prefix}{encoded}é😀{encoded}")
                };
                assert_eq!(output, expected, "{source:?}, html={html}");
            }
        }
    }
}

#[test]
fn escaping_uses_heading_capture_before_footnote_capture() {
    let source = "# Main &\"<>\0\n\nNote[^n].\n\n[^n]: # Nested &\"<>\0\n\n    Body &\"<>\0\n";
    let mut options = Options::gfm();
    options.compile.heading_ids = true;
    let expected = core_markdown::to_html_with_options(source, &options).unwrap();
    let marker = "&amp;&quot;&lt;&gt;�";
    let footnotes = expected.find("<section data-footnotes").unwrap();
    assert!(expected[..footnotes].contains(&format!("Main {marker}</h1>")));
    assert!(expected[footnotes..].contains(&format!("Nested {marker}</h1>")));
    assert!(expected[footnotes..].contains(&format!("Body {marker}")));
    let ast = Document::parse(source, &options.parse).unwrap();
    let mut output = String::new();
    render_events(source, ast.events(), &options.compile, &mut output).unwrap();
    assert_eq!(output, expected);
    output.clear();
    render_events(
        source,
        EventParser::new(source, &options.parse).unwrap(),
        &options.compile,
        &mut output,
    )
    .unwrap();
    assert_eq!(output, expected);
}

#[test]
fn failed_escape_writes_retain_the_last_successfully_written_byte() {
    struct Writer {
        output: String,
        fail_on: Option<&'static str>,
    }
    impl Write for Writer {
        fn write_str(&mut self, value: &str) -> fmt::Result {
            if Some(value) == self.fail_on {
                self.fail_on = None;
                return Err(fmt::Error);
            }
            self.output.push_str(value);
            Ok(())
        }
    }
    for (value, fail_on, expected) in [
        ("prefix\n&tail", "&amp;", "prefix\n<hr />"),
        ("prefix&tail", "&amp;", "prefix\n<hr />"),
        ("&tail", "tail", "&amp;\n<hr />"),
        ("&\n<tail", "\n", "&amp;\n<hr />"),
        ("<tail", "&lt;", "<hr />"),
    ] {
        let mut writer = Writer {
            output: String::new(),
            fail_on: Some(fail_on),
        };
        let options = Options::default();
        let mut renderer = EventHtmlRenderer::new("", &options.compile, &mut writer);
        renderer
            .event(event(EventKind::Start(Tag::Document(None))))
            .unwrap();
        assert!(matches!(
            renderer.event(event(EventKind::Text(Text::Literal(value)))),
            Err(StreamError::Write(_))
        ));
        renderer.event(event(EventKind::Rule)).unwrap();
        renderer
            .event(event(EventKind::End(TagKind::Document)))
            .unwrap();
        assert_eq!(writer.output, expected, "{value:?}, fail_on={fail_on:?}");
    }
}
