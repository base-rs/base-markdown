// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Image text is written as phrasing, like link text, so every shape the parser gives it, such as
//! footnote calls, links and images in it, reads back as itself (#190).

mod support;
use base_markdown::{normalize_markdown, to_html_with_options, to_markdown, Document, Options};
use pretty_assertions::assert_eq;
use support::fixture::canonical;

/// Parse `source`, normalize it and write it, then check that what is written reads back as the
/// same tree, renders the same HTML and is written again the same.
fn round_trip(source: &str) -> String {
    let options = Options::gfm();
    let mut document = Document::parse(source, &options.parse).unwrap();
    document.normalize();
    let markdown = to_markdown(&document).unwrap();
    let mut reparsed = Document::parse(markdown.as_str(), &options.parse).unwrap();
    assert_eq!(canonical(&reparsed), canonical(&document), "{source:?}");
    let html = |value: &str| to_html_with_options(value, &options);
    assert_eq!(
        html(&markdown).trim_end(),
        html(source).trim_end(),
        "{source:?}"
    );
    reparsed.normalize();
    assert_eq!(to_markdown(&reparsed).unwrap(), markdown, "{source:?}");
    markdown
}

#[test]
fn footnote_calls_in_image_text_are_written() {
    for (source, expected) in [
        // The issue's reproducer, which was written `![see ](/u)`.
        (
            "![see [^1]](/u)\n\n[^1]: x\n",
            "![see [^1]](/u)\n\n[^1]: x\n",
        ),
        ("![[^1]](/u)\n\n[^1]: x\n", "![[^1]](/u)\n\n[^1]: x\n"),
        (
            "![a [^1] b [^1]][r]\n\n[r]: /u\n[^1]: x\n",
            "![a [^1] b [^1]][r]\n\n[r]: /u\n\n[^1]: x\n",
        ),
        (
            "| ![a [^1]](/i) |\n| - |\n\n[^1]: x\n",
            "| ![a [^1]](/i) |\n| --- |\n\n[^1]: x\n",
        ),
        (
            "# ![a [^1]](/i)\n\n[^1]: x\n",
            "# ![a [^1]](/i)\n\n[^1]: x\n",
        ),
        // A `:` after a call at the start of a line in image text would start a definition.
        (
            "![a  \n[^1]\\: x](/i)\n\n[^1]: n\n",
            "![a\\\n[^1]\\: x](/i)\n\n[^1]: n\n",
        ),
        // Without a definition, `[^1]` is text, and stays text.
        ("![see [^1]](/u)\n", "![see \\[^1\\]](/u)\n"),
    ] {
        assert_eq!(round_trip(source), expected, "{source:?}");
    }
}

#[test]
fn links_and_images_in_image_text_are_written() {
    for (source, expected) in [
        // The issue's reproducer, which was written `![a b](/i)`.
        ("![a [b](/v)](/i)", "![a [b](/v)](/i)\n"),
        ("![a ![b](/v)](/i)", "![a ![b](/v)](/i)\n"),
        // `[` in a destination in image text is not escaped, as in a paragraph.
        ("![a [b](/v[x)](/i)", "![a [b](/v[x)](/i)\n"),
        ("![[a](/b)[c](/d)](/i)", "![[a](/b)[c](/d)](/i)\n"),
        ("![![![a](b)](c)](d)", "![![![a](b)](c)](d)\n"),
        (
            "![a *b ![c *d*](e) f* g](u)",
            "![a *b ![c *d*](e) f* g](u)\n",
        ),
        (
            "![a [b](/v)][r]\n\n[r]: /i\n",
            "![a [b](/v)][r]\n\n[r]: /i\n",
        ),
        // References in image text keep their form.
        (
            "![a [b][r]](/i)\n\n[r]: /v\n",
            "![a [b][r]](/i)\n\n[r]: /v\n",
        ),
        ("![a [r][]](/i)\n\n[r]: /v\n", "![a [r][]](/i)\n\n[r]: /v\n"),
        ("![a [r]](/i)\n\n[r]: /v\n", "![a [r]](/i)\n\n[r]: /v\n"),
        ("![a ![r]](/i)\n\n[r]: /v\n", "![a ![r]](/i)\n\n[r]: /v\n"),
        // A `(` after a shortcut reference would make it a resource.
        (
            "![a [r]\\(x)](u)\n\n[r]: /a\n",
            "![a [r]\\(x)](u)\n\n[r]: /a\n",
        ),
        (
            "[a ![r]\\(x)](u)\n\n[r]: /a\n",
            "[a ![r]\\(x)](u)\n\n[r]: /a\n",
        ),
        // Autolinks, and literals, which form in image text outside link text.
        ("![a <http://x.y> b](/i)", "![a <http://x.y> b](/i)\n"),
        ("![<x@y.z>](/i)", "![<x@y.z>](/i)\n"),
        ("![www.x.com](/i)", "![www.x.com](/i)\n"),
        ("![a www.x.com b](/i)", "![a www.x.com b](/i)\n"),
        ("![a https://x.y/z](/i)", "![a https://x.y/z](/i)\n"),
        ("![a x@y.z](/i)", "![a x@y.z](/i)\n"),
        // Not bare where what follows would change where it ends, as in a paragraph.
        (
            "![a www.x.com.](/i)",
            "![a [www.x.com](https://www.x.com).](/i)\n",
        ),
        ("[![<http://x.y>](/i)](/l)", "[![<http://x.y>](/i)](/l)\n"),
        // No literal forms in image text inside link text.
        ("[![www.x.com](/i)](/l)", "[![www.x.com](/i)](/l)\n"),
    ] {
        assert_eq!(round_trip(source), expected, "{source:?}");
    }
}

#[test]
fn other_phrasing_in_image_text_is_written() {
    for (source, expected) in [
        ("![*a* **b**](/i)", "![*a* **b**](/i)\n"),
        ("![~~d~~](/i)", "![~~d~~](/i)\n"),
        ("![`c`](/i)", "![`c`](/i)\n"),
        ("![<b>](/i)", "![<b>](/i)\n"),
        ("![a  \nb](/i)", "![a\\\nb](/i)\n"),
        ("![a\nb](/i)", "![a\nb](/i)\n"),
        ("![a\nb](u)\n===", "![a\nb](u)\n=====\n"),
        // `![^` starts no image.
        ("![\\^a](/i)", "![\\^a](/i)\n"),
        ("![\\^a](/i)\n\n[^a]: x\n", "![\\^a](/i)\n\n[^a]: x\n"),
    ] {
        assert_eq!(round_trip(source), expected, "{source:?}");
    }
}

#[test]
fn literals_before_the_end_of_image_text_stay_bare() {
    // Image text keeps spaces at its end, so a literal before them is at a boundary, and one before
    // a soft break, which unwrapping writes as a space, stays bare when it is written again.
    for (source, expected) in [
        ("![x@y.z ](u)", "![x@y.z ](u)\n"),
        ("![www.x.com\t](u)", "![www.x.com\t](u)\n"),
        ("![a\nx@y.z ](u)", "![a\nx@y.z ](u)\n"),
    ] {
        assert_eq!(round_trip(source), expected, "{source:?}");
    }
    for (source, expected) in [
        ("![x@y.z\n](u)", "![x@y.z ](u)\n"),
        ("![ www.x.com\n](u)", "![ www.x.com ](u)\n"),
        ("> ![https://a.b\n](u)", "> ![https://a.b ](u)\n"),
    ] {
        let markdown = normalize_markdown(source).unwrap();
        assert_eq!(markdown, expected, "{source:?}");
        assert_eq!(
            normalize_markdown(&markdown).unwrap(),
            markdown,
            "{source:?}"
        );
    }
}

#[test]
fn image_text_html_is_its_plain_text() {
    // HTML alt text is the plain text of the image text, whatever it holds.
    let html = to_html_with_options(
        "![a [b](/v) ![c](/w) *d* [^1]](/i)\n\n[^1]: x\n",
        &Options::gfm(),
    );
    assert!(
        html.starts_with("<p><img src=\"/i\" alt=\"a b c d \" /></p>"),
        "{}",
        html
    );
}

#[test]
fn images_nested_in_images_read_back_up_to_the_limit() {
    // The paragraph and each image count one level, and the text in the innermost one more.
    let nested = |depth: usize| format!("{}a{}", "![".repeat(depth), "](u)".repeat(depth));
    let source = nested(126);
    assert_eq!(round_trip(&source), format!("{source}\n"));
    // Deeper, it is an error like any content nested too deep, not a panic.
    let error = normalize_markdown(&nested(200)).unwrap_err();
    assert_eq!(error.rule_id, "nesting-too-deep");
}
