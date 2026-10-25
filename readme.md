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

let html = to_html_with_options("* [x] done", &Options::gfm());
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

### v0.2.0

- Fuse inline-title continuation normalization and decoding, share semantic heading text and terminal line-ending rules, and remove infallible API wrappers. `EventParser::new` now returns the parser directly and `to_html_with_options` returns `String`; remove `?`, `.unwrap()` or `.expect(...)` from those calls. This is a breaking API change; checked event consumers, document validation and Markdown serialization retain their errors. See the migration in `context/pull-parser.md`.

- Share Markdown delimiter and link-label rendering, and format HTML list numbers and footnote counters without temporary strings. Preserve rendered output and diagnostics. Remove the no-op `log` feature and the example's unused logger dependency; callers must stop requesting `features = ["log"]`.

- Centralize editable text materialization, avoid temporary run-length allocations for ordinary inline-code and math delimiters, and cache footnote IDs while sharing definition lookup indexing. Preserve output, first-call numbering, duplicate-definition precedence, and source-range errors.

- Remove unused Markdown serializer scopes, share frontmatter fence validation, and reserve heading anchors and suffix counters in one map, preserving output and collision behavior.

- Remove the hidden public `decode_named` and `decode_numeric` compatibility helpers and their HTML 4 entity table. This is a breaking API removal; Markdown character-reference decoding continues to use HTML 5.

- Share reference/footnote index construction and lookup, write footnote backlinks directly through the HTML writer, and borrow Markdown escape context instead of copying it. Skip irrelevant escape scans and reserve fenced-body/closing-marker capacity together, preserving output and diagnostic behavior.

- Share Markdown scratch rewrites and marker-run scanning across code, math, frontmatter and tables. Skip inline normalization copies without line endings and avoid speculative rendering for literal text prefixes, preserving output and diagnostic behavior.

- Write Markdown block separators from static strings, borrow unchanged text when unwrapping soft line breaks, and splice removed heading sections as one sibling range. Preserve output, stable node IDs, detached subtrees, and edit-context hints.

- Consolidate Markdown destination/title formatting and construct-specific option checks. Share AST linking and preorder advancement; copy sections without per-subtree mapping stacks or repeated insertion-position scans, while preserving rendered output, edit semantics, and diagnostics.

- Simplify parser ownership and event delivery: remove the private inline workspace wrapper, keep document completion outside the hot event loop, and share allocation-free block-child traversal while preserving the semantic event and document APIs.

- Scan ordinary inline link destination runs with the shared byte scanner, retaining escape, nesting, whitespace, control-byte and Unicode behavior without additional allocations.

- Skip the intermediate lowercase string when Unicode identifier folding can uppercase directly, preserving reference matching and visible footnote identifiers.

- Borrow canonical ASCII reference keys from source while preserving Unicode folding, whitespace normalization, and first-definition precedence. Represent pending block separators as delivery phases, removing separate state and per-inline separator handling.

- Scan reference-label word runs with the shared byte scanner while preserving whitespace and Unicode folding. Represent inline event delivery as a parser phase instead of a separate flag.

- Reduce ordinary parsing overhead in the unified parser: finish blank paragraphs before block lookahead, dispatch scanners by opening byte, skip disabled extension bookkeeping, and scan definition destinations directly within their physical source line. Keep unchanged raw inline values outside the owned normalization path.

- Use one source-backed pull parser for every construct configuration. Remove the grammar tokenizer, token-edit pipeline, and compatibility dispatch; handle multiline inline syntax, definitions, footnotes, math, and frontmatter directly while retaining the semantic event and document APIs. Correct multiline reference labels inside containers and table recognition after headers or rows beginning with syntax markers.

- Name individual extensions by their syntax: remove `gfm_` from construct flags, parser and HTML options, internal modules, and tokenizer state. For example, `gfm_table` becomes `table` and `gfm_footnote_definitions` becomes `footnote_definitions`. Serialized configuration keys drop the corresponding `gfm` prefix. This is a breaking API and configuration-format rename; the `gfm()` presets and extension behavior remain unchanged. See `context/extensions.md` for migration examples.

- Serialize list markers directly and reuse traversal positions instead of rescanning siblings for each ordered item. Correct the ordered marker when serializing an attached item on its own. Borrow prepared reference indexes during grammar inline tokenization, remove duplicate reference-key collection and the retained parse context, and distinguish document and inline-region lifecycles. Share byte-based source line-ending detection.

- Share inline HTML and angle autolink recognizers between compact and grammar parsing, keeping complete single-line constructs on the compact path. Replace serializer escape regexes with static byte rules and remove the runtime regex dependency. Preserve Unicode digits as ordinary text when escaping ordered-list markers, which require ASCII digits.

- Streamline Markdown serialization with byte scanners, one shared output buffer, and reusable escaping scratch. Keep compact inline storage separate from grammar fallback, transferring it on demand. Simplify diagnostics to borrowed reasons, static identifiers, and byte spans; remove public unist coordinates and the hidden location utility. Serialize Unicode lookalikes in URL schemes as resource links rather than invalid autolinks.

- Make the source-backed `Document` arena the sole AST. Remove `mdast`, `to_mdast`, the separate tree compiler, and the legacy `Renderer`, `HtmlRenderer`, and `MarkdownRenderer` APIs. Markdown serialization now accepts documents or subtree IDs and shares the existing semantic event representation. Add checked subtree editing, heading and section helpers, lazy authored-syntax metadata, and validated arena serialization. This is a breaking API and serialized-AST format change; see the migration guide in `context/document.md`.

- Dispatch compact block recognition and paragraph interruption by opening byte, and skip delimiter resolution for empty inline scopes, preserving output and allocation counts.

- Share source and configuration references between compact block delivery and its reusable inline workspace, removing duplicate parser fields without additional allocations.

- Remove MDX support, including the `Constructs::mdx` and `ParseOptions::mdx` presets, MDX construct fields, parser callbacks and exports, hidden MDX identifier/source-map helpers, and JSX/expression/ESM AST variants. Braces and JavaScript import/export statements are ordinary Markdown; angle-bracket syntax follows CommonMark HTML rules. This is a breaking API change. Remove the JavaScript parser test dependency and tokenizer error machinery used only by MDX.

- Keep the active child cursor in the AST builder instead of every arena node, share bounded character-reference scanning without a separate buffer of decoded values, and stream GFM tag filtering directly to the HTML writer. Use explicit phases for compact document completion.

- Share the reusable inline workspace across compact and grammar parsing, derive paragraph tightness from its containing list, and use the same SIMD HTML encoder for event rendering and legacy helpers. Align local block scanner state to avoid sensitivity to stack layout. Remove duplicate scratch state, paragraph flag rewrites, and a separate scalar escaping loop while preserving output.

- Locate the first matching NEON candidate directly after detection, avoiding scalar rescans while preserving the original loop for chunks without candidates.

- Classify safe URL runs with a static byte lookup and skip empty replacement writes, preserving existing protocol validation and percent encoding.

- Write unescaped HTML spans directly, select the output destination once for spans needing replacements, and skip empty runs around escaped characters without temporary allocations.

- Skip redundant root-container checks and reuse scanned block indentation during compact preparation, preserving source ranges and HTML output.

- Coalesce escaped punctuation and following ordinary source text into a single semantic event and AST text node. Retain source-backed ranges and existing structured link/code metadata without allocating combined strings.

- Batch fixed HTML tags and attribute delimiters in the shared event renderer, reducing writer calls without changing output bytes or adding temporary allocations.

- Shrink reusable inline records by storing literal-link prefixes as small tags, and reduce block event span lookup overhead. Reserve up to 4 KiB for convenience HTML output to reduce early growth while bounding spare capacity for documents with little rendered output.

- Prepare common raw HTML blocks and ordinary GFM blocks/tables in compact source storage. Reuse inline candidates for ordinary GFM syntax, tasks, strikethrough, and simple literal links, avoiding document-wide grammar fallback and per-region token allocations. Preserve complex constructs through conservative fallback. GFM now shares existing compact CommonMark corrections for list tightness, literal code endings, emphasis boundaries, and incomplete fences in lazy paragraphs.

- Prepare ordinary same-line reference definitions in the compact block pass, retaining source-backed metadata and forward-reference resolution. Keep authored definition events, including duplicates, while sorting a separate first-definition-wins lookup index. Share that table with inline parsing and initialize grammar fallback keys only on demand. Preserve definition spans and invisible-definition separators. These documents now use compact container rules for list tightness, indentation tabs, and contiguous indented code; ambiguous and multiline definitions retain grammar preparation.

- Resolve same-line reference links and images in reusable compact inline storage. Reuse ASCII label normalization, binary-search definitions, and retain source-backed destinations and titles. Compile out compact reference handling when no definitions exist. Preserve duplicate-definition precedence and expanded-tab label limits. Apply compact emphasis resolution to reference-bearing paragraphs, correcting cases such as `[a]*a**_` after a defined reference.

- Add a shared pull-based semantic event contract, direct HTML writer, source-backed arena document, and incremental AST builder. Use the same event writer for the default HTML API and document traversal. Ordinary source values retain byte ranges; editing a value detaches it into owned storage.

- Use normalized semantic text when generating opt-in heading IDs, including multiline code and math spans. Balance mixed-list closing tags against their semantic openings, correcting previously mismatched closures after unclosed fences. Restore line positions after failed multiline frontmatter attempts, and keep adjacent setext headings balanced when earlier underline text joins a paragraph. Stop retained HTML blank lines at their enclosing container boundary, and preserve sibling list items when HTML consumes the separating newline. Keep collapsed spaces significant in reference labels, so `[a b]` and `[ab]` resolve independently.

- Prepare ordinary CommonMark blocks directly in a compact source-backed arena, deferring inline work until consumption. Correct sibling lists after unclosed fences, container indentation tabs, and empty unclosed code bodies on this path. Use the document line-ending style for separators inserted before container closing tags. Unsupported definition/HTML forms and other extension configurations retain the compatibility producer.

- Resolve ordinary inline syntax in reusable compact storage with indexed code delimiters, bounded emphasis searches, and source-backed link metadata. Scan inline candidates and HTML escapes with bounded AArch64 NEON lookups and a portable scalar fallback. Keep ordinary leading links on the compact path, and reuse compact inline resolution in eligible grammar-backed paragraphs. Correct emphasis adjacent to escaped delimiter characters.

- Improve Markdown parsing throughput and large-document scaling by indexing and batching event edits, scanning ordinary byte runs, compacting internal events and states, and reusing tokenizer buffers. Retain extension support and ordinary HTML output, with the corrections described above.

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
