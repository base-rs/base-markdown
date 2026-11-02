// Released under the MIT License.
// Copyright, 2022, by Bernhard Berger.
// Copyright, 2022-2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

use base_markdown::{to_html_with_options, CompileOptions, Options};
use pretty_assertions::assert_eq;

#[test]
fn tabs_flow() {
    let danger = &Options {
        compile: CompileOptions {
            allow_dangerous_html: true,
            allow_dangerous_protocol: true,
            ..Default::default()
        },
        ..Options::commonmark()
    };

    assert_eq!(
        to_html_with_options("    x", &Options::commonmark()),
        "<pre><code>x\n</code></pre>",
        "should support a 4*SP to start code"
    );

    assert_eq!(
        to_html_with_options("\tx", &Options::commonmark()),
        "<pre><code>x\n</code></pre>",
        "should support a HT to start code"
    );

    assert_eq!(
        to_html_with_options(" \tx", &Options::commonmark()),
        "<pre><code>x\n</code></pre>",
        "should support a SP + HT to start code"
    );

    assert_eq!(
        to_html_with_options("  \tx", &Options::commonmark()),
        "<pre><code>x\n</code></pre>",
        "should support a 2*SP + HT to start code"
    );

    assert_eq!(
        to_html_with_options("   \tx", &Options::commonmark()),
        "<pre><code>x\n</code></pre>",
        "should support a 3*SP + HT to start code"
    );

    assert_eq!(
        to_html_with_options("    \tx", &Options::commonmark()),
        "<pre><code>\tx\n</code></pre>",
        "should support a 4*SP to start code, and leave the next HT as code data"
    );

    assert_eq!(
        to_html_with_options("   \t# x", &Options::commonmark()),
        "<pre><code># x\n</code></pre>",
        "should not support a 3*SP + HT to start an ATX heading"
    );

    assert_eq!(
        to_html_with_options("   \t> x", &Options::commonmark()),
        "<pre><code>&gt; x\n</code></pre>",
        "should not support a 3*SP + HT to start a block quote"
    );

    assert_eq!(
        to_html_with_options("   \t- x", &Options::commonmark()),
        "<pre><code>- x\n</code></pre>",
        "should not support a 3*SP + HT to start a list item"
    );

    assert_eq!(
        to_html_with_options("   \t---", &Options::commonmark()),
        "<pre><code>---\n</code></pre>",
        "should not support a 3*SP + HT to start a thematic break"
    );

    assert_eq!(
        to_html_with_options("   \t```", &Options::commonmark()),
        "<pre><code>```\n</code></pre>",
        "should not support a 3*SP + HT to start a fenced code"
    );

    assert_eq!(
        to_html_with_options("   \t<div>", &Options::commonmark()),
        "<pre><code>&lt;div&gt;\n</code></pre>",
        "should not support a 3*SP + HT to start HTML"
    );

    assert_eq!(
        to_html_with_options("#\tx\t#\t", &Options::commonmark()),
        "<h1>x</h1>",
        "should support tabs around ATX heading sequences"
    );

    assert_eq!(
        to_html_with_options("#\t\tx\t\t#\t\t", &Options::commonmark()),
        "<h1>x</h1>",
        "should support arbitrary tabs around ATX heading sequences"
    );

    assert_eq!(
        to_html_with_options("```\tx\ty\t\n```\t", &Options::commonmark()),
        "<pre><code class=\"language-x\"></code></pre>",
        "should support tabs around fenced code fences, info, and meta"
    );

    assert_eq!(
        to_html_with_options("```\t\tx\t\ty\t\t\n```\t\t", &Options::commonmark()),
        "<pre><code class=\"language-x\"></code></pre>",
        "should support arbitrary tabs around fenced code fences, info, and meta"
    );

    assert_eq!(
        to_html_with_options("```x\n\t```", &Options::commonmark()),
        "<pre><code class=\"language-x\">\t```\n</code></pre>\n",
        "should not support tabs before fenced code closing fences"
    );

    assert_eq!(
        to_html_with_options("<x\ty\tz\t=\t\"\tx\t\">", danger),
        "<x\ty\tz\t=\t\"\tx\t\">",
        "should support tabs in HTML (if whitespace is allowed)"
    );

    assert_eq!(
        to_html_with_options("*\t*\t*\t", &Options::commonmark()),
        "<hr />",
        "should support tabs in thematic breaks"
    );

    assert_eq!(
        to_html_with_options("*\t\t*\t\t*\t\t", &Options::commonmark()),
        "<hr />",
        "should support arbitrary tabs in thematic breaks"
    );
}

#[test]
fn tabs_text() {
    assert_eq!(
        to_html_with_options("<http:\t>", &Options::commonmark()),
        "<p>&lt;http:\t&gt;</p>",
        "should not support a tab to start an autolink w/ protocol’s rest"
    );

    assert_eq!(
        to_html_with_options("<http:x\t>", &Options::commonmark()),
        "<p>&lt;http:x\t&gt;</p>",
        "should not support a tab in an autolink w/ protocol’s rest"
    );

    assert_eq!(
        to_html_with_options("<example\t@x.com>", &Options::commonmark()),
        "<p>&lt;example\t@x.com&gt;</p>",
        "should not support a tab in an email autolink’s local part"
    );

    assert_eq!(
        to_html_with_options("<example@x\ty.com>", &Options::commonmark()),
        "<p>&lt;example@x\ty.com&gt;</p>",
        "should not support a tab in an email autolink’s label"
    );

    assert_eq!(
        to_html_with_options("\\\tx", &Options::commonmark()),
        "<p>\\\tx</p>",
        "should not support character escaped tab"
    );

    assert_eq!(
        to_html_with_options("&#9;", &Options::commonmark()),
        "<p>\t</p>",
        "should support character reference resolving to a tab"
    );

    assert_eq!(
        to_html_with_options("`\tx`", &Options::commonmark()),
        "<p><code>\tx</code></p>",
        "should support a tab starting code"
    );

    assert_eq!(
        to_html_with_options("`x\t`", &Options::commonmark()),
        "<p><code>x\t</code></p>",
        "should support a tab ending code"
    );

    assert_eq!(
        to_html_with_options("`\tx\t`", &Options::commonmark()),
        "<p><code>\tx\t</code></p>",
        "should support tabs around code"
    );

    assert_eq!(
        to_html_with_options("`\tx `", &Options::commonmark()),
        "<p><code>\tx </code></p>",
        "should support a tab starting, and a space ending, code"
    );

    assert_eq!(
        to_html_with_options("` x\t`", &Options::commonmark()),
        "<p><code> x\t</code></p>",
        "should support a space starting, and a tab ending, code"
    );

    // Note: CM does not strip it in this case.
    // However, that should be a bug there: makes more sense to remove it like
    // trailing spaces.
    assert_eq!(
        to_html_with_options("x\t\ny", &Options::commonmark()),
        "<p>x\ny</p>",
        "should support a trailing tab at a line ending in a paragraph"
    );

    assert_eq!(
        to_html_with_options("x\n\ty", &Options::commonmark()),
        "<p>x\ny</p>",
        "should support an initial tab after a line ending in a paragraph"
    );

    assert_eq!(
        to_html_with_options("x[\ty](z)", &Options::commonmark()),
        "<p>x<a href=\"z\">\ty</a></p>",
        "should support an initial tab in a link label"
    );

    assert_eq!(
        to_html_with_options("x[y\t](z)", &Options::commonmark()),
        "<p>x<a href=\"z\">y\t</a></p>",
        "should support a final tab in a link label"
    );

    assert_eq!(
        to_html_with_options("[x\ty](z)", &Options::commonmark()),
        "<p><a href=\"z\">x\ty</a></p>",
        "should support a tab in a link label"
    );

    // Note: CM.js bug, see: <https://github.com/commonmark/commonmark.js/issues/191>
    assert_eq!(
        to_html_with_options("[x](\ty)", &Options::commonmark()),
        "<p><a href=\"y\">x</a></p>",
        "should support a tab starting a link resource"
    );

    assert_eq!(
        to_html_with_options("[x](y\t)", &Options::commonmark()),
        "<p><a href=\"y\">x</a></p>",
        "should support a tab ending a link resource"
    );

    assert_eq!(
        to_html_with_options("[x](y\t\"z\")", &Options::commonmark()),
        "<p><a href=\"y\" title=\"z\">x</a></p>",
        "should support a tab between a link destination and title"
    );
}

#[test]
fn tabs_virtual_spaces() {
    assert_eq!(
        to_html_with_options("```\n\tx", &Options::commonmark()),
        "<pre><code>\tx\n</code></pre>\n",
        "should support a tab in fenced code"
    );

    assert_eq!(
        to_html_with_options(" ```\n\tx", &Options::commonmark()),
        "<pre><code>   x\n</code></pre>\n",
        "should strip 1 space from an initial tab in fenced code if the opening fence is indented as such"
    );

    assert_eq!(
        to_html_with_options("  ```\n\tx", &Options::commonmark()),
        "<pre><code>  x\n</code></pre>\n",
        "should strip 2 spaces from an initial tab in fenced code if the opening fence is indented as such"
    );

    assert_eq!(
        to_html_with_options("   ```\n\tx", &Options::commonmark()),
        "<pre><code> x\n</code></pre>\n",
        "should strip 3 spaces from an initial tab in fenced code if the opening fence is indented as such"
    );

    assert_eq!(
        to_html_with_options("-\ta\n\n\tb", &Options::commonmark()),
        "<ul>\n<li>\n<p>a</p>\n<p>b</p>\n</li>\n</ul>",
        "should support a part of a tab as a container, and the rest of a tab as flow"
    );
}
