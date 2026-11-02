// Released under the MIT License.
// Copyright, 2022, by Bernhard Berger.
// Copyright, 2022-2025, by Titus Wormer.
// Copyright, 2023, by cel.
// Copyright, 2025, by Ophir Lojkine.
// Copyright, 2026, by Samuel Williams.

use crate::util::line_ending::LineEnding;
use alloc::string::String;

/// Control which constructs are enabled.
///
/// Not all constructs can be configured.
/// Notably, blank lines and paragraphs cannot be turned off.
///
/// ## Examples
///
/// ```
/// use base_markdown::Constructs;
/// # fn main() {
///
/// // Use the default trait to get `CommonMark` constructs:
/// let commonmark = Constructs::default();
///
/// // To turn on all of GFM, use the `gfm` method:
/// let gfm = Constructs::gfm();
///
/// // Or, mix and match:
/// let custom = Constructs {
///   math_flow: true,
///   math_text: true,
///   ..Constructs::gfm()
/// };
/// # }
/// ```
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct Constructs {
    /// Attention.
    ///
    /// ```markdown
    /// > | a *b* c **d**.
    ///       ^^^   ^^^^^
    /// ```
    pub attention: bool,
    /// Autolink.
    ///
    /// ```markdown
    /// > | a <https://example.com> b <user@example.org>.
    ///       ^^^^^^^^^^^^^^^^^^^^^   ^^^^^^^^^^^^^^^^^^
    /// ```
    ///
    /// Only these schemes, in any case, and email addresses form autolinks:
    ///
    /// * `http`, `https`, `ftp`, `sftp`, `ssh`, `irc` and `ircs`, followed by `://` and a host, such as `<ssh://example.com>`.
    /// * `mailto` and `xmpp`, followed by one email address and an optional `?query`, such as `<mailto:a@example.com?subject=x>`. `xmpp` also allows a `/resource` after the address.
    /// * `tel` and `sms`, followed by `+`, digits, `-`, `.`, `(` and `)`, with at least one digit, such as `<tel:+1-555-0100>`.
    ///
    /// This deliberately diverges from `CommonMark`, which accepts any scheme of 2–32 characters and an empty target: other text such as `<svg:rect>` or `Option<std::io::Error>` is raw HTML when valid and text otherwise.
    pub autolink: bool,
    /// Block quote.
    ///
    /// ```markdown
    /// > | > a
    ///     ^^^
    /// ```
    pub block_quote: bool,
    /// Character escape.
    ///
    /// ```markdown
    /// > | a \* b
    ///       ^^
    /// ```
    pub character_escape: bool,
    /// Character reference.
    ///
    /// ```markdown
    /// > | a &amp; b
    ///       ^^^^^
    /// ```
    pub character_reference: bool,
    /// Code (indented).
    ///
    /// ```markdown
    /// > |     a
    ///     ^^^^^
    /// ```
    pub code_indented: bool,
    /// Code (fenced).
    ///
    /// ```markdown
    /// > | ~~~js
    ///     ^^^^^
    /// > | console.log(1)
    ///     ^^^^^^^^^^^^^^
    /// > | ~~~
    ///     ^^^
    /// ```
    pub code_fenced: bool,
    /// Code (text).
    ///
    /// ```markdown
    /// > | a `b` c
    ///       ^^^
    /// ```
    pub code_text: bool,
    /// Definition.
    ///
    /// ```markdown
    /// > | [a]: b "c"
    ///     ^^^^^^^^^^
    /// ```
    pub definition: bool,
    /// Frontmatter with an unindented opening fence on the first document line,
    /// outside list items and blockquotes, and a matching closing fence.
    ///
    /// Frontmatter is language-agnostic, following [Markly](https://github.com/socketry/markly) and [cmarkly](https://github.com/socketry/cmarkly). All forms produce a source-backed opaque node with [`NodeSyntax::Frontmatter`][crate::NodeSyntax::Frontmatter] metadata. Untagged `---` and `+++` imply YAML and TOML, with an empty info string. A format hint after either delimiter (such as `--- json` or `---yaml`), or a language-tagged backtick/tilde code fence, supplies an explicit language. The parser preserves the raw body and the full authored info string without parsing the named format; [`Document::frontmatter_value`][crate::Document::frontmatter_value] returns the body. HTML omits the frontmatter. Backtick/tilde fences without a language, fences later in a document, and unclosed fences remain ordinary Markdown.
    ///
    /// Markdown serialization retains the stored info string and body while normalizing spacing around the info string. It can lengthen fences or change their marker when edited content would close them prematurely, writing an implied language explicitly when replacing an untagged dash/plus delimiter, and adds a line ending before the closing fence when an edited body needs one. Round trips preserve the body and effective language.
    ///
    /// ````
    /// use base_markdown::{Constructs, Document, NodeSyntax, ParseOptions};
    ///
    /// let options = ParseOptions {
    ///     constructs: Constructs { frontmatter: true, ..Constructs::default() },
    ///     ..ParseOptions::default()
    /// };
    /// let document = Document::parse("```json title=example\n{\"draft\": true}\n```\n\n# Hello", &options).unwrap();
    /// let node = document.children(document.root()).next().unwrap();
    /// let Some(NodeSyntax::Frontmatter { info, .. }) = document.syntax(node) else { panic!() };
    /// assert_eq!(info.resolve(document.source()).unwrap(), "json title=example");
    /// assert_eq!(document.frontmatter_value(node), Some("{\"draft\": true}\n"));
    /// ````
    ///
    /// ````markdown
    /// > | ---
    ///     ^^^
    /// > | title: Neptune
    ///     ^^^^^^^^^^^^^^
    /// > | ---
    ///     ^^^
    /// ````
    pub frontmatter: bool,
    /// Autolink literal.
    ///
    /// ```markdown
    /// > | https://example.com
    ///     ^^^^^^^^^^^^^^^^^^^
    /// ```
    ///
    /// `http`, `https`, `ftp`, `sftp`, `ssh`, `irc` and `ircs` followed by `://` and a domain, `mailto:` and `xmpp:` followed by an email address (`xmpp:` also with an optional `/resource`), `www.` domains and email addresses form links, with schemes in any case. `tel:` and `sms:` link only in angle brackets.
    pub autolink_literal: bool,
    /// Footnote definition.
    ///
    /// ```markdown
    /// > | [^a]: b
    ///     ^^^^^^^
    /// ```
    pub footnote_definition: bool,
    /// Footnote label start.
    ///
    /// ```markdown
    /// > | a[^b]
    ///      ^^
    /// ```
    pub label_start_footnote: bool,
    /// Strikethrough.
    ///
    /// ```markdown
    /// > | a ~b~ c.
    ///       ^^^
    /// ```
    pub strikethrough: bool,
    /// Table.
    ///
    /// ```markdown
    /// > | | a |
    ///     ^^^^^
    /// > | | - |
    ///     ^^^^^
    /// > | | b |
    ///     ^^^^^
    /// ```
    pub table: bool,
    /// Task list item.
    ///
    /// ```markdown
    /// > | * [x] y.
    ///       ^^^
    /// ```
    pub task_list_item: bool,
    /// Hard break (escape).
    ///
    /// ```markdown
    /// > | a\
    ///      ^
    ///   | b
    /// ```
    pub hard_break_escape: bool,
    /// Hard break (trailing).
    ///
    /// ```markdown
    /// > | a␠␠
    ///      ^^
    ///   | b
    /// ```
    pub hard_break_trailing: bool,
    /// Heading (atx).
    ///
    /// ```markdown
    /// > | # a
    ///     ^^^
    /// ```
    pub heading_atx: bool,
    /// Heading (setext).
    ///
    /// ```markdown
    /// > | a
    ///     ^^
    /// > | ==
    ///     ^^
    /// ```
    pub heading_setext: bool,
    /// HTML (flow).
    ///
    /// ```markdown
    /// > | <div>
    ///     ^^^^^
    /// ```
    pub html_flow: bool,
    /// HTML (text).
    ///
    /// ```markdown
    /// > | a <b> c
    ///       ^^^
    /// ```
    pub html_text: bool,
    /// Label start (image).
    ///
    /// ```markdown
    /// > | a ![b](c) d
    ///       ^^
    /// ```
    pub label_start_image: bool,
    /// Label start (link).
    ///
    /// ```markdown
    /// > | a [b](c) d
    ///       ^
    /// ```
    pub label_start_link: bool,
    /// Label end.
    ///
    /// ```markdown
    /// > | a [b](c) d
    ///         ^^^^
    /// ```
    pub label_end: bool,
    /// List items.
    ///
    /// ```markdown
    /// > | * a
    ///     ^^^
    /// ```
    pub list_item: bool,
    /// Math (flow).
    ///
    /// ```markdown
    /// > | $$
    ///     ^^
    /// > | \frac{1}{2}
    ///     ^^^^^^^^^^^
    /// > | $$
    ///     ^^
    /// ```
    pub math_flow: bool,
    /// Math (text).
    ///
    /// ```markdown
    /// > | a $b$ c
    ///       ^^^
    /// ```
    pub math_text: bool,

    /// Thematic break.
    ///
    /// ```markdown
    /// > | ***
    ///     ^^^
    /// ```
    pub thematic_break: bool,
}

impl Default for Constructs {
    /// `CommonMark`.
    ///
    /// `CommonMark` is a relatively strong specification of how markdown
    /// works.
    /// Most markdown parsers try to follow it.
    ///
    /// For more information, see the `CommonMark` specification:
    /// <https://spec.commonmark.org>.
    fn default() -> Self {
        Self {
            attention: true,
            autolink: true,
            block_quote: true,
            character_escape: true,
            character_reference: true,
            code_indented: true,
            code_fenced: true,
            code_text: true,
            definition: true,
            frontmatter: false,
            autolink_literal: false,
            label_start_footnote: false,
            footnote_definition: false,
            strikethrough: false,
            table: false,
            task_list_item: false,
            hard_break_escape: true,
            hard_break_trailing: true,
            heading_atx: true,
            heading_setext: true,
            html_flow: true,
            html_text: true,
            label_start_image: true,
            label_start_link: true,
            label_end: true,
            list_item: true,
            math_flow: false,
            math_text: false,

            thematic_break: true,
        }
    }
}

impl Constructs {
    /// GFM.
    ///
    /// GFM stands for **GitHub flavored markdown**.
    /// GFM extends `CommonMark` and adds support for autolink literals,
    /// footnotes, strikethrough, tables, and tasklists.
    ///
    /// For more information, see the GFM specification:
    /// <https://github.github.com/gfm/>.
    ///
    /// Parsing differs from GitHub in a few deliberate ways, which
    /// [`Options::gfm`](crate::Options::gfm) lists.
    pub fn gfm() -> Self {
        Self {
            autolink_literal: true,
            footnote_definition: true,
            label_start_footnote: true,
            strikethrough: true,
            table: true,
            task_list_item: true,
            ..Self::default()
        }
    }
}

/// Configuration that describes how to compile to HTML.
///
/// You likely either want to turn on the dangerous options
/// (`allow_dangerous_html`, `allow_dangerous_protocol`) when dealing with
/// input you trust, or want to customize how footnotes are compiled
/// (typically because the input markdown is not in English).
///
/// ## Examples
///
/// ```
/// use base_markdown::CompileOptions;
/// # fn main() {
///
/// // Use the default trait to get safe defaults:
/// let safe = CompileOptions::default();
///
/// // Live dangerously / trust the author:
/// let danger = CompileOptions {
///   allow_dangerous_html: true,
///   allow_dangerous_protocol: true,
///   ..CompileOptions::default()
/// };
///
/// // In French:
/// let enFrançais = CompileOptions {
///   footnote_back_label: Some("Arrière".into()),
///   footnote_label: Some("Notes de bas de page".into()),
///   ..CompileOptions::default()
/// };
/// # }
/// ```
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, Default)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(default, rename_all = "camelCase")
)]
pub struct CompileOptions {
    /// Whether to allow all values in images.
    ///
    /// The default is `false`,
    /// which lets `allow_dangerous_protocol` control protocol safety for
    /// both links and images.
    ///
    /// Pass `true` to allow all values as `src` on images,
    /// regardless of `allow_dangerous_protocol`.
    /// This is safe because the
    /// [HTML specification][whatwg-html-image-processing]
    /// does not allow executable code in images.
    ///
    /// [whatwg-html-image-processing]: https://html.spec.whatwg.org/multipage/images.html#images-processing-model
    ///
    /// ## Examples
    ///
    /// ```
    /// use base_markdown::{to_html_with_options, CompileOptions, Options};
    ///
    /// // By default, some protocols in image sources are dropped:
    /// assert_eq!(
    ///     to_html_with_options(
    ///         "![](data:image/gif;base64,R0lGODlhAQABAAAAACH5BAEKAAEALAAAAAABAAEAAAICTAEAOw==)",
    ///         &Options::default()
    ///     ),
    ///     "<p><img src=\"\" alt=\"\" /></p>"
    /// );
    ///
    /// // Turn `allow_any_img_src` on to allow all values as `src` on images.
    /// // This is safe because browsers do not execute code in images.
    /// assert_eq!(
    ///     to_html_with_options(
    ///         "![](javascript:alert(1))",
    ///         &Options {
    ///             compile: CompileOptions {
    ///               allow_any_img_src: true,
    ///               ..CompileOptions::default()
    ///             },
    ///             ..Options::default()
    ///         }
    ///     ),
    ///     "<p><img src=\"javascript:alert(1)\" alt=\"\" /></p>"
    /// );
    /// ```
    pub allow_any_img_src: bool,

    /// Whether to allow (dangerous) HTML.
    ///
    /// The default is `false`, which still parses the HTML according to
    /// `CommonMark` but shows the HTML as text instead of as elements.
    ///
    /// Pass `true` for trusted content to get actual HTML elements.
    ///
    /// When using GFM, make sure to also turn off `tagfilter`.
    /// Otherwise, some dangerous HTML is still ignored.
    ///
    /// ## Examples
    ///
    /// ```
    /// use base_markdown::{to_html, to_html_with_options, CompileOptions, Options};
    ///
    /// // `markdown-rs` is safe by default:
    /// assert_eq!(
    ///     to_html("Hi, <i>venus</i>!"),
    ///     "<p>Hi, &lt;i&gt;venus&lt;/i&gt;!</p>"
    /// );
    ///
    /// // Turn `allow_dangerous_html` on to allow potentially dangerous HTML:
    /// assert_eq!(
    ///     to_html_with_options(
    ///         "Hi, <i>venus</i>!",
    ///         &Options {
    ///             compile: CompileOptions {
    ///               allow_dangerous_html: true,
    ///               ..CompileOptions::default()
    ///             },
    ///             ..Options::default()
    ///         }
    ///     ),
    ///     "<p>Hi, <i>venus</i>!</p>"
    /// );
    /// ```
    pub allow_dangerous_html: bool,

    /// Whether to allow dangerous protocols in links and images.
    ///
    /// The default is `false`, which drops URLs in links and images that use
    /// dangerous protocols.
    ///
    /// Pass `true` for trusted content to support all protocols.
    ///
    /// URLs that have no protocol (which means it’s relative to the current
    /// page, such as `./some/page.html`) and URLs that have a safe protocol
    /// (for images: `http`, `https`; for links: the autolink schemes, `http`,
    /// `https`, `ftp`, `sftp`, `ssh`, `irc`, `ircs`, `mailto`, `xmpp`, `tel`
    /// and `sms`), are safe.
    /// All other URLs are dangerous and dropped.
    ///
    /// When the option `allow_all_protocols_in_img` is enabled,
    /// `allow_dangerous_protocol` only applies to links.
    ///
    /// This is safe because the
    /// [HTML specification][whatwg-html-image-processing]
    /// does not allow executable code in images.
    /// All modern browsers respect this.
    ///
    /// [whatwg-html-image-processing]: https://html.spec.whatwg.org/multipage/images.html#images-processing-model
    ///
    /// ## Examples
    ///
    /// ```
    /// use base_markdown::{to_html, to_html_with_options, CompileOptions, Options};
    ///
    /// // `markdown-rs` is safe by default:
    /// assert_eq!(
    ///     to_html("[a](javascript:alert(1))"),
    ///     "<p><a href=\"\">a</a></p>"
    /// );
    ///
    /// // Turn `allow_dangerous_protocol` on to allow potentially dangerous protocols:
    /// assert_eq!(
    ///     to_html_with_options(
    ///         "[a](javascript:alert(1))",
    ///         &Options {
    ///             compile: CompileOptions {
    ///               allow_dangerous_protocol: true,
    ///               ..CompileOptions::default()
    ///             },
    ///             ..Options::default()
    ///         }
    ///     ),
    ///     "<p><a href=\"javascript:alert(1)\">a</a></p>"
    /// );
    /// ```
    pub allow_dangerous_protocol: bool,

    // To do: `doc_markdown` is broken.
    #[allow(clippy::doc_markdown)]
    /// Default line ending to use when compiling to HTML, for line endings not
    /// in `value`.
    ///
    /// Parsed values store every line ending as `\n`, whether the document
    /// uses `\n`, `\r\n` or `\r`, so line endings in content and between
    /// blocks are compiled as `\n`.
    /// In some cases, such as `> a`, CommonMark requires that extra line
    /// endings are added: `<blockquote>\n<p>a</p>\n</blockquote>`.
    ///
    /// Those are `\n` too when the document has a line ending.
    /// If there is no line ending, `default_line_ending` is used.
    /// If that isn’t configured, `\n` is used.
    ///
    /// ## Examples
    ///
    /// ```
    /// use base_markdown::{to_html, to_html_with_options, CompileOptions, LineEnding, Options};
    ///
    /// // `markdown-rs` uses `\n` by default:
    /// assert_eq!(
    ///     to_html("> a"),
    ///     "<blockquote>\n<p>a</p>\n</blockquote>"
    /// );
    ///
    /// // Define `default_line_ending` to configure the default:
    /// assert_eq!(
    ///     to_html_with_options(
    ///         "> a",
    ///         &Options {
    ///             compile: CompileOptions {
    ///               default_line_ending: LineEnding::CarriageReturnLineFeed,
    ///               ..CompileOptions::default()
    ///             },
    ///             ..Options::default()
    ///         }
    ///     ),
    ///     "<blockquote>\r\n<p>a</p>\r\n</blockquote>"
    /// );
    /// ```
    pub default_line_ending: LineEnding,

    /// Textual label to describe the backreference back to footnote calls.
    ///
    /// The default value is `"Back to content"`.
    /// Change it when the markdown is not in English.
    ///
    /// This label is used in the `aria-label` attribute on each backreference
    /// (the `↩` links).
    /// It affects users of assistive technology.
    ///
    /// ## Examples
    ///
    /// ```
    /// use base_markdown::{to_html_with_options, CompileOptions, Options, ParseOptions};
    ///
    /// // `"Back to content"` is used by default:
    /// assert_eq!(
    ///     to_html_with_options(
    ///         "[^a]\n\n[^a]: b",
    ///         &Options::gfm()
    ///     ),
    ///     "<p><sup><a href=\"#user-content-fn-a\" id=\"user-content-fnref-a\" data-footnote-ref=\"\" aria-describedby=\"footnote-label\">1</a></sup></p>\n<section data-footnotes=\"\" class=\"footnotes\"><h2 id=\"footnote-label\" class=\"sr-only\">Footnotes</h2>\n<ol>\n<li id=\"user-content-fn-a\">\n<p>b <a href=\"#user-content-fnref-a\" data-footnote-backref=\"\" aria-label=\"Back to content\" class=\"data-footnote-backref\">↩</a></p>\n</li>\n</ol>\n</section>\n"
    /// );
    ///
    /// // Pass `footnote_back_label` to use something else:
    /// assert_eq!(
    ///     to_html_with_options(
    ///         "[^a]\n\n[^a]: b",
    ///         &Options {
    ///             parse: ParseOptions::gfm(),
    ///             compile: CompileOptions {
    ///               footnote_back_label: Some("Arrière".into()),
    ///               ..CompileOptions::gfm()
    ///             }
    ///         }
    ///     ),
    ///     "<p><sup><a href=\"#user-content-fn-a\" id=\"user-content-fnref-a\" data-footnote-ref=\"\" aria-describedby=\"footnote-label\">1</a></sup></p>\n<section data-footnotes=\"\" class=\"footnotes\"><h2 id=\"footnote-label\" class=\"sr-only\">Footnotes</h2>\n<ol>\n<li id=\"user-content-fn-a\">\n<p>b <a href=\"#user-content-fnref-a\" data-footnote-backref=\"\" aria-label=\"Arrière\" class=\"data-footnote-backref\">↩</a></p>\n</li>\n</ol>\n</section>\n"
    /// );
    /// ```
    pub footnote_back_label: Option<String>,

    /// Prefix to use before the `id` attribute on footnotes to prevent them
    /// from *clobbering*.
    ///
    /// The default is `"user-content-"`.
    /// Pass `Some("".into())` for trusted markdown and when you are careful
    /// with polyfilling.
    /// You could pass a different prefix.
    ///
    /// DOM clobbering is this:
    ///
    /// ```html
    /// <p id="x"></p>
    /// <script>alert(x) // `x` now refers to the `p#x` DOM element</script>
    /// ```
    ///
    /// The above example shows that elements are made available by browsers,
    /// by their ID, on the `window` object.
    /// This is a security risk because you might be expecting some other
    /// variable at that place.
    /// It can also break polyfills.
    /// Using a prefix solves these problems.
    ///
    /// ## Examples
    ///
    /// ```
    /// use base_markdown::{to_html_with_options, CompileOptions, Options, ParseOptions};
    ///
    /// // `"user-content-"` is used by default:
    /// assert_eq!(
    ///     to_html_with_options(
    ///         "[^a]\n\n[^a]: b",
    ///         &Options::gfm()
    ///     ),
    ///     "<p><sup><a href=\"#user-content-fn-a\" id=\"user-content-fnref-a\" data-footnote-ref=\"\" aria-describedby=\"footnote-label\">1</a></sup></p>\n<section data-footnotes=\"\" class=\"footnotes\"><h2 id=\"footnote-label\" class=\"sr-only\">Footnotes</h2>\n<ol>\n<li id=\"user-content-fn-a\">\n<p>b <a href=\"#user-content-fnref-a\" data-footnote-backref=\"\" aria-label=\"Back to content\" class=\"data-footnote-backref\">↩</a></p>\n</li>\n</ol>\n</section>\n"
    /// );
    ///
    /// // Pass `footnote_clobber_prefix` to use something else:
    /// assert_eq!(
    ///     to_html_with_options(
    ///         "[^a]\n\n[^a]: b",
    ///         &Options {
    ///             parse: ParseOptions::gfm(),
    ///             compile: CompileOptions {
    ///               footnote_clobber_prefix: Some("".into()),
    ///               ..CompileOptions::gfm()
    ///             }
    ///         }
    ///     ),
    ///     "<p><sup><a href=\"#fn-a\" id=\"fnref-a\" data-footnote-ref=\"\" aria-describedby=\"footnote-label\">1</a></sup></p>\n<section data-footnotes=\"\" class=\"footnotes\"><h2 id=\"footnote-label\" class=\"sr-only\">Footnotes</h2>\n<ol>\n<li id=\"fn-a\">\n<p>b <a href=\"#fnref-a\" data-footnote-backref=\"\" aria-label=\"Back to content\" class=\"data-footnote-backref\">↩</a></p>\n</li>\n</ol>\n</section>\n"
    /// );
    /// ```
    pub footnote_clobber_prefix: Option<String>,

    /// Attributes to use on the footnote label.
    ///
    /// The default value is `"class=\"sr-only\""`.
    /// Change it to show the label and add other attributes.
    ///
    /// This label is typically hidden visually (assuming a `sr-only` CSS class
    /// is defined that does that), and thus affects screen readers only.
    /// If you do have such a class, but want to show this section to everyone,
    /// pass an empty string.
    /// You can also add different attributes.
    ///
    /// > 👉 **Note**: `id="footnote-label"` is always added, because footnote
    /// > calls use it with `aria-describedby` to provide an accessible label.
    ///
    /// ## Examples
    ///
    /// ```
    /// use base_markdown::{to_html_with_options, CompileOptions, Options, ParseOptions};
    ///
    /// // `"class=\"sr-only\""` is used by default:
    /// assert_eq!(
    ///     to_html_with_options(
    ///         "[^a]\n\n[^a]: b",
    ///         &Options::gfm()
    ///     ),
    ///     "<p><sup><a href=\"#user-content-fn-a\" id=\"user-content-fnref-a\" data-footnote-ref=\"\" aria-describedby=\"footnote-label\">1</a></sup></p>\n<section data-footnotes=\"\" class=\"footnotes\"><h2 id=\"footnote-label\" class=\"sr-only\">Footnotes</h2>\n<ol>\n<li id=\"user-content-fn-a\">\n<p>b <a href=\"#user-content-fnref-a\" data-footnote-backref=\"\" aria-label=\"Back to content\" class=\"data-footnote-backref\">↩</a></p>\n</li>\n</ol>\n</section>\n"
    /// );
    ///
    /// // Pass `footnote_label_attributes` to use something else:
    /// assert_eq!(
    ///     to_html_with_options(
    ///         "[^a]\n\n[^a]: b",
    ///         &Options {
    ///             parse: ParseOptions::gfm(),
    ///             compile: CompileOptions {
    ///               footnote_label_attributes: Some("class=\"footnote-heading\"".into()),
    ///               ..CompileOptions::gfm()
    ///             }
    ///         }
    ///     ),
    ///     "<p><sup><a href=\"#user-content-fn-a\" id=\"user-content-fnref-a\" data-footnote-ref=\"\" aria-describedby=\"footnote-label\">1</a></sup></p>\n<section data-footnotes=\"\" class=\"footnotes\"><h2 id=\"footnote-label\" class=\"footnote-heading\">Footnotes</h2>\n<ol>\n<li id=\"user-content-fn-a\">\n<p>b <a href=\"#user-content-fnref-a\" data-footnote-backref=\"\" aria-label=\"Back to content\" class=\"data-footnote-backref\">↩</a></p>\n</li>\n</ol>\n</section>\n"
    /// );
    /// ```
    pub footnote_label_attributes: Option<String>,

    /// HTML tag name to use for the footnote label element.
    ///
    /// The default value is `"h2"`.
    /// Change it to match your document structure.
    ///
    /// This label is typically hidden visually (assuming a `sr-only` CSS class
    /// is defined that does that), and thus affects screen readers only.
    /// If you do have such a class, but want to show this section to everyone,
    /// pass different attributes with the `footnote_label_attributes`
    /// option.
    ///
    /// ## Examples
    ///
    /// ```
    /// use base_markdown::{to_html_with_options, CompileOptions, Options, ParseOptions};
    ///
    /// // `"h2"` is used by default:
    /// assert_eq!(
    ///     to_html_with_options(
    ///         "[^a]\n\n[^a]: b",
    ///         &Options::gfm()
    ///     ),
    ///     "<p><sup><a href=\"#user-content-fn-a\" id=\"user-content-fnref-a\" data-footnote-ref=\"\" aria-describedby=\"footnote-label\">1</a></sup></p>\n<section data-footnotes=\"\" class=\"footnotes\"><h2 id=\"footnote-label\" class=\"sr-only\">Footnotes</h2>\n<ol>\n<li id=\"user-content-fn-a\">\n<p>b <a href=\"#user-content-fnref-a\" data-footnote-backref=\"\" aria-label=\"Back to content\" class=\"data-footnote-backref\">↩</a></p>\n</li>\n</ol>\n</section>\n"
    /// );
    ///
    /// // Pass `footnote_label_tag_name` to use something else:
    /// assert_eq!(
    ///     to_html_with_options(
    ///         "[^a]\n\n[^a]: b",
    ///         &Options {
    ///             parse: ParseOptions::gfm(),
    ///             compile: CompileOptions {
    ///               footnote_label_tag_name: Some("h1".into()),
    ///               ..CompileOptions::gfm()
    ///             }
    ///         }
    ///     ),
    ///     "<p><sup><a href=\"#user-content-fn-a\" id=\"user-content-fnref-a\" data-footnote-ref=\"\" aria-describedby=\"footnote-label\">1</a></sup></p>\n<section data-footnotes=\"\" class=\"footnotes\"><h1 id=\"footnote-label\" class=\"sr-only\">Footnotes</h1>\n<ol>\n<li id=\"user-content-fn-a\">\n<p>b <a href=\"#user-content-fnref-a\" data-footnote-backref=\"\" aria-label=\"Back to content\" class=\"data-footnote-backref\">↩</a></p>\n</li>\n</ol>\n</section>\n"
    /// );
    /// ```
    pub footnote_label_tag_name: Option<String>,

    /// Textual label to use for the footnotes section.
    ///
    /// The default value is `"Footnotes"`.
    /// Change it when the markdown is not in English.
    ///
    /// This label is typically hidden visually (assuming a `sr-only` CSS class
    /// is defined that does that), and thus affects screen readers only.
    /// If you do have such a class, but want to show this section to everyone,
    /// pass different attributes with the `footnote_label_attributes`
    /// option.
    ///
    /// ## Examples
    ///
    /// ```
    /// use base_markdown::{to_html_with_options, CompileOptions, Options, ParseOptions};
    ///
    /// // `"Footnotes"` is used by default:
    /// assert_eq!(
    ///     to_html_with_options(
    ///         "[^a]\n\n[^a]: b",
    ///         &Options::gfm()
    ///     ),
    ///     "<p><sup><a href=\"#user-content-fn-a\" id=\"user-content-fnref-a\" data-footnote-ref=\"\" aria-describedby=\"footnote-label\">1</a></sup></p>\n<section data-footnotes=\"\" class=\"footnotes\"><h2 id=\"footnote-label\" class=\"sr-only\">Footnotes</h2>\n<ol>\n<li id=\"user-content-fn-a\">\n<p>b <a href=\"#user-content-fnref-a\" data-footnote-backref=\"\" aria-label=\"Back to content\" class=\"data-footnote-backref\">↩</a></p>\n</li>\n</ol>\n</section>\n"
    /// );
    ///
    /// // Pass `footnote_label` to use something else:
    /// assert_eq!(
    ///     to_html_with_options(
    ///         "[^a]\n\n[^a]: b",
    ///         &Options {
    ///             parse: ParseOptions::gfm(),
    ///             compile: CompileOptions {
    ///               footnote_label: Some("Notes de bas de page".into()),
    ///               ..CompileOptions::gfm()
    ///             }
    ///         }
    ///     ),
    ///     "<p><sup><a href=\"#user-content-fn-a\" id=\"user-content-fnref-a\" data-footnote-ref=\"\" aria-describedby=\"footnote-label\">1</a></sup></p>\n<section data-footnotes=\"\" class=\"footnotes\"><h2 id=\"footnote-label\" class=\"sr-only\">Notes de bas de page</h2>\n<ol>\n<li id=\"user-content-fn-a\">\n<p>b <a href=\"#user-content-fnref-a\" data-footnote-backref=\"\" aria-label=\"Back to content\" class=\"data-footnote-backref\">↩</a></p>\n</li>\n</ol>\n</section>\n"
    /// );
    /// ```
    pub footnote_label: Option<String>,

    /// Whether or not task list HTML `<input>` items are enabled.
    ///
    /// This determines whether or not the user of the browser is able
    /// to click and toggle generated checkbox items. The default is false.
    ///
    /// ## Examples
    ///
    /// ```
    /// use base_markdown::{to_html_with_options, CompileOptions, Options, ParseOptions};
    ///
    /// // With `task_list_item_checkable`, generated `<input type="checkbox" />`
    /// // tags do not contain the attribute `disabled=""` and are thus toggleable by
    /// // browser users.
    /// assert_eq!(
    ///     to_html_with_options(
    ///         "* [x] y.",
    ///         &Options {
    ///             parse: ParseOptions::gfm(),
    ///             compile: CompileOptions {
    ///                 task_list_item_checkable: true,
    ///                 ..CompileOptions::gfm()
    ///             }
    ///         }
    ///     ),
    ///     "<ul>\n<li><input type=\"checkbox\" checked=\"\" /> y.</li>\n</ul>"
    /// );
    /// ```
    pub task_list_item_checkable: bool,

    /// Whether to filter disallowed raw HTML tags.
    ///
    /// This option does nothing if `allow_dangerous_html` is not turned on.
    /// The default is `false`, which does not filter raw HTML tags.
    /// Pass `true` for output that is a bit closer to GitHub’s actual output.
    ///
    /// The tagfilter is kinda weird and kinda useless.
    /// The tag filter is a naïve attempt at XSS protection.
    /// You should use a proper HTML sanitizing algorithm instead.
    ///
    /// ## Examples
    ///
    /// ```
    /// use base_markdown::{to_html_with_options, CompileOptions, Options, ParseOptions};
    ///
    /// // With `allow_dangerous_html`, `markdown-rs` passes HTML through untouched:
    /// assert_eq!(
    ///     to_html_with_options(
    ///         "<iframe>",
    ///         &Options {
    ///             parse: ParseOptions::gfm(),
    ///             compile: CompileOptions {
    ///               allow_dangerous_html: true,
    ///               ..CompileOptions::default()
    ///             }
    ///         }
    ///     ),
    ///     "<iframe>"
    /// );
    ///
    /// // Pass `tagfilter: true` to make some of that safe:
    /// assert_eq!(
    ///     to_html_with_options(
    ///         "<iframe>",
    ///         &Options {
    ///             parse: ParseOptions::gfm(),
    ///             compile: CompileOptions {
    ///               allow_dangerous_html: true,
    ///               tagfilter: true,
    ///               ..CompileOptions::default()
    ///             }
    ///         }
    ///     ),
    ///     "&lt;iframe>"
    /// );
    /// ```
    ///
    /// ## References
    ///
    /// * [*§ 6.1 Disallowed Raw HTML (extension)* in GFM](https://github.github.com/gfm/#disallowed-raw-html-extension-)
    /// * [`cmark-gfm#extensions/tagfilter.c`](https://github.com/github/cmark-gfm/blob/master/extensions/tagfilter.c)
    pub tagfilter: bool,

    /// Whether to add unique `id` attributes to headings for in-page links
    /// and tables of contents.
    ///
    /// IDs use the same text normalization and duplicate suffixes as
    /// [`Document::headings`][crate::Document::headings].
    ///
    /// The default is `false`.
    pub heading_ids: bool,
}

impl CompileOptions {
    /// GFM.
    ///
    /// GFM stands for **GitHub flavored markdown**.
    /// On the compilation side, the GFM preset turns on the tagfilter.
    /// The tagfilter is useless, but it’s included here for consistency, and
    /// this method exists for parity to parse options.
    ///
    /// For more information, see the GFM specification:
    /// <https://github.github.com/gfm/>.
    ///
    /// HTML output differs from GitHub in a few deliberate ways, such as using
    /// LF line endings, which
    /// [`Options::gfm`](crate::Options::gfm) lists.
    pub fn gfm() -> Self {
        Self {
            tagfilter: true,
            ..Self::default()
        }
    }
}

/// Configuration that describes how to parse from markdown.
///
/// You can use this:
///
/// * To control what markdown constructs are turned on and off
/// * To control some of those constructs
///
/// In most cases, you will want to use the default trait or `gfm` method.
///
/// ## Examples
///
/// ```
/// use base_markdown::ParseOptions;
/// # fn main() {
///
/// // Use the default trait to parse markdown according to `CommonMark`:
/// let commonmark = ParseOptions::default();
///
/// // Use the `gfm` method to parse markdown according to GFM:
/// let gfm = ParseOptions::gfm();
/// # }
/// ```
#[allow(clippy::struct_excessive_bools)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(default, rename_all = "camelCase")
)]
#[derive(Debug)]
pub struct ParseOptions {
    /// Which constructs to enable and disable.
    ///
    /// The default is to follow `CommonMark`.
    ///
    /// ## Examples
    ///
    /// ```
    /// use base_markdown::{to_html, to_html_with_options, Constructs, Options, ParseOptions};
    ///
    /// // `markdown-rs` follows CommonMark by default:
    /// assert_eq!(
    ///     to_html("    indented code?"),
    ///     "<pre><code>indented code?\n</code></pre>"
    /// );
    ///
    /// // Pass `constructs` to choose what to enable and disable:
    /// assert_eq!(
    ///     to_html_with_options(
    ///         "    indented code?",
    ///         &Options {
    ///             parse: ParseOptions {
    ///               constructs: Constructs {
    ///                 code_indented: false,
    ///                 ..Constructs::default()
    ///               },
    ///               ..ParseOptions::default()
    ///             },
    ///             ..Options::default()
    ///         }
    ///     ),
    ///     "<p>indented code?</p>"
    /// );
    /// ```
    #[cfg_attr(feature = "serde", serde(default))]
    pub constructs: Constructs,

    /// Whether to parse a language prefix, such as `ruby:`, before inline code.
    ///
    /// When enabled, the prefix is removed from the surrounding text and stored as the inline code's `language`, and HTML rendering emits a `language-*` class. The default is `false`, which retains `CommonMark` behavior.
    ///
    /// ```
    /// use base_markdown::{to_html_with_options, Options, ParseOptions};
    ///
    /// let options = Options {
    ///     parse: ParseOptions { inline_code_info: true, ..ParseOptions::default() },
    ///     ..Options::default()
    /// };
    /// assert_eq!(
    ///     to_html_with_options("Call rust:`main()`.", &options),
    ///     "<p>Call <code class=\"language-rust\">main()</code>.</p>"
    /// );
    /// ```
    #[cfg_attr(feature = "serde", serde(default))]
    pub inline_code_info: bool,

    /// Whether indented type 6 and type 7 HTML blocks continue across blank lines when later content keeps a consistent indentation.
    ///
    /// This keeps indented HTML content together instead of ending the block at the first blank line. The default is `false`, which follows `CommonMark`'s blank-line rule.
    #[cfg_attr(feature = "serde", serde(default))]
    pub html_block_blank_lines: bool,

    /// Whether `:` is allowed in HTML tag names for namespace prefixes such as `<svg:circle />`, in both block and inline HTML.
    ///
    /// The default is `false` to preserve `CommonMark` parsing of text such as `<m:abc>`.
    #[cfg_attr(feature = "serde", serde(default))]
    pub html_tag_namespaces: bool,

    /// Whether to support strikethrough with a single tilde
    ///
    /// This option does nothing if `strikethrough` is not turned on in
    /// `constructs`.
    /// This option does not affect strikethrough with double tildes.
    ///
    /// The default is `true`, which follows how markdown on `github.com`
    /// works, as strikethrough with single tildes is supported.
    /// Pass `false`, to follow the GFM spec more strictly, by not allowing
    /// strikethrough with single tildes.
    ///
    /// ## Examples
    ///
    /// ```
    /// use base_markdown::{to_html_with_options, Constructs, Options, ParseOptions};
    ///
    /// // `markdown-rs` supports single tildes by default:
    /// assert_eq!(
    ///     to_html_with_options(
    ///         "~a~",
    ///         &Options {
    ///             parse: ParseOptions {
    ///               constructs: Constructs::gfm(),
    ///               ..ParseOptions::default()
    ///             },
    ///             ..Options::default()
    ///         }
    ///     ),
    ///     "<p><del>a</del></p>"
    /// );
    ///
    /// // Pass `strikethrough_single_tilde: false` to turn that off:
    /// assert_eq!(
    ///     to_html_with_options(
    ///         "~a~",
    ///         &Options {
    ///             parse: ParseOptions {
    ///               constructs: Constructs::gfm(),
    ///               strikethrough_single_tilde: false,
    ///               ..ParseOptions::default()
    ///             },
    ///             ..Options::default()
    ///         }
    ///     ),
    ///     "<p>~a~</p>"
    /// );
    /// ```
    #[cfg_attr(feature = "serde", serde(default))]
    pub strikethrough_single_tilde: bool,

    /// Whether to support math (text) with a single dollar
    ///
    /// This option does nothing if `math_text` is not turned on in
    /// `constructs`.
    /// This option does not affect math (text) with two or more dollars.
    ///
    /// The default is `true`, which is more close to how code (text) and
    /// Pandoc work, as it allows math with a single dollar to form.
    /// However, single dollars can interfere with “normal” dollars in text.
    /// Pass `false`, to only allow math (text) to form when two or more
    /// dollars are used.
    /// If you pass `false`, you can still use two or more dollars for text
    /// math.
    ///
    /// ## Examples
    ///
    /// ```
    /// use base_markdown::{to_html_with_options, Constructs, Options, ParseOptions};
    ///
    /// // `markdown-rs` supports single dollars by default:
    /// assert_eq!(
    ///     to_html_with_options(
    ///         "$a$",
    ///         &Options {
    ///             parse: ParseOptions {
    ///               constructs: Constructs {
    ///                 math_text: true,
    ///                 ..Constructs::default()
    ///               },
    ///               ..ParseOptions::default()
    ///             },
    ///             ..Options::default()
    ///         }
    ///     ),
    ///     "<p><code class=\"language-math math-inline\">a</code></p>"
    /// );
    ///
    /// // Pass `math_text_single_dollar: false` to turn that off:
    /// assert_eq!(
    ///     to_html_with_options(
    ///         "$a$",
    ///         &Options {
    ///             parse: ParseOptions {
    ///               constructs: Constructs {
    ///                 math_text: true,
    ///                 ..Constructs::default()
    ///               },
    ///               math_text_single_dollar: false,
    ///               ..ParseOptions::default()
    ///             },
    ///             ..Options::default()
    ///         }
    ///     ),
    ///     "<p>$a$</p>"
    /// );
    /// ```
    #[cfg_attr(feature = "serde", serde(default))]
    pub math_text_single_dollar: bool,
}

impl Default for ParseOptions {
    /// `CommonMark` defaults.
    fn default() -> Self {
        Self {
            constructs: Constructs::default(),
            inline_code_info: false,
            html_block_blank_lines: false,
            html_tag_namespaces: false,
            strikethrough_single_tilde: true,
            math_text_single_dollar: true,
        }
    }
}

impl ParseOptions {
    /// GFM.
    ///
    /// GFM stands for GitHub flavored markdown.
    /// GFM extends `CommonMark` and adds support for autolink literals,
    /// footnotes, strikethrough, tables, and tasklists.
    ///
    /// For more information, see the GFM specification:
    /// <https://github.github.com/gfm/>
    ///
    /// Parsing differs from GitHub in a few deliberate ways, which
    /// [`Options::gfm`](crate::Options::gfm) lists.
    pub fn gfm() -> Self {
        Self {
            constructs: Constructs::gfm(),
            ..Self::default()
        }
    }

    /// The syntax [`normalize_markdown`][crate::normalize_markdown] parses.
    ///
    /// This is GFM with frontmatter and math. Tools that inspect a document
    /// before normalizing it can parse with these options to see the same
    /// structure the normalizer does.
    pub fn normalize() -> Self {
        let mut options = Self::gfm();
        options.constructs.frontmatter = true;
        options.constructs.math_flow = true;
        options.constructs.math_text = true;
        options
    }
}

/// Configuration that describes how to parse from markdown and compile to
/// HTML.
///
/// In most cases, you will want to use the default trait or `gfm` method.
///
/// ## Examples
///
/// ```
/// use base_markdown::Options;
/// # fn main() {
///
/// // Use the default trait to compile markdown to HTML according to `CommonMark`:
/// let commonmark = Options::default();
///
/// // Use the `gfm` method to compile markdown to HTML according to GFM:
/// let gfm = Options::gfm();
/// # }
/// ```
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Default)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(default)
)]
pub struct Options {
    /// Configuration that describes how to parse from markdown.
    pub parse: ParseOptions,
    /// Configuration that describes how to compile to HTML.
    pub compile: CompileOptions,
}

impl Options {
    /// GFM.
    ///
    /// GFM stands for GitHub flavored markdown.
    /// GFM extends `CommonMark` and adds support for autolink literals,
    /// footnotes, strikethrough, tables, and tasklists.
    /// On the compilation side, the GFM preset turns on the tagfilter.
    /// The tagfilter is useless, but it’s included here for consistency.
    ///
    /// For more information, see the GFM specification:
    /// <https://github.github.com/gfm/>
    ///
    /// # Differences from GitHub
    ///
    /// These are deliberate:
    ///
    /// * A `www.` literal needs a domain after the dot: `www. a` and `www.#a` are text.
    /// * Literal domains must be RFC 952 and RFC 1123 host names, with no `_`, no empty label and
    ///   no `-` at either end of a label, though letters of internationalized names are allowed:
    ///   `www.a_b.com` and `a@-b.com` are text, with no shorter link, and `www.例.jp` links.
    /// * A backslash ends a literal: `www.a.b\x` links `www.a.b`, not `http://www.a.b%5C`.
    /// * Literals also link `ftp://`, `sftp://`, `ssh://`, `irc://` and `ircs://`: `ssh://a.b`.
    /// * Angle-bracket autolinks need a scheme from the link-scheme table, or an email address:
    ///   `<gopher://a>` and `Option<std::io::Error>` are not links.
    /// * A `*` or `_` run next to `~` is not flanking on that side: `a*~b~*c` has no emphasis.
    /// * HTML output uses LF line endings, whatever the source used: `a\r\nb` renders `a\nb`.
    pub fn gfm() -> Self {
        Self {
            parse: ParseOptions::gfm(),
            compile: CompileOptions::gfm(),
        }
    }
}

#[cfg(test)]
#[path = "configuration/tests.rs"]
mod tests;
