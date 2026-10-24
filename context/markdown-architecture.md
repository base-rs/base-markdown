# Markdown Parser Architecture

This crate exposes one pull-based parser and a semantic event contract shared by direct HTML rendering and a source-backed arena AST. A block pass records containers, source fragments, and definitions. One reusable inline scanner resolves a block only when its events are requested. CommonMark, independently enabled extensions, and custom construct configurations use the same implementation.
The [Pull Parser Architecture](pull-parser.md) specifies the source-backed semantic event contract and implementation of lazy inline parsing, shared HTML rendering, and incremental arena AST construction.

Extensions use their syntax names throughout configuration and implementation: `table`, `strikethrough`, `autolink_literal`, `task_list_item`, and footnote constructs. Each can be enabled independently through `Constructs`; `Options::gfm()` remains a convenient preset. See [Extension configuration](extensions.md) for examples and the configuration-name migration.

MDX is no longer supported. JavaScript expressions and import/export statements are ordinary Markdown, and angle-bracket syntax follows CommonMark HTML rules. There are no MDX presets, callback hooks, tokenizer states, or AST variants. Markdown parsing itself has no syntax errors; public result-returning APIs remain compatible while custom event consumers can still report invalid streams or output failures.

## Main paths

- `to_html` and `to_html_with_options` pull `EventParser` events through the shared `EventHtmlRenderer`. This path does not construct an AST.
- `Document::parse` builds a source-backed arena incrementally from semantic events; `Document::events` uses the same HTML writer.
- `Document` is the only AST. Markdown serialization traverses its node IDs directly; HTML rendering traverses semantic events through `EventHtmlRenderer`. Custom renderers consume events or node IDs.
- AST helpers include stack-free traversal, `text_content`, code-fence metadata, subtree detachment/attachment, and heading lookup and section replacement. Detached nodes remain in their document arena and retain stable IDs. See [Document AST and migration](document.md).

The parser owns its compact block arena, prepared definition indexes, and one reusable inline workspace. It does not produce a document-wide grammar token stream or select a compatibility backend. Block and inline syntax handlers consume source bytes directly, and HTML/autolink recognizers are small shared byte state machines.

Ordinary inline regions scan physical source fragments without copying them. Constructs that span stripped container prefixes use a lazily constructed logical view and source-segment map with the same scanner. Semantic values map back to source ranges when contiguous, or materialize only the value that requires normalization. Logical-view offsets never escape in public events.

Reference and footnote indexes normalize raw identifiers before inline resolution. They preserve authored definitions, including duplicates, while lookup selects the first normalized identifier. String decoding honors independently disabled escape and character-reference constructs. Frontmatter, math, tables, tasks, literal links, and inline-code metadata have direct handlers in the block or inline parser.

The Markdown serializer writes recursively into one output string. Output ranges
provide preceding context without copying parent strings; reusable scratch handles
indentation and table escaping. Formatting and escaping use byte scanners and static scope rules; regex is a test-only dependency. The escape matcher retains non-overlapping neighbor consumption and uses ASCII digits for ordered-list markers.

List markers are written directly into the shared output. Ordered items reuse the
current traversal position, making sibling numbering linear in the number of
items. Serializing an attached item on its own scans its source-list siblings once
to retain its original number and uses the configured ordered marker.

Diagnostics use static identifiers, borrowed or owned reasons, and optional
half-open byte spans. `Message::span` replaces the former boxed `Place`; public
`unist` coordinates and the hidden `Location` helper have been removed. Custom
producers can still return `StreamError::Parse(Message)`.

The Markdown serializer lives in `src/markdown/`. Keep its construct handlers and safety logic separate from the parser's event-to-HTML compiler. Serializer changes should preserve references, extensions, and formatting options when possible.

`MarkdownOptions::line_wrapping` controls soft source line breaks in text. `Preserve` is the default and keeps round-trip behavior; `Unwrap` joins those breaks with spaces. Explicit hard-break nodes, code, and block boundaries are not unwrapped. The serializer does not currently enforce a line width.

## Code fences

`EventKind::CodeBlock::fence` retains the opening character, length and indentation in [`CodeFence`](../src/code_fence.rs), independently of node provenance. Indented code and synthetic blocks without fence metadata use `None`. `NodeSyntax::Code` lazily exposes the decoded complete opening info string, while language and semantic body remain on the event. Frontmatter syntax retains both opening and closing fence lengths. Markdown serialization chooses ordinary code formatting from `MarkdownOptions`.

## Frontmatter

`Constructs::frontmatter` recognizes closed fences only at the beginning of a document, outside containers and without opening indentation. Every form produces an opaque leaf with `NodeSyntax::Frontmatter`: untagged `---` and `+++`, tagged delimiter forms (including `---yaml`), and language-tagged backtick or tilde fences.

The node follows Markly/cmarkly's format-agnostic model: syntax `info` is the entire opaque info string, trimmed only at its outer spaces and tabs; `Document::frontmatter_value` returns the raw body, including its trailing line ending. The first info word identifies the language, with `yaml` implied by untagged dash delimiters and `toml` by untagged plus delimiters. The authored info string is empty when the language is implicit. Callers decode the body using the appropriate language parser. Opening and closing fence lengths are retained. Closing code fences follow the usual marker, minimum length, and indentation rules; the Rust extension requires a closing fence for every form.

HTML rendering omits frontmatter. All frontmatter uses one Markdown serialization handler, which preserves info and body while normalizing fence spacing. It lengthens code fences when edited content could close them, and switches conflicting dash/plus delimiters to tildes, making an implicit language explicit in the replacement fence. Edited bodies without a trailing line ending gain one before the closing fence. Keep these safety checks in the Markdown handler, separate from the public AST type.

## Tests and generated data

Integration tests are organized by Markdown construct in `tests/`; keep regressions close to the behavior they cover. `tests/commonmark.rs` exercises the CommonMark corpus, and `tests/html_renderer.rs`, `tests/markdown_renderer.rs`, and `tests/document.rs` cover document rendering and edits. `tests/markdown_serializer_*.rs` contains the Markdown serializer corpus. Its test-only declarative fixture builder converts constructor descriptions into real documents; it is not a second parser or renderer.

Run formatting, linting, and tests from the workspace root. The standard test workflow requires 100% source-region coverage for the published `core-markdown` package across all targets and features, then runs the workspace test task. This runs the package's tests again with default features and covers the private Bake and generator packages, which are outside the coverage gate. The crate exposes optional `json` and `serde` features. The former tokenizer tracing `log` feature remains accepted as a no-op for compatibility. The `generate/` program refreshes generated CommonMark and Unicode data used by the tests.
