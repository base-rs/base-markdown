# `base-markdown`

A Markdown parser for Rust that follows CommonMark, apart from a documented restriction on angle-bracket autolinks, with pull events, a source-backed AST, extensions, and HTML and Markdown renderers. It builds on [`markdown-rs`](https://github.com/wooorm/markdown-rs), with compact CommonMark parsing and a single source-backed document representation. That restriction is deliberate: only a fixed table of schemes, such as `https://`, `mailto:` and `tel:`, and email addresses form angle-bracket autolinks, so text such as `Option<std::io::Error>` stays text.

[![Build][badge-build-image]][badge-build-url] [![Coverage][badge-coverage-image]][badge-coverage-url]

## Motivation

Socketry's documentation tooling needs to inspect, edit, and render Markdown sections as structured data. This fork builds on `markdown-rs` with a pull-based event parser, a single source-backed document AST with editing helpers, shared HTML and Markdown renderers, and optional syntax extensions for richer documentation.

## Usage

```sh
cargo add base-markdown
```

The library supports Rust 1.73 and later.

For direct conversion from Markdown source to HTML:

```rust
let html = base_markdown::to_html("## Hello, *world*!");
assert_eq!(html, "<h2>Hello, <em>world</em>!</h2>");
```

Enable GFM constructs with `Options::gfm()`:

```rust
use base_markdown::{to_html_with_options, Options};

let html = to_html_with_options("* [x] done", &Options::gfm());
```

See the [API documentation](https://docs.rs/base-markdown/latest/base_markdown/) for pull events, document editing, parser options, and frontmatter, the [architecture guide](context/pull-parser.md) for the parser design, and the [performance comparison](benches/comparison/readme.md) for benchmarks against other Markdown engines.

## Releasing

Prepare a release with `cargo bake cargo:version:patch` (or `minor`, `major`, or `bump --version X.Y.Z`), commit it with a `Release: vVERSION` trailer, then run `cargo bake cargo:release` and open a pull request. After review and merge, GitHub Actions publishes the release when the configured `crates-io` environment approves it, then creates or updates the matching GitHub Release from `releases.md`. Follow the shared [Releasing skill](https://github.com/base-rs/base-project/blob/main/context/releasing.md) for the standard process.

## Releases

<!-- bake-readme:releases:start -->

See [releases.md](releases.md) for the full release history.

### v0.1.4

- Give setext underlines, thematic breaks and other interrupting lines precedence over a following GFM delimiter row, as cmark-gfm does. Previously `a\n===\n|-` rendered the underline and delimiter row as a second paragraph instead of a heading followed by `|-`. After a link reference definition, an underline-shaped line followed by a delimiter row now forms a table header, and on a line that closes or opens containers it is a thematic break rather than paragraph text.
- Add `IncrementalParser`, which accepts Markdown in pieces and emits the events of each document-level block once appended text can no longer change it. Its combined output equals `EventParser` events without the document start and end events, except that references resolve only against definitions before the end of their document-level block.

### v0.1.3

- Add `normalize_markdown` and `normalize_markdown_with_options`, which parse Markdown with GFM, frontmatter, and math and serialize it with default formatting and soft line breaks unwrapped. This is the normalization `bake-markdown` applies, so tools that generate Markdown can produce output it leaves unchanged.
- Add `ParseOptions::normalize`, the syntax `normalize_markdown` parses, so tools can inspect a document with the same structure the normalizer sees.
- Indent the content of GFM task list items to the list marker's content column rather than past the checkbox. Nested lists, paragraphs, and code in a task item were previously re-parsed as paragraph text or indented code, so repeated serialization changed or grew the document.
- Keep the blank line between a nested list and a following paragraph, table, definition, or setext heading in the same list item. Without it, the next parse continued the nested list's last paragraph, merging the two.
- Write a heading with soft line breaks as an ATX heading when soft breaks are unwrapped. It was previously written as a setext heading, which the next normalization turned into an ATX heading.
- Leave underscores between letters or digits unescaped when serializing Markdown, as `mdast-util-to-markdown` does, so `io_uring` and `snake_case` no longer become `io\_uring` and `snake\_case`. Such underscores cannot open or close emphasis, so the rendered output is unchanged.

### v0.1.2

- Replace a line ending before inline HTML only when the HTML could start an HTML block there, as `mdast-util-to-markdown` does. Line endings before other tags, such as `<span>` or `</span>`, are now kept, and a hard break before them is no longer lost.
- Keep a hard break before inline HTML that could start an HTML block by indenting the next line four spaces, so it continues the paragraph. Previously the break's line ending was replaced with a space, leaving a literal backslash.

<!-- bake-readme:releases:end -->

## See Also

- [`markdown-rs`](https://github.com/wooorm/markdown-rs) — the upstream parser.
- [CommonMark](https://commonmark.org/) — the Markdown specification.

## Contributing

Please open an issue or pull request on [GitHub](https://github.com/base-rs/base-markdown).

### Agent Context

Run `cargo bake agent:context:install` to install shared context and skills. Use `cargo bake agent:context:list` and `cargo bake agent:context:show` to discover relevant guides, follow `agents.md` if present, and apply skills under `.agents/skills/`. The installer preserves repository-owned `agents.md`; it does not create or regenerate that file.

[badge-build-image]: https://github.com/base-rs/base-markdown/actions/workflows/test.yml/badge.svg

[badge-build-url]: https://github.com/base-rs/base-markdown/actions

[badge-coverage-image]: https://github.com/base-rs/base-markdown/actions/workflows/test.yml/badge.svg

[badge-coverage-url]: https://github.com/base-rs/base-markdown/actions/workflows/test.yml
