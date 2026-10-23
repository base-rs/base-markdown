# `core-markdown`

A CommonMark-compliant Markdown parser for Rust with pull events, a source-backed AST, extensions, and HTML and Markdown renderers. It builds on [`markdown-rs`](https://github.com/wooorm/markdown-rs), with compact CommonMark parsing and Socketry's AST helpers and renderer APIs.

[![Build][badge-build-image]][badge-build-url] [![Coverage][badge-coverage-image]][badge-coverage-url]

## Motivation

Socketry's documentation tooling needs to inspect, edit, and render Markdown sections as structured data. This fork builds on `markdown-rs`'s CommonMark parser with AST editing helpers, reusable renderers, and optional syntax extensions for richer documentation.

The fork adds three parsing options to `ParseOptions`, all disabled by default so ordinary parsing retains CommonMark behavior:

- `inline_code_info` recognizes a language prefix before inline code, such as `` rust:`code` ``. It removes the prefix from the surrounding text, stores the language on the AST node, and emits a `language-rust` class when rendering HTML.
- `html_block_blank_lines` lets ordinary HTML blocks continue across blank lines when subsequent content maintains consistent indentation. This keeps indented HTML content together instead of ending the block at the first blank line.
- `html_tag_namespaces` recognizes namespace-prefixed HTML tags, such as `<svg:circle />`, in both block and inline HTML.

The existing `Constructs::frontmatter` option also supports language-agnostic frontmatter, following [Markly](https://github.com/socketry/markly) and [cmarkly](https://github.com/socketry/cmarkly). A closed, language-tagged backtick or tilde fence at the start of a document becomes frontmatter; format hints on `---` or `+++` do too. The AST preserves the full info string and raw body without parsing the named format, and HTML output omits it.

For tools that inspect code blocks and frontmatter, `Node::code_fence()` exposes the source opening fence's character, length, and indentation, following Markly's fence structure. This metadata remains available when nodes are cloned or detached, even after source positions are removed.

Beyond parsing, the fork adds AST traversal, text extraction, heading and section editing, and `Fragment` nodes for working with detached content. `HtmlRenderer`, `MarkdownRenderer`, and the custom `Renderer` interface render edited trees; Markdown serialization preserves language metadata and can unwrap soft line breaks. Optional `CompileOptions::heading_ids` generates unique heading anchors for in-page links and tables of contents.

The fork also includes fixes for stale MDX parser errors and parsing and serialization edge cases, including CRLF handling in inline code and math. See [releases.md](releases.md) for changes and the [API documentation](https://docs.rs/core-markdown/latest/core_markdown/) for configuration details.

## Usage

### Install

```sh
cargo add core-markdown
```

The library supports Rust 1.73 and later.

### Render HTML

For direct conversion from Markdown source to HTML:

```rust
let html = core_markdown::to_html("## Hello, *world*!");
assert_eq!(html, "<h2>Hello, <em>world</em>!</h2>");
```

Enable GFM constructs with `Options::gfm()`:

```rust
use core_markdown::{to_html_with_options, Options};

let html = to_html_with_options("* [x] done", &Options::gfm()).unwrap();
```

### Pull events and build a source-backed AST

`EventParser` yields resolved semantic events on demand. Direct HTML rendering and the source-backed `Document` AST use this same contract. A document can own the input buffer, while ordinary text values retain byte ranges into it:

```rust
use core_markdown::{render_events, CompileOptions, Document, ParseOptions};

let source = String::from("# Introduction\n\nHello, *world*!");
let document = Document::parse(source, &ParseOptions::default()).unwrap();
let mut html = String::new();
render_events(
    document.source(),
    document.events(),
    &CompileOptions::default(),
    &mut html,
).unwrap();
assert_eq!(html, "<h1>Introduction</h1>\n<p>Hello, <em>world</em>!</p>");
```

`AstBuilder::consume` accepts an event budget so callers can build the AST over several steps without collecting the complete stream. `Document::text_mut` copies a source-backed value only when it is edited, and document traversal allocates nothing. Parsing accepts a complete UTF-8 buffer and prepares block structure before yielding events; inline resolution is deferred until consumption. See the [architecture](context/pull-parser.md) and [performance comparison](benches/comparison/readme.md) for allocation measurements and compatibility paths.

### Work with the legacy mdast API

Parse to a syntax tree when you need to inspect or transform Markdown:

```rust
use core_markdown::{to_mdast, ParseOptions};

let document = to_mdast("# Introduction\n\nHello, *world*!", &ParseOptions::default()).unwrap();
assert_eq!(document.text_content(), "IntroductionHello, world!");
```

Nodes provide traversal, text extraction, code-fence information, heading lookup, and child or section editing. `extract_children()` returns a `Fragment` that can be cloned, rendered, or serialized on its own.

Use `HtmlRenderer` or a custom `Renderer` to render an AST. Serialize nodes and fragments back to Markdown with `Node::to_markdown()`:

```rust
let markdown = document.to_markdown();
assert!(markdown.starts_with("# Introduction"));
```

`MarkdownOptions::line_wrapping` can preserve soft source line breaks or unwrap them into spaces when serializing Markdown.

### Inspect code fences

`Node::code_fence()` exposes the opening marker, length, and indentation, following Markly's fence structure:

```rust
use core_markdown::{to_mdast, ParseOptions};

let document = to_mdast("  ~~~~rust\n  let x = 1;\n  ~~~~", &ParseOptions::default()).unwrap();
let fence = document.children().unwrap()[0].code_fence().unwrap();
assert_eq!(fence.character, '~');
assert_eq!(fence.length, 4);
assert_eq!(fence.indent, 2);
```

Indentation is measured in columns relative to the containing block. Fence metadata survives cloning, detaching nodes, and removing source positions. The helper also supports frontmatter; inline and indented code return `None`. This describes the source opening fence, while ordinary code serialization uses `MarkdownOptions` to choose its output formatting.

### Extensions

The parser inherits support for CommonMark, GFM, MDX, frontmatter, and math constructs from `markdown-rs`. The fork-specific parsing options are described under [Motivation](#motivation). HTML output escapes raw HTML and omits MDX expressions by default. See the [API documentation](https://docs.rs/core-markdown/latest/core_markdown/) for parser options, renderer configuration, and AST types.

### Language-agnostic frontmatter

Enable the existing frontmatter construct to accept any format. The opening fence must be unindented on the first line of the document, outside list items and blockquotes, and have a matching closing fence:

````rust
use core_markdown::{mdast::Node, to_mdast, Constructs, ParseOptions};

let options = ParseOptions {
    constructs: Constructs { frontmatter: true, ..Constructs::default() },
    ..ParseOptions::default()
};
let document = to_mdast("```json title=example\n{\"draft\": true}\n```\n\n# Hello", &options).unwrap();
if let Node::Frontmatter(frontmatter) = &document.children().unwrap()[0] {
    assert_eq!(frontmatter.language(), Some("json"));
    assert_eq!(frontmatter.info, "json title=example");
    assert_eq!(frontmatter.value, "{\"draft\": true}\n");
}
````

All frontmatter uses `Node::Frontmatter`. Untagged `---` and `+++` imply YAML and TOML respectively, with an empty `info` string. Tagged forms such as `--- json` and `---yaml` supply an explicit language hint. `language()` and `Node::code_language()` return the effective language; `Node::code_info()` returns the authored info string. Backtick/tilde fences without a language, fences later in a document, and unclosed fences remain ordinary Markdown.

Markdown serialization retains the stored info string and body while normalizing spacing around the info string. It can lengthen fences or change their marker when edited content would close them prematurely, and adds a line ending before the closing fence when an edited body needs one. When changing an untagged dash/plus delimiter to a code fence, it writes the implied language explicitly. Round trips preserve the body and effective language while allowing these formatting adjustments.

## Releasing

Prepare a release with `cargo bake cargo:version:patch` (or `minor`, `major`, or `bump --version X.Y.Z`), commit with a `Release: vVERSION` Git trailer, then run `cargo bake cargo:release` and open a pull request. After review and merge, GitHub Actions publishes the release when the configured `crates-io` environment approves it. Follow the shared [Releasing skill](https://github.com/core-rs/core-project/blob/main/context/releasing.md) for the standard process.

## Releases

<!-- bake-readme:releases:start -->

See [releases.md](releases.md) for the full release history.

### v0.1.0

- Publish as `core-markdown` in the `core-rs` organization, with version numbering restarted at `0.1.0`.
- Require a `Release: vVERSION` Git trailer on the final release commit; version changes alone no longer trigger publication.

<!-- bake-readme:releases:end -->

## See Also

- [`markdown-rs`](https://github.com/wooorm/markdown-rs) — the upstream parser.
- [CommonMark](https://commonmark.org/) — the Markdown specification.

## Contributing

Please open an issue or pull request on [GitHub](https://github.com/core-rs/core-markdown).

### Agent Context

Run `cargo bake agent:context:install` to install shared context and skills. Read `.agents/context/index.md` to find relevant guides, follow `agents.md` if present, and apply skills under `.agents/skills/`. The installer preserves repository-owned `agents.md`; it does not create or regenerate that file.

[badge-build-image]: https://github.com/core-rs/core-markdown/actions/workflows/test.yml/badge.svg

[badge-build-url]: https://github.com/core-rs/core-markdown/actions

[badge-coverage-image]: https://github.com/core-rs/core-markdown/actions/workflows/test.yml/badge.svg

[badge-coverage-url]: https://github.com/core-rs/core-markdown/actions/workflows/test.yml
