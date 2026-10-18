// Released under the MIT License.
// Copyright, 2022, by Bernhard Berger.
// Copyright, 2022-2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

mod test_utils;
use core_markdown::{message, to_html_with_options, Constructs, Options, ParseOptions};
use pretty_assertions::assert_eq;
use test_utils::swc::{parse_esm, parse_expression};

#[test]
fn mdx_swc() -> Result<(), message::Message> {
    let swc = Options {
        parse: ParseOptions {
            constructs: Constructs::mdx(),
            mdx_esm_parse: Some(Box::new(parse_esm)),
            mdx_expression_parse: Some(Box::new(parse_expression)),
            ..Default::default()
        },
        ..Default::default()
    };

    assert_eq!(
        to_html_with_options("{'}'}", &swc)?,
        "",
        "should support JavaScript-aware flow expressions w/ `mdx_expression_parse`"
    );

    assert_eq!(
        to_html_with_options("a {'}'} b", &swc)?,
        "<p>a  b</p>",
        "should support JavaScript-aware text expressions w/ `mdx_expression_parse`"
    );

    assert_eq!(
        to_html_with_options("<a {...a/*}*/} />", &swc)?,
        "",
        "should support JavaScript-aware attribute expressions w/ `mdx_expression_parse`"
    );

    assert_eq!(
        to_html_with_options("<a b={'}'} />", &swc)?,
        "",
        "should support JavaScript-aware attribute value expressions w/ `mdx_expression_parse`"
    );

    assert_eq!(
        to_html_with_options("import a from 'b'\n\nexport {a}\n\n# c", &swc)?,
        "<h1>c</h1>",
        "should support JavaScript-aware ESM w/ `mdx_esm_parse`"
    );

    Ok(())
}
