// Released under the MIT License.
// Copyright, 2022, by Bernhard Berger.
// Copyright, 2022-2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

mod support;
use base_markdown::{
    message, to_html_with_options, CompileOptions, Constructs, Options, ParseOptions,
};
use pretty_assertions::assert_eq;
use support::fixture::parse_fixture;
use support::fixture::Position;
use support::fixture::{Node, Paragraph, Root, Text};

#[test]
fn character_reference() -> Result<(), message::Message> {
    assert_eq!(
        to_html_with_options(
            "&nbsp; &amp; &copy; &AElig; &Dcaron;\n&frac34; &HilbertSpace; &DifferentialD;\n&ClockwiseContourIntegral; &ngE;", &Options::commonmark()),
        "<p>\u{a0} &amp; © Æ Ď\n¾ ℋ ⅆ\n∲ ≧̸</p>",
        "should support named character references"
    );

    assert_eq!(
        to_html_with_options("&#35; &#1234; &#992; &#0;", &Options::commonmark()),
        "<p># Ӓ Ϡ �</p>",
        "should support decimal character references"
    );

    assert_eq!(
        to_html_with_options("&#X22; &#XD06; &#xcab;", &Options::commonmark()),
        "<p>&quot; ആ ಫ</p>",
        "should support hexadecimal character references"
    );

    assert_eq!(
      to_html_with_options(
        "&nbsp &x; &#; &#x;\n&#987654321;\n&#abcdef0;\n&ThisIsNotDefined; &hi?;", &Options::commonmark()),
      "<p>&amp;nbsp &amp;x; &amp;#; &amp;#x;\n&amp;#987654321;\n&amp;#abcdef0;\n&amp;ThisIsNotDefined; &amp;hi?;</p>",
      "should not support other things that look like character references"
    );

    assert_eq!(
        to_html_with_options("&copy", &Options::commonmark()),
        "<p>&amp;copy</p>",
        "should not support character references w/o semicolon"
    );

    assert_eq!(
        to_html_with_options("&MadeUpEntity;", &Options::commonmark()),
        "<p>&amp;MadeUpEntity;</p>",
        "should not support unknown named character references"
    );

    assert_eq!(
        to_html_with_options(
            "<a href=\"&ouml;&ouml;.html\">",
            &Options {
                compile: CompileOptions {
                    allow_dangerous_html: true,
                    allow_dangerous_protocol: true,
                    ..Default::default()
                },
                ..Options::commonmark()
            }
        ),
        "<a href=\"&ouml;&ouml;.html\">",
        "should not care about character references in html"
    );

    assert_eq!(
        to_html_with_options(
            "[foo](/f&ouml;&ouml; \"f&ouml;&ouml;\")",
            &Options::commonmark()
        ),
        "<p><a href=\"/f%C3%B6%C3%B6\" title=\"föö\">foo</a></p>",
        "should support character references in resource URLs and titles"
    );

    assert_eq!(
        to_html_with_options(
            "[foo]: /f&ouml;&ouml; \"f&ouml;&ouml;\"\n\n[foo]",
            &Options::commonmark()
        ),
        "<p><a href=\"/f%C3%B6%C3%B6\" title=\"föö\">foo</a></p>",
        "should support character references in definition URLs and titles"
    );

    assert_eq!(
        to_html_with_options("``` f&ouml;&ouml;\nfoo\n```", &Options::commonmark()),
        "<pre><code class=\"language-föö\">foo\n</code></pre>",
        "should support character references in code language"
    );

    assert_eq!(
        to_html_with_options("`f&ouml;&ouml;`", &Options::commonmark()),
        "<p><code>f&amp;ouml;&amp;ouml;</code></p>",
        "should not support character references in text code"
    );

    assert_eq!(
        to_html_with_options("    f&ouml;f&ouml;", &Options::commonmark()),
        "<pre><code>f&amp;ouml;f&amp;ouml;\n</code></pre>",
        "should not support character references in indented code"
    );

    assert_eq!(
        to_html_with_options("&#42;foo&#42;\n*foo*", &Options::commonmark()),
        "<p>*foo*\n<em>foo</em></p>",
        "should not support character references as construct markers (1)"
    );

    assert_eq!(
        to_html_with_options("&#42; foo\n\n* foo", &Options::commonmark()),
        "<p>* foo</p>\n<ul>\n<li>foo</li>\n</ul>",
        "should not support character references as construct markers (2)"
    );

    assert_eq!(
        to_html_with_options("[a](url &quot;tit&quot;)", &Options::commonmark()),
        "<p>[a](url &quot;tit&quot;)</p>",
        "should not support character references as construct markers (3)"
    );

    assert_eq!(
        to_html_with_options("foo&#10;&#10;bar", &Options::commonmark()),
        "<p>foo\n\nbar</p>",
        "should not support character references as whitespace (1)"
    );

    assert_eq!(
        to_html_with_options("&#9;foo", &Options::commonmark()),
        "<p>\tfoo</p>",
        "should not support character references as whitespace (2)"
    );

    // Extra:
    assert_eq!(
        to_html_with_options("&CounterClockwiseContourIntegral;", &Options::commonmark()),
        "<p>∳</p>",
        "should support the longest possible named character reference"
    );

    assert_eq!(
        to_html_with_options("&#xff9999;", &Options::commonmark()),
        "<p>�</p>",
        "should “support” a longest possible hexadecimal character reference"
    );

    assert_eq!(
        to_html_with_options("&#9999999;", &Options::commonmark()),
        "<p>�</p>",
        "should “support” a longest possible decimal character reference"
    );

    assert_eq!(
        to_html_with_options("&CounterClockwiseContourIntegrali;", &Options::commonmark()),
        "<p>&amp;CounterClockwiseContourIntegrali;</p>",
        "should not support the longest possible named character reference"
    );

    assert_eq!(
        to_html_with_options("&#xff99999;", &Options::commonmark()),
        "<p>&amp;#xff99999;</p>",
        "should not support a longest possible hexadecimal character reference"
    );

    assert_eq!(
        to_html_with_options("&#99999999;", &Options::commonmark()),
        "<p>&amp;#99999999;</p>",
        "should not support a longest possible decimal character reference"
    );

    assert_eq!(
        to_html_with_options("&-;", &Options::commonmark()),
        "<p>&amp;-;</p>",
        "should not support the other characters after `&`"
    );

    assert_eq!(
        to_html_with_options("&#-;", &Options::commonmark()),
        "<p>&amp;#-;</p>",
        "should not support the other characters after `#`"
    );

    assert_eq!(
        to_html_with_options("&#x-;", &Options::commonmark()),
        "<p>&amp;#x-;</p>",
        "should not support the other characters after `#x`"
    );

    assert_eq!(
        to_html_with_options("&lt-;", &Options::commonmark()),
        "<p>&amp;lt-;</p>",
        "should not support the other characters inside a name"
    );

    assert_eq!(
        to_html_with_options("&#9-;", &Options::commonmark()),
        "<p>&amp;#9-;</p>",
        "should not support the other characters inside a demical"
    );

    assert_eq!(
        to_html_with_options("&#x9-;", &Options::commonmark()),
        "<p>&amp;#x9-;</p>",
        "should not support the other characters inside a hexademical"
    );

    assert_eq!(
        to_html_with_options(
            "&amp;",
            &Options {
                parse: ParseOptions {
                    constructs: Constructs {
                        character_reference: false,
                        ..Constructs::commonmark()
                    },
                    ..ParseOptions::commonmark()
                },
                ..Options::commonmark()
            }
        ),
        "<p>&amp;amp;</p>",
        "should support turning off character references"
    );

    assert_eq!(
        parse_fixture("&nbsp; &amp; &copy; &AElig; &Dcaron;\n&frac34; &HilbertSpace; &DifferentialD;\n&ClockwiseContourIntegral; &ngE;\n&#35; &#1234; &#992; &#0;\n&#X22; &#XD06; &#xcab;", &Default::default())?,
        Node::Root(Root {
            children: vec![Node::Paragraph(Paragraph {
                children: vec![Node::Text(Text {
                    value: "\u{a0} & © Æ Ď\n¾ ℋ ⅆ\n∲ ≧̸\n# Ӓ Ϡ �\n\" ആ ಫ".into(),
                    position: Some(Position::new(1, 1, 0, 5, 23, 158))
                }),],
                position: Some(Position::new(1, 1, 0, 5, 23, 158))
            })],
            position: Some(Position::new(1, 1, 0, 5, 23, 158))
        }),
        "should support character references as `Text`s in mdast"
    );

    Ok(())
}
