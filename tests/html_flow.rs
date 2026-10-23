// Released under the MIT License.
// Copyright, 2022, by Bernhard Berger.
// Copyright, 2022-2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

use core_markdown::{
    mdast::{Html, Node, Root},
    message, to_html, to_html_with_options, to_mdast,
    unist::Position,
    CompileOptions, Constructs, Options, ParseOptions,
};
use pretty_assertions::assert_eq;

#[test]
fn html_flow() -> Result<(), message::Message> {
    let danger = Options {
        compile: CompileOptions {
            allow_dangerous_html: true,
            allow_dangerous_protocol: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let namespaced = Options {
        parse: ParseOptions {
            html_tag_namespaces: true,
            ..Default::default()
        },
        compile: CompileOptions {
            allow_dangerous_html: true,
            allow_dangerous_protocol: true,
            ..Default::default()
        },
    };

    assert_eq!(
        to_html("<!-- asd -->"),
        "&lt;!-- asd --&gt;",
        "should support a heading w/ rank 1"
    );

    assert_eq!(
        to_html_with_options("<!-- asd -->", &danger)?,
        "<!-- asd -->",
        "should support a heading w/ rank 1"
    );

    assert_eq!(
        to_html_with_options("<svg:circle />", &namespaced)?,
        "<svg:circle />",
        "should support namespace-prefixed flow tags when enabled"
    );

    assert_eq!(
        to_html_with_options(
            "<x>",
            &Options {
                parse: ParseOptions {
                    constructs: Constructs {
                        html_flow: false,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                ..Default::default()
            }
        )?,
        "<p>&lt;x&gt;</p>",
        "should support turning off html (flow)"
    );

    assert_eq!(
        to_mdast("<div>\nstuff\n</div>", &Default::default())?,
        Node::Root(Root {
            children: vec![Node::Html(Html {
                value: "<div>\nstuff\n</div>".into(),
                position: Some(Position::new(1, 1, 0, 3, 7, 18))
            })],
            position: Some(Position::new(1, 1, 0, 3, 7, 18))
        }),
        "should support HTML (flow) as `Html`s in mdast"
    );

    Ok(())
}

#[test]
fn html_flow_can_continue_across_blank_lines_when_indented() -> Result<(), message::Message> {
    let options = ParseOptions {
        html_block_blank_lines: true,
        ..Default::default()
    };

    let indented = to_mdast("<div>\n\n  text\n", &options)?;
    let children = indented.children().expect("root has children");
    assert_eq!(children.len(), 1);
    assert!(matches!(children[0], Node::Html(_)));
    assert_eq!(children[0].to_string(), "<div>\n\n  text\n");

    let terminated = to_mdast("<div>\n\ntext\n", &options)?;
    let children = terminated.children().expect("root has children");
    assert_eq!(children.len(), 2);
    assert!(matches!(children[0], Node::Html(_)));
    assert!(matches!(children[1], Node::Paragraph(_)));
    assert_eq!(children[0].to_string(), "<div>\n\n");
    assert_eq!(children[1].to_string(), "text");

    let at_eof = to_mdast("<div>\n\n", &options)?;
    let children = at_eof.children().expect("root has children");
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].to_string(), "<div>\n\n");

    let blank_with_spaces = to_mdast("<div>\n  \n  text\n", &options)?;
    assert_eq!(blank_with_spaces.to_string(), "<div>\n  \n  text\n");

    let tab_indented = to_mdast("<div>\n\n\ttext\n", &options)?;
    assert_eq!(tab_indented.to_string(), "<div>\n\n\ttext\n");

    let trailing_indent = to_mdast("<div>\n  ", &options)?;
    assert_eq!(trailing_indent.to_string(), "<div>\n  ");

    let shallower_continuation = to_mdast("<div>\n\tfirst\n  second\n\n  last\n", &options)?;
    assert!(shallower_continuation.to_string().contains("<div>"));

    Ok(())
}

#[test]
fn html_flow_1_raw() -> Result<(), message::Message> {
    let danger = Options {
        compile: CompileOptions {
            allow_dangerous_html: true,
            allow_dangerous_protocol: true,
            ..Default::default()
        },
        ..Default::default()
    };

    assert_eq!(
        to_html_with_options(
            "<pre language=\"haskell\"><code>
import Text.HTML.TagSoup

main :: IO ()
main = print $ parseTags tags
</code></pre>
okay",
            &danger
        )?,
        "<pre language=\"haskell\"><code>
import Text.HTML.TagSoup

main :: IO ()
main = print $ parseTags tags
</code></pre>
<p>okay</p>",
        "should support raw pre tags (type 1)"
    );

    assert_eq!(
        to_html_with_options(
            "<script type=\"text/javascript\">
// JavaScript example

document.getElementById(\"demo\").innerHTML = \"Hello JavaScript!\";
</script>
okay",
            &danger
        )?,
        "<script type=\"text/javascript\">
// JavaScript example

document.getElementById(\"demo\").innerHTML = \"Hello JavaScript!\";
</script>
<p>okay</p>",
        "should support raw script tags"
    );

    assert_eq!(
        to_html_with_options(
            "<style
  type=\"text/css\">
h1 {color:red;}

p {color:blue;}
</style>
okay",
            &danger
        )?,
        "<style
  type=\"text/css\">
h1 {color:red;}

p {color:blue;}
</style>
<p>okay</p>",
        "should support raw style tags"
    );

    assert_eq!(
        to_html_with_options("<style\n  type=\"text/css\">\n\nfoo", &danger)?,
        "<style\n  type=\"text/css\">\n\nfoo",
        "should support raw tags w/o ending"
    );

    assert_eq!(
        to_html_with_options("<style>p{color:red;}</style>\n*foo*", &danger)?,
        "<style>p{color:red;}</style>\n<p><em>foo</em></p>",
        "should support raw tags w/ start and end on a single line"
    );

    assert_eq!(
        to_html_with_options("<script>\nfoo\n</script>1. *bar*", &danger)?,
        "<script>\nfoo\n</script>1. *bar*",
        "should support raw tags w/ more data on ending line"
    );

    assert_eq!(
        to_html_with_options("<script", &danger)?,
        "<script",
        "should support an eof directly after a raw tag name"
    );

    assert_eq!(
        to_html_with_options("</script\nmore", &danger)?,
        "<p>&lt;/script\nmore</p>",
        "should not support a raw closing tag"
    );

    assert_eq!(
        to_html_with_options("<script/", &danger)?,
        "<p>&lt;script/</p>",
        "should not support an eof after a self-closing slash"
    );

    assert_eq!(
        to_html_with_options("<script/\n*asd*", &danger)?,
        "<p>&lt;script/\n<em>asd</em></p>",
        "should not support a line ending after a self-closing slash"
    );

    assert_eq!(
        to_html_with_options("<script/>", &danger)?,
        "<script/>",
        "should support an eof after a self-closing tag"
    );

    assert_eq!(
        to_html_with_options("<script/>\na", &danger)?,
        "<script/>\na",
        "should support a line ending after a self-closing tag"
    );

    assert_eq!(
        to_html_with_options("<script/>a", &danger)?,
        "<p><script/>a</p>",
        "should not support other characters after a self-closing tag"
    );

    assert_eq!(
        to_html_with_options("<script>a", &danger)?,
        "<script>a",
        "should support other characters after a raw opening tag"
    );

    // Extra.
    assert_eq!(
        to_html_with_options("Foo\n<script", &danger)?,
        "<p>Foo</p>\n<script",
        "should support interrupting paragraphs w/ raw tags"
    );

    assert_eq!(
        to_html_with_options("<script>a</script\nb", &danger)?,
        "<script>a</script\nb",
        "should not support stopping raw if the closing tag does not have a `>`"
    );

    assert_eq!(
        to_html_with_options("<script>\n  \n  \n</script>", &danger)?,
        "<script>\n  \n  \n</script>",
        "should support blank lines in raw"
    );

    assert_eq!(
        to_html_with_options("> <script>\na", &danger)?,
        "<blockquote>\n<script>\n</blockquote>\n<p>a</p>",
        "should not support lazyness (1)"
    );

    assert_eq!(
        to_html_with_options("> a\n<script>", &danger)?,
        "<blockquote>\n<p>a</p>\n</blockquote>\n<script>",
        "should not support lazyness (2)"
    );

    Ok(())
}

#[test]
fn html_flow_2_comment() -> Result<(), message::Message> {
    let danger = Options {
        compile: CompileOptions {
            allow_dangerous_html: true,
            allow_dangerous_protocol: true,
            ..Default::default()
        },
        ..Default::default()
    };

    assert_eq!(
        to_html_with_options("<!-- Foo\n\nbar\n   baz -->\nokay", &danger)?,
        "<!-- Foo\n\nbar\n   baz -->\n<p>okay</p>",
        "should support comments (type 2)"
    );

    assert_eq!(
        to_html_with_options("<!-- foo -->*bar*\n*baz*", &danger)?,
        "<!-- foo -->*bar*\n<p><em>baz</em></p>",
        "should support comments w/ start and end on a single line"
    );

    assert_eq!(
        to_html_with_options("<!-asd-->", &danger)?,
        "<p>&lt;!-asd--&gt;</p>",
        "should not support a single dash to start comments"
    );

    assert_eq!(
        to_html_with_options("<!-->", &danger)?,
        "<!-->",
        "should support comments where the start dashes are the end dashes (1)"
    );

    assert_eq!(
        to_html_with_options("<!--->", &danger)?,
        "<!--->",
        "should support comments where the start dashes are the end dashes (2)"
    );

    assert_eq!(
        to_html_with_options("<!---->", &danger)?,
        "<!---->",
        "should support empty comments"
    );

    // If the `\"` is encoded, we’re in text. If it remains, we’re in HTML.
    assert_eq!(
        to_html_with_options("<!--\n->\n\"", &danger)?,
        "<!--\n->\n\"",
        "should not end a comment at one dash (`->`)"
    );
    assert_eq!(
        to_html_with_options("<!--\n-->\n\"", &danger)?,
        "<!--\n-->\n<p>&quot;</p>",
        "should end a comment at two dashes (`-->`)"
    );
    assert_eq!(
        to_html_with_options("<!--\n--->\n\"", &danger)?,
        "<!--\n--->\n<p>&quot;</p>",
        "should end a comment at three dashes (`--->`)"
    );
    assert_eq!(
        to_html_with_options("<!--\n---->\n\"", &danger)?,
        "<!--\n---->\n<p>&quot;</p>",
        "should end a comment at four dashes (`---->`)"
    );

    assert_eq!(
        to_html_with_options("  <!-- foo -->", &danger)?,
        "  <!-- foo -->",
        "should support comments w/ indent"
    );

    assert_eq!(
        to_html_with_options("    <!-- foo -->", &danger)?,
        "<pre><code>&lt;!-- foo --&gt;\n</code></pre>",
        "should not support comments w/ a 4 character indent"
    );

    // Extra.
    assert_eq!(
        to_html_with_options("Foo\n<!--", &danger)?,
        "<p>Foo</p>\n<!--",
        "should support interrupting paragraphs w/ comments"
    );

    assert_eq!(
        to_html_with_options("<!--\n  \n  \n-->", &danger)?,
        "<!--\n  \n  \n-->",
        "should support blank lines in comments"
    );

    assert_eq!(
        to_html_with_options("> <!--\na", &danger)?,
        "<blockquote>\n<!--\n</blockquote>\n<p>a</p>",
        "should not support lazyness (1)"
    );

    assert_eq!(
        to_html_with_options("> a\n<!--", &danger)?,
        "<blockquote>\n<p>a</p>\n</blockquote>\n<!--",
        "should not support lazyness (2)"
    );

    Ok(())
}

#[test]
fn html_flow_3_instruction() -> Result<(), message::Message> {
    let danger = Options {
        compile: CompileOptions {
            allow_dangerous_html: true,
            allow_dangerous_protocol: true,
            ..Default::default()
        },
        ..Default::default()
    };

    assert_eq!(
        to_html_with_options("<?php\n\n  echo \">\";\n\n?>\nokay", &danger)?,
        "<?php\n\n  echo \">\";\n\n?>\n<p>okay</p>",
        "should support instructions (type 3)"
    );

    assert_eq!(
        to_html_with_options("<?>", &danger)?,
        "<?>",
        "should support empty instructions where the `?` is part of both the start and the end"
    );

    assert_eq!(
        to_html_with_options("<??>", &danger)?,
        "<??>",
        "should support empty instructions"
    );

    // Extra.
    assert_eq!(
        to_html_with_options("Foo\n<?", &danger)?,
        "<p>Foo</p>\n<?",
        "should support interrupting paragraphs w/ instructions"
    );

    assert_eq!(
        to_html_with_options("<?\n  \n  \n?>", &danger)?,
        "<?\n  \n  \n?>",
        "should support blank lines in instructions"
    );

    assert_eq!(
        to_html_with_options("> <?\na", &danger)?,
        "<blockquote>\n<?\n</blockquote>\n<p>a</p>",
        "should not support lazyness (1)"
    );

    assert_eq!(
        to_html_with_options("> a\n<?", &danger)?,
        "<blockquote>\n<p>a</p>\n</blockquote>\n<?",
        "should not support lazyness (2)"
    );

    Ok(())
}

#[test]
fn html_flow_4_declaration() -> Result<(), message::Message> {
    let danger = Options {
        compile: CompileOptions {
            allow_dangerous_html: true,
            allow_dangerous_protocol: true,
            ..Default::default()
        },
        ..Default::default()
    };

    assert_eq!(
        to_html_with_options("<!DOCTYPE html>", &danger)?,
        "<!DOCTYPE html>",
        "should support declarations (type 4)"
    );

    assert_eq!(
        to_html_with_options("<!123>", &danger)?,
        "<p>&lt;!123&gt;</p>",
        "should not support declarations that start w/o an alpha"
    );

    assert_eq!(
        to_html_with_options("<!>", &danger)?,
        "<p>&lt;!&gt;</p>",
        "should not support declarations w/o an identifier"
    );

    assert_eq!(
        to_html_with_options("<!a>", &danger)?,
        "<!a>",
        "should support declarations w/o a single alpha as identifier"
    );

    // Extra.
    assert_eq!(
        to_html_with_options("Foo\n<!d", &danger)?,
        "<p>Foo</p>\n<!d",
        "should support interrupting paragraphs w/ declarations"
    );

    // Note about the lower letter:
    // <https://github.com/commonmark/commonmark-spec/pull/621>
    assert_eq!(
        to_html_with_options("<!a\n  \n  \n>", &danger)?,
        "<!a\n  \n  \n>",
        "should support blank lines in declarations"
    );

    assert_eq!(
        to_html_with_options("> <!a\nb", &danger)?,
        "<blockquote>\n<!a\n</blockquote>\n<p>b</p>",
        "should not support lazyness (1)"
    );

    assert_eq!(
        to_html_with_options("> a\n<!b", &danger)?,
        "<blockquote>\n<p>a</p>\n</blockquote>\n<!b",
        "should not support lazyness (2)"
    );

    Ok(())
}

#[test]
fn html_flow_5_cdata() -> Result<(), message::Message> {
    let danger = Options {
        compile: CompileOptions {
            allow_dangerous_html: true,
            allow_dangerous_protocol: true,
            ..Default::default()
        },
        ..Default::default()
    };

    assert_eq!(
        to_html_with_options(
            "<![CDATA[\nfunction matchwo(a,b)\n{\n  if (a < b && a < 0) then {\n    return 1;\n\n  } else {\n\n    return 0;\n  }\n}\n]]>\nokay",
            &danger
        )?,
        "<![CDATA[\nfunction matchwo(a,b)\n{\n  if (a < b && a < 0) then {\n    return 1;\n\n  } else {\n\n    return 0;\n  }\n}\n]]>\n<p>okay</p>",
        "should support cdata (type 5)"
    );

    assert_eq!(
        to_html_with_options("<![CDATA[]]>", &danger)?,
        "<![CDATA[]]>",
        "should support empty cdata"
    );

    assert_eq!(
        to_html_with_options("<![CDATA]]>", &danger)?,
        "<p>&lt;![CDATA]]&gt;</p>",
        "should not support cdata w/ a missing `[`"
    );

    assert_eq!(
        to_html_with_options("<![CDATA[]]]>", &danger)?,
        "<![CDATA[]]]>",
        "should support cdata w/ a single `]` as content"
    );

    // Extra.
    assert_eq!(
        to_html_with_options("Foo\n<![CDATA[", &danger)?,
        "<p>Foo</p>\n<![CDATA[",
        "should support interrupting paragraphs w/ cdata"
    );

    // Note: cmjs parses this differently.
    // See: <https://github.com/commonmark/commonmark.js/issues/193>
    assert_eq!(
        to_html_with_options("<![cdata[]]>", &danger)?,
        "<p>&lt;![cdata[]]&gt;</p>",
        "should not support lowercase cdata"
    );

    assert_eq!(
        to_html_with_options("<![CDATA[\n  \n  \n]]>", &danger)?,
        "<![CDATA[\n  \n  \n]]>",
        "should support blank lines in cdata"
    );

    assert_eq!(
        to_html_with_options("> <![CDATA[\na", &danger)?,
        "<blockquote>\n<![CDATA[\n</blockquote>\n<p>a</p>",
        "should not support lazyness (1)"
    );

    assert_eq!(
        to_html_with_options("> a\n<![CDATA[", &danger)?,
        "<blockquote>\n<p>a</p>\n</blockquote>\n<![CDATA[",
        "should not support lazyness (2)"
    );

    Ok(())
}

#[test]
fn html_flow_6_basic() -> Result<(), message::Message> {
    let danger = Options {
        compile: CompileOptions {
            allow_dangerous_html: true,
            allow_dangerous_protocol: true,
            ..Default::default()
        },
        ..Default::default()
    };

    assert_eq!(
        to_html_with_options(
            "<table><tr><td>\n<pre>\n**Hello**,\n\n_world_.\n</pre>\n</td></tr></table>",
            &danger
        )?,
        "<table><tr><td>\n<pre>\n**Hello**,\n<p><em>world</em>.\n</pre></p>\n</td></tr></table>",
        "should support html (basic)"
    );

    assert_eq!(
        to_html_with_options(
            "<table>
  <tr>
    <td>
           hi
    </td>
  </tr>
</table>

okay.",
            &danger
        )?,
        "<table>
  <tr>
    <td>
           hi
    </td>
  </tr>
</table>
<p>okay.</p>",
        "should support html of type 6 (1)"
    );

    assert_eq!(
        to_html_with_options(" <div>\n  *hello*\n         <foo><a>", &danger)?,
        " <div>\n  *hello*\n         <foo><a>",
        "should support html of type 6 (2)"
    );

    assert_eq!(
        to_html_with_options("</div>\n*foo*", &danger)?,
        "</div>\n*foo*",
        "should support html starting w/ a closing tag"
    );

    assert_eq!(
        to_html_with_options("<DIV CLASS=\"foo\">\n\n*Markdown*\n\n</DIV>", &danger)?,
        "<DIV CLASS=\"foo\">\n<p><em>Markdown</em></p>\n</DIV>",
        "should support html w/ markdown in between"
    );

    assert_eq!(
        to_html_with_options("<div id=\"foo\"\n  class=\"bar\">\n</div>", &danger)?,
        "<div id=\"foo\"\n  class=\"bar\">\n</div>",
        "should support html w/ line endings (1)"
    );

    assert_eq!(
        to_html_with_options("<div id=\"foo\" class=\"bar\n  baz\">\n</div>", &danger)?,
        "<div id=\"foo\" class=\"bar\n  baz\">\n</div>",
        "should support html w/ line endings (2)"
    );

    assert_eq!(
        to_html_with_options("<div>\n*foo*\n\n*bar*", &danger)?,
        "<div>\n*foo*\n<p><em>bar</em></p>",
        "should support an unclosed html element"
    );

    assert_eq!(
        to_html_with_options("<div id=\"foo\"\n*hi*", &danger)?,
        "<div id=\"foo\"\n*hi*",
        "should support garbage html (1)"
    );

    assert_eq!(
        to_html_with_options("<div class\nfoo", &danger)?,
        "<div class\nfoo",
        "should support garbage html (2)"
    );

    assert_eq!(
        to_html_with_options("<div *???-&&&-<---\n*foo*", &danger)?,
        "<div *???-&&&-<---\n*foo*",
        "should support garbage html (3)"
    );

    assert_eq!(
        to_html_with_options("<div><a href=\"bar\">*foo*</a></div>", &danger)?,
        "<div><a href=\"bar\">*foo*</a></div>",
        "should support other tags in the opening (1)"
    );

    assert_eq!(
        to_html_with_options("<table><tr><td>\nfoo\n</td></tr></table>", &danger)?,
        "<table><tr><td>\nfoo\n</td></tr></table>",
        "should support other tags in the opening (2)"
    );

    assert_eq!(
        to_html_with_options("<div></div>\n``` c\nint x = 33;\n```", &danger)?,
        "<div></div>\n``` c\nint x = 33;\n```",
        "should include everything ’till a blank line"
    );

    assert_eq!(
        to_html_with_options("> <div>\n> foo\n\nbar", &danger)?,
        "<blockquote>\n<div>\nfoo\n</blockquote>\n<p>bar</p>",
        "should support basic tags w/o ending in containers (1)"
    );

    assert_eq!(
        to_html_with_options("- <div>\n- foo", &danger)?,
        "<ul>\n<li>\n<div>\n</li>\n<li>foo</li>\n</ul>",
        "should support basic tags w/o ending in containers (2)"
    );

    assert_eq!(
        to_html_with_options("  <div>", &danger)?,
        "  <div>",
        "should support basic tags w/ indent"
    );

    assert_eq!(
        to_html_with_options("    <div>", &danger)?,
        "<pre><code>&lt;div&gt;\n</code></pre>",
        "should not support basic tags w/ a 4 character indent"
    );

    assert_eq!(
        to_html_with_options("Foo\n<div>\nbar\n</div>", &danger)?,
        "<p>Foo</p>\n<div>\nbar\n</div>",
        "should support interrupting paragraphs w/ basic tags"
    );

    assert_eq!(
        to_html_with_options("<div>\nbar\n</div>\n*foo*", &danger)?,
        "<div>\nbar\n</div>\n*foo*",
        "should require a blank line to end"
    );

    assert_eq!(
        to_html_with_options("<div>\n\n*Emphasized* text.\n\n</div>", &danger)?,
        "<div>\n<p><em>Emphasized</em> text.</p>\n</div>",
        "should support interleaving w/ blank lines"
    );

    assert_eq!(
        to_html_with_options("<div>\n*Emphasized* text.\n</div>", &danger)?,
        "<div>\n*Emphasized* text.\n</div>",
        "should not support interleaving w/o blank lines"
    );

    assert_eq!(
        to_html_with_options(
            "<table>\n\n<tr>\n\n<td>\nHi\n</td>\n\n</tr>\n\n</table>",
            &danger
        )?,
        "<table>\n<tr>\n<td>\nHi\n</td>\n</tr>\n</table>",
        "should support blank lines between adjacent html"
    );

    assert_eq!(
        to_html_with_options(
            "<table>

  <tr>

    <td>
      Hi
    </td>

  </tr>

</table>",
            &danger
        )?,
        "<table>
  <tr>
<pre><code>&lt;td&gt;
  Hi
&lt;/td&gt;
</code></pre>
  </tr>
</table>",
        "should not support indented, blank-line delimited, adjacent html"
    );

    assert_eq!(
        to_html_with_options("</1>", &danger)?,
        "<p>&lt;/1&gt;</p>",
        "should not support basic tags w/ an incorrect name start character"
    );

    assert_eq!(
        to_html_with_options("<div", &danger)?,
        "<div",
        "should support an eof directly after a basic tag name"
    );

    assert_eq!(
        to_html_with_options("<div\n", &danger)?,
        "<div\n",
        "should support a line ending directly after a tag name"
    );

    assert_eq!(
        to_html_with_options("<div ", &danger)?,
        "<div ",
        "should support an eof after a space directly after a tag name"
    );

    assert_eq!(
        to_html_with_options("<div/", &danger)?,
        "<p>&lt;div/</p>",
        "should not support an eof directly after a self-closing slash"
    );

    assert_eq!(
        to_html_with_options("<div/\n*asd*", &danger)?,
        "<p>&lt;div/\n<em>asd</em></p>",
        "should not support a line ending after a self-closing slash"
    );

    assert_eq!(
        to_html_with_options("<div/>", &danger)?,
        "<div/>",
        "should support an eof after a self-closing tag"
    );

    assert_eq!(
        to_html_with_options("<div/>\na", &danger)?,
        "<div/>\na",
        "should support a line ending after a self-closing tag"
    );

    assert_eq!(
        to_html_with_options("<div/>a", &danger)?,
        "<div/>a",
        "should support another character after a self-closing tag"
    );

    assert_eq!(
        to_html_with_options("<div>a", &danger)?,
        "<div>a",
        "should support another character after a basic opening tag"
    );

    // Extra.
    assert_eq!(
        to_html_with_options("Foo\n<div/>", &danger)?,
        "<p>Foo</p>\n<div/>",
        "should support interrupting paragraphs w/ self-closing basic tags"
    );

    assert_eq!(
        to_html_with_options("<div\n  \n  \n>", &danger)?,
        "<div\n<blockquote>\n</blockquote>",
        "should not support blank lines in basic"
    );

    assert_eq!(
        to_html_with_options("> <div\na", &danger)?,
        "<blockquote>\n<div\n</blockquote>\n<p>a</p>",
        "should not support lazyness (1)"
    );

    assert_eq!(
        to_html_with_options("> a\n<div", &danger)?,
        "<blockquote>\n<p>a</p>\n</blockquote>\n<div",
        "should not support lazyness (2)"
    );

    Ok(())
}

#[test]
fn html_flow_7_complete() -> Result<(), message::Message> {
    let danger = Options {
        compile: CompileOptions {
            allow_dangerous_html: true,
            allow_dangerous_protocol: true,
            ..Default::default()
        },
        ..Default::default()
    };

    assert_eq!(
        to_html_with_options("<a href=\"foo\">\n*bar*\n</a>", &danger)?,
        "<a href=\"foo\">\n*bar*\n</a>",
        "should support complete tags (type 7)"
    );

    assert_eq!(
        to_html_with_options("<Warning>\n*bar*\n</Warning>", &danger)?,
        "<Warning>\n*bar*\n</Warning>",
        "should support non-html tag names"
    );

    assert_eq!(
        to_html_with_options("<i class=\"foo\">\n*bar*\n</i>", &danger)?,
        "<i class=\"foo\">\n*bar*\n</i>",
        "should support non-“block” html tag names (1)"
    );

    assert_eq!(
        to_html_with_options("<del>\n*foo*\n</del>", &danger)?,
        "<del>\n*foo*\n</del>",
        "should support non-“block” html tag names (2)"
    );

    assert_eq!(
        to_html_with_options("</ins>\n*bar*", &danger)?,
        "</ins>\n*bar*",
        "should support closing tags"
    );

    assert_eq!(
        to_html_with_options("<del>\n\n*foo*\n\n</del>", &danger)?,
        "<del>\n<p><em>foo</em></p>\n</del>",
        "should support interleaving"
    );

    assert_eq!(
        to_html_with_options("<del>*foo*</del>", &danger)?,
        "<p><del><em>foo</em></del></p>",
        "should not support interleaving w/o blank lines"
    );

    assert_eq!(
        to_html_with_options("<div>\n  \nasd", &danger)?,
        "<div>\n<p>asd</p>",
        "should support interleaving w/ whitespace-only blank lines"
    );

    assert_eq!(
        to_html_with_options("Foo\n<a href=\"bar\">\nbaz", &danger)?,
        "<p>Foo\n<a href=\"bar\">\nbaz</p>",
        "should not support interrupting paragraphs w/ complete tags"
    );

    assert_eq!(
        to_html_with_options("<x", &danger)?,
        "<p>&lt;x</p>",
        "should not support an eof directly after a tag name"
    );

    assert_eq!(
        to_html_with_options("<x/", &danger)?,
        "<p>&lt;x/</p>",
        "should not support an eof directly after a self-closing slash"
    );

    assert_eq!(
        to_html_with_options("<x\n", &danger)?,
        "<p>&lt;x</p>\n",
        "should not support a line ending directly after a tag name"
    );

    assert_eq!(
        to_html_with_options("<x ", &danger)?,
        "<p>&lt;x</p>",
        "should not support an eof after a space directly after a tag name"
    );

    assert_eq!(
        to_html_with_options("<x/", &danger)?,
        "<p>&lt;x/</p>",
        "should not support an eof directly after a self-closing slash"
    );

    assert_eq!(
        to_html_with_options("<x/\n*asd*", &danger)?,
        "<p>&lt;x/\n<em>asd</em></p>",
        "should not support a line ending after a self-closing slash"
    );

    assert_eq!(
        to_html_with_options("<x/>", &danger)?,
        "<x/>",
        "should support an eof after a self-closing tag"
    );

    assert_eq!(
        to_html_with_options("<x/>\na", &danger)?,
        "<x/>\na",
        "should support a line ending after a self-closing tag"
    );

    assert_eq!(
        to_html_with_options("<x/>a", &danger)?,
        "<p><x/>a</p>",
        "should not support another character after a self-closing tag"
    );

    assert_eq!(
        to_html_with_options("<x>a", &danger)?,
        "<p><x>a</p>",
        "should not support another character after an opening tag"
    );

    assert_eq!(
        to_html_with_options("<x y>", &danger)?,
        "<x y>",
        "should support boolean attributes in a complete tag"
    );

    assert_eq!(
        to_html_with_options("<x\ny>", &danger)?,
        "<p><x\ny></p>",
        "should not support a line ending before an attribute name"
    );

    assert_eq!(
        to_html_with_options("<x\n  y>", &danger)?,
        "<p><x\ny></p>",
        "should not support a line ending w/ whitespace before an attribute name"
    );

    assert_eq!(
        to_html_with_options("<x\n  \ny>", &danger)?,
        "<p>&lt;x</p>\n<p>y&gt;</p>",
        "should not support a line ending w/ whitespace and another line ending before an attribute name"
    );

    assert_eq!(
        to_html_with_options("<x y\nz>", &danger)?,
        "<p><x y\nz></p>",
        "should not support a line ending between attribute names"
    );

    assert_eq!(
        to_html_with_options("<x y   z>", &danger)?,
        "<x y   z>",
        "should support whitespace between attribute names"
    );

    assert_eq!(
        to_html_with_options("<x:y>", &danger)?,
        "<p>&lt;x:y&gt;</p>",
        "should not support a colon in a tag name"
    );

    assert_eq!(
        to_html_with_options("<x_y>", &danger)?,
        "<p>&lt;x_y&gt;</p>",
        "should not support an underscore in a tag name"
    );

    assert_eq!(
        to_html_with_options("<x.y>", &danger)?,
        "<p>&lt;x.y&gt;</p>",
        "should not support a dot in a tag name"
    );

    assert_eq!(
        to_html_with_options("<x :y>", &danger)?,
        "<x :y>",
        "should support a colon to start an attribute name"
    );

    assert_eq!(
        to_html_with_options("<x _y>", &danger)?,
        "<x _y>",
        "should support an underscore to start an attribute name"
    );

    assert_eq!(
        to_html_with_options("<x .y>", &danger)?,
        "<p>&lt;x .y&gt;</p>",
        "should not support a dot to start an attribute name"
    );

    assert_eq!(
        to_html_with_options("<x y:>", &danger)?,
        "<x y:>",
        "should support a colon to end an attribute name"
    );

    assert_eq!(
        to_html_with_options("<x y_>", &danger)?,
        "<x y_>",
        "should support an underscore to end an attribute name"
    );

    assert_eq!(
        to_html_with_options("<x y.>", &danger)?,
        "<x y.>",
        "should support a dot to end an attribute name"
    );

    assert_eq!(
        to_html_with_options("<x y123>", &danger)?,
        "<x y123>",
        "should support numbers to end an attribute name"
    );

    assert_eq!(
        to_html_with_options("<x data->", &danger)?,
        "<x data->",
        "should support a dash to end an attribute name"
    );

    assert_eq!(
        to_html_with_options("<x y=>", &danger)?,
        "<p>&lt;x y=&gt;</p>",
        "should not upport an initializer w/o a value"
    );

    assert_eq!(
        to_html_with_options("<x y==>", &danger)?,
        "<p>&lt;x y==&gt;</p>",
        "should not support an equals to as an initializer"
    );

    assert_eq!(
        to_html_with_options("<x y=z>", &danger)?,
        "<x y=z>",
        "should support a single character as an unquoted attribute value"
    );

    assert_eq!(
        to_html_with_options("<x y=\"\">", &danger)?,
        "<x y=\"\">",
        "should support an empty double quoted attribute value"
    );

    assert_eq!(
        to_html_with_options("<x y=\"\">", &danger)?,
        "<x y=\"\">",
        "should support an empty single quoted attribute value"
    );

    assert_eq!(
        to_html_with_options("<x y=\"\n\">", &danger)?,
        "<p><x y=\"\n\"></p>",
        "should not support a line ending in a double quoted attribute value"
    );

    assert_eq!(
        to_html_with_options("<x y=\"\n\">", &danger)?,
        "<p><x y=\"\n\"></p>",
        "should not support a line ending in a single quoted attribute value"
    );

    assert_eq!(
        to_html_with_options("<w x=y\nz>", &danger)?,
        "<p><w x=y\nz></p>",
        "should not support a line ending in/after an unquoted attribute value"
    );

    assert_eq!(
        to_html_with_options("<w x=y\"z>", &danger)?,
        "<p>&lt;w x=y&quot;z&gt;</p>",
        "should not support a double quote in/after an unquoted attribute value"
    );

    assert_eq!(
        to_html_with_options("<w x=y'z>", &danger)?,
        "<p>&lt;w x=y'z&gt;</p>",
        "should not support a single quote in/after an unquoted attribute value"
    );

    assert_eq!(
        to_html_with_options("<x y=\"\"z>", &danger)?,
        "<p>&lt;x y=&quot;&quot;z&gt;</p>",
        "should not support an attribute after a double quoted attribute value"
    );

    assert_eq!(
        to_html_with_options("<x>\n  \n  \n>", &danger)?,
        "<x>\n<blockquote>\n</blockquote>",
        "should not support blank lines in complete"
    );

    assert_eq!(
        to_html_with_options("> <a>\n*bar*", &danger)?,
        "<blockquote>\n<a>\n</blockquote>\n<p><em>bar</em></p>",
        "should not support lazyness (1)"
    );

    assert_eq!(
        to_html_with_options("> a\n<a>", &danger)?,
        "<blockquote>\n<p>a</p>\n</blockquote>\n<a>",
        "should not support lazyness (2)"
    );

    Ok(())
}

#[test]
fn retained_blank_lines_end_at_the_enclosing_container_boundary() {
    use core_markdown::{render_events, Document, EventParser};
    let options = Options {
        parse: ParseOptions {
            html_block_blank_lines: true,
            ..ParseOptions::default()
        },
        ..Options::default()
    };
    for (source, expected) in [
        ("><v>\n\n", "<blockquote>\n&lt;v&gt;\n</blockquote>\n"),
        (
            "> <v>\n>\n\na",
            "<blockquote>\n&lt;v&gt;\n\n</blockquote>\n<p>a</p>",
        ),
        (
            "- <v>\n\na",
            "<ul>\n<li>\n&lt;v&gt;\n\n</li>\n</ul>\n<p>a</p>",
        ),
    ] {
        let output = to_html_with_options(source, &options).unwrap();
        assert_eq!(output, expected, "{source:?}");
        let mut checked = String::new();
        render_events(
            source,
            EventParser::new(source, &options.parse).unwrap(),
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

#[test]
fn retained_html_keeps_only_lines_inside_the_active_container() {
    use core_markdown::{render_events, stream::EventKind, Document, EventParser};
    let options = Options {
        parse: ParseOptions {
            html_block_blank_lines: true,
            ..ParseOptions::default()
        },
        ..Options::default()
    };
    for (source, raw, quotes) in [
        ("> <v>\n>\n>\n\n", "<v>\n\n\n", 1),
        ("><v>\n \t", "<v>\n", 1),
        ("> > <v>\n> >\n>\n> outside", "<v>\n\n", 2),
        ("> - <v>\n>\n> outside", "<v>\n\n", 1),
    ] {
        let events = EventParser::new(source, &options.parse).unwrap();
        let mut bodies = Vec::new();
        for event in events {
            if let EventKind::Html { value, block: true } = event.unwrap().kind {
                bodies.push(value.resolve(source).unwrap().to_string());
            }
        }
        assert_eq!(bodies, [raw], "{source:?}");
        let output = to_html_with_options(source, &options).unwrap();
        assert_eq!(output.matches("<blockquote>").count(), quotes);
        assert_eq!(output.matches("</blockquote>").count(), quotes);
        if source.ends_with("outside") {
            assert!(
                output.ends_with("<p>outside</p>\n</blockquote>"),
                "{:?}",
                output
            );
        }
        let document = Document::parse(source, &options.parse).unwrap();
        let mut arena = String::new();
        render_events(source, document.events(), &options.compile, &mut arena).unwrap();
        assert_eq!(arena, output);
    }
}

#[test]
fn retained_html_preserves_adjacent_sibling_list_items() {
    use core_markdown::{render_events, Document, EventParser};
    let options = Options {
        parse: ParseOptions {
            html_block_blank_lines: true,
            ..ParseOptions::default()
        },
        ..Options::default()
    };
    for source in [
        "- <v>\n\n-",
        "- <v>\n\n\n-",
        "- >\r\t<x>\n-",
        "- >\r\t<x>\n\n-",
        "- >\r\t<x>\n\n\n-",
    ] {
        let output = to_html_with_options(source, &options).unwrap();
        assert_eq!(output.matches("<ul>").count(), 1, "{source:?}");
        assert_eq!(output.matches("<li>").count(), 2, "{source:?}");
        assert!(
            output
                .replace('\r', "\n")
                .ends_with("</li>\n<li></li>\n</ul>"),
            "{:?}",
            output
        );
        let mut checked = String::new();
        render_events(
            source,
            EventParser::new(source, &options.parse).unwrap(),
            &options.compile,
            &mut checked,
        )
        .unwrap();
        assert_eq!(checked, output);
        let document = Document::parse(source, &options.parse).unwrap();
        let mut arena = String::new();
        render_events(source, document.events(), &options.compile, &mut arena).unwrap();
        assert_eq!(arena, output);
    }
}

#[test]
fn compact_html_preserves_values_spans_and_rendering_paths() {
    use core_markdown::{render_events, stream::EventKind, Document, EventParser};

    let mut options = Options::default();
    options.compile.allow_dangerous_html = true;
    let mut grammar = Options::default();
    grammar.compile.allow_dangerous_html = true;
    // None of these sources contains math. This independently selects grammar
    // preparation without changing HTML blank-line or namespace semantics.
    grammar.parse.constructs.math_flow = true;
    let blocks = [
        "<!-- comment -->",
        "<!-->",
        "<!--->",
        "<!-- --x\n\n  --> tail",
        "<!-- -x -->",
        "<!-- unclosed\n\nlast",
        "<?target?> tail",
        "<?>\ntext",
        "<?target\n\n?>",
        "<!DOCTYPE html>",
        "<!D\n>",
        "<![CDATA[<x>]]>",
        "<![CDATA[\n]\n]]>",
        "<script>x</StYlE> tail",
        "<PRE>\n\ncode\n</pre>",
        "<textarea\ntext",
        "<style>x</notraw><x></textarea>",
        "<script></123></abcdefghi></script>",
        "<div>\n**literal**\n</div>",
        "</DIV> tail",
        "<div/>\nmore",
        "<div/ >",
        "<divx>",
        "<custom key='value'>\ntext",
        "<https://example.com>",
        "<",
        "</",
        "<!?x>",
        "<!-- nul\0 -->",
        "  <!-- indented -->",
        "   <div>\n   content",
        "<!-- first -->\n<!-- second -->",
        "<!-- first -->\n[a]: /a\n[a]",
    ];
    for block in blocks {
        for ending in ["", "\n", "\n\n", "\r", "\r\n"] {
            for source in [
                format!("{block}{ending}"),
                format!("before\n{block}{ending}"),
                format!("{block}\n\nafter{ending}"),
                format!("> {}{ending}", block.replace('\n', "\n> ")),
                format!("- {}{ending}", block.replace('\n', "\n  ")),
                format!("-\t{}{ending}", block.replace('\n', "\n    ")),
                format!("- {}{ending}", block.replace('\n', "\n\t")),
                format!("- before\n\n  {}{ending}", block.replace('\n', "\n  ")),
            ] {
                let expected = to_html_with_options(&source, &grammar).unwrap();
                let direct = to_html_with_options(&source, &options).unwrap();
                // Ordinary paragraphs outside the HTML block use the existing compact
                // container-tab normalization. HTML values and spans remain exact below.
                if !source.contains('\t') {
                    assert_eq!(direct, expected, "source: {source:?}");
                }
                let html = |parse: &ParseOptions| {
                    EventParser::new(&source, parse)
                        .unwrap()
                        .filter_map(|event| {
                            let event = event.unwrap();
                            match event.kind {
                                EventKind::Html { value, block } => Some((
                                    value.resolve(&source).unwrap().to_owned(),
                                    block,
                                    event.span,
                                )),
                                _ => None,
                            }
                        })
                        .collect::<Vec<_>>()
                };
                assert_eq!(
                    html(&options.parse),
                    html(&grammar.parse),
                    "HTML: {source:?}"
                );
                let mut checked = String::new();
                render_events(
                    &source,
                    EventParser::new(&source, &options.parse).unwrap(),
                    &options.compile,
                    &mut checked,
                )
                .unwrap();
                assert_eq!(checked, direct, "checked: {source:?}");
                let document = Document::parse(source.as_str(), &options.parse).unwrap();
                let mut arena = String::new();
                render_events(&source, document.events(), &options.compile, &mut arena).unwrap();
                assert_eq!(arena, direct, "arena: {source:?}");
            }
        }
    }
}

#[test]
fn unclosed_html_container_boundaries_preserve_terminal_source_endings() {
    use core_markdown::{stream::EventKind, EventParser};
    let mut options = Options::default();
    options.compile.allow_dangerous_html = true;
    let mut grammar = Options::default();
    grammar.compile.allow_dangerous_html = true;
    grammar.parse.constructs.math_flow = true;
    for opening in ["<script>", "<!--", "<?target", "<!DOCTYPE", "<![CDATA["] {
        for prefix in ["- ", "> "] {
            for between in ["", "  \n", "  \n  \n"] {
                for following in ["- x", "> x", "text", "", "\ntext"] {
                    for ending in ["\n", "\r", "\r\n"] {
                        let source = format!("{prefix}{opening}\n{between}{following}")
                            .replace('\n', ending);
                        assert_eq!(
                            to_html_with_options(&source, &options).unwrap(),
                            to_html_with_options(&source, &grammar).unwrap(),
                            "{source:?}"
                        );
                        let values = |parse: &ParseOptions| {
                            EventParser::new(&source, parse)
                                .unwrap()
                                .map(Result::unwrap)
                                .filter_map(|event| match event.kind {
                                    EventKind::Html { value, block } => Some((
                                        event.span,
                                        value.resolve(&source).unwrap().to_string(),
                                        block,
                                    )),
                                    _ => None,
                                })
                                .collect::<Vec<_>>()
                        };
                        assert_eq!(values(&options.parse), values(&grammar.parse), "{source:?}");
                    }
                }
            }
        }
    }
}
