# `core-markdown`

A CommonMark-compliant Markdown parser for Rust with pull events, a source-backed AST, extensions, and HTML and Markdown renderers. It builds on [`markdown-rs`](https://github.com/wooorm/markdown-rs), with compact CommonMark parsing and a single source-backed document representation.

[![Build][badge-build-image]][badge-build-url] [![Coverage][badge-coverage-image]][badge-coverage-url]

## Motivation

Socketry's documentation tooling needs to inspect, edit, and render Markdown sections as structured data. This fork builds on `markdown-rs`'s CommonMark parser with AST editing helpers, reusable renderers, and optional syntax extensions for richer documentation.

The fork adds three parsing options to `ParseOptions`, all disabled by default so ordinary parsing retains CommonMark behavior:

- `inline_code_info` recognizes a language prefix before inline code, such as `` rust:`code` ``. It removes the prefix from the surrounding text, stores the language on the AST node, and emits a `language-rust` class when rendering HTML.
- `html_block_blank_lines` lets ordinary HTML blocks continue across blank lines when subsequent content maintains consistent indentation. This keeps indented HTML content together instead of ending the block at the first blank line.
- `html_tag_namespaces` recognizes namespace-prefixed HTML tags, such as `<svg:circle />`, in both block and inline HTML.

The existing `Constructs::frontmatter` option also supports language-agnostic frontmatter, following [Markly](https://github.com/socketry/markly) and [cmarkly](https://github.com/socketry/cmarkly). A closed, language-tagged backtick or tilde fence at the start of a document becomes frontmatter; format hints on `---` or `+++` do too. The AST preserves the full info string and raw body without parsing the named format, and HTML output omits it.

The AST stores nodes in a contiguous arena and ordinary text as offsets into one source buffer. `Document` provides traversal, text extraction, heading lookup, section replacement, and checked subtree editing. Both parsed source and edited documents feed the same semantic event contract into the same HTML writer. Markdown serialization preserves reference style, code info strings, and frontmatter; optional heading IDs share their anchors with the table-of-contents helper.

The fork also includes fixes for parsing and serialization edge cases, including CRLF handling in inline code and math. See [releases.md](releases.md) for changes and the [API documentation](https://docs.rs/core-markdown/latest/core_markdown/) for configuration details.

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

### Edit and serialize a document

Nodes use stable `NodeId` indices. Editing text materializes that value; detaching or moving a subtree retains its IDs and source metadata:

```rust
use core_markdown::{to_markdown, Document, HeadingOptions, ParseOptions};

let mut document = Document::parse(String::from("# Introduction\n\nHello"), &ParseOptions::default()).unwrap();
let heading = document.headings(&HeadingOptions::default())[0].node;
let text = document.children(heading).next().unwrap();
document.text_mut(text).unwrap().push_str(" updated");
assert!(to_markdown(&document).unwrap().starts_with("# Introduction updated"));
```

`append_child`, `detach`, and `attach` edit structure without rebuilding the tree. `extract_children` returns detached IDs in the same document. `replace_heading_section` copies replacement content from another document, materializing its source references before that source can be dropped and converting reference links to inline resources to preserve their resolved targets. Detached arena slots remain available until the document is dropped. `render_node` and `to_markdown_node` render individual subtrees.

`NodeSyntax` provides optional authored formatting outside the hot event/node layout. Source metadata is inferred on demand; `set_syntax` stores an explicit override, and `None` disconnects that relationship. `set_kind` replaces semantic content and clears inferred syntax for that node; `text_mut` preserves it. `MarkdownOptions::line_wrapping` preserves soft breaks by default or unwraps them into spaces.

### Inspect code fences

Code-block events retain the opening marker, length, and indentation independently of source offsets:

```rust
use core_markdown::{stream::EventKind, Document, ParseOptions};

let document = Document::parse("  ~~~~rust\n  let x = 1;\n  ~~~~", &ParseOptions::default()).unwrap();
let code = document.children(document.root()).next().unwrap();
if let EventKind::CodeBlock { fence: Some(fence), .. } = &document.node(code).unwrap().kind {
    assert_eq!(fence.character, '~');
    assert_eq!(fence.length, 4);
    assert_eq!(fence.indent, 2);
}
```

Indentation is measured in columns relative to the containing block. `NodeSyntax::Code` exposes the full decoded info string; frontmatter metadata also retains its closing fence length. Ordinary code serialization chooses output formatting from `MarkdownOptions`.

### Extensions

The parser inherits support for CommonMark, GFM, frontmatter, and math constructs from `markdown-rs`. The fork-specific parsing options are described under [Motivation](#motivation). HTML output escapes raw HTML by default. MDX, JSX, and JavaScript expression parsing have been removed to simplify the Markdown pipeline. See the [API documentation](https://docs.rs/core-markdown/latest/core_markdown/) for parser options, renderer configuration, and AST types.

### Language-agnostic frontmatter

Enable the existing frontmatter construct to accept any format. The opening fence must be unindented on the first line of the document, outside list items and blockquotes, and have a matching closing fence:

````rust
use core_markdown::{Document, NodeSyntax, Constructs, ParseOptions};

let options = ParseOptions {
    constructs: Constructs { frontmatter: true, ..Constructs::default() },
    ..ParseOptions::default()
};
let document = Document::parse("```json title=example\n{\"draft\": true}\n```\n\n# Hello", &options).unwrap();
let node = document.children(document.root()).next().unwrap();
if let Some(NodeSyntax::Frontmatter { info, .. }) = document.syntax(node) {
    assert_eq!(info.resolve(document.source()).unwrap(), "json title=example");
    assert_eq!(document.frontmatter_value(node), Some("{\"draft\": true}\n"));
}
````

Frontmatter uses an opaque semantic leaf with `NodeSyntax::Frontmatter` metadata. Untagged `---` and `+++` imply YAML and TOML respectively, with an empty info string. Tagged forms such as `--- json` and `---yaml` supply an explicit language hint. Backtick/tilde fences without a language, fences later in a document, and unclosed fences remain ordinary Markdown.

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
