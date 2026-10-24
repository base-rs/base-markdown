# Markdown Parser Architecture

This crate exposes a pull-based semantic event contract shared by direct HTML rendering and a source-backed arena AST. Ordinary CommonMark and GFM use compact block records and reusable inline candidates, including common raw HTML blocks, tables, tasks, strikethrough, and simple literal links. A grammar state machine supplies the compatibility path for extensions and unsupported compact constructs, while all AST construction uses the semantic event stream.

The [Pull Parser Architecture](pull-parser.md) specifies the source-backed semantic event contract and implementation of lazy inline parsing, shared HTML rendering, and incremental arena AST construction.

MDX is no longer supported. JavaScript expressions and import/export statements are ordinary Markdown, and angle-bracket syntax follows CommonMark HTML rules. There are no MDX presets, callback hooks, tokenizer states, or AST variants. Markdown parsing itself has no syntax errors; public result-returning APIs remain compatible while custom event consumers can still report invalid streams or output failures.

## Main paths

- `to_html` and `to_html_with_options` pull `EventParser` events through the shared `EventHtmlRenderer`. This path does not construct an AST.
- `Document::parse` builds a source-backed arena incrementally from semantic events; `Document::events` uses the same HTML writer.
- `Document` is the only AST. Markdown serialization traverses its node IDs directly; HTML rendering traverses semantic events through `EventHtmlRenderer`. Custom renderers consume events or node IDs.
- AST helpers include stack-free traversal, `text_content`, code-fence metadata, subtree detachment/attachment, and heading lookup and section replacement. Detached nodes remain in their document arena and retain stable IDs. See [Document AST and migration](document.md).

The compact producer owns only its inline candidates and reference definitions.
Inline HTML and angle autolinks share byte recognizers with the grammar tokenizer. Complete single-line constructs stay source-backed in the compact producer; rejected, multiline, or incomplete angles retain grammar handling for container prefixes and line endings.

When a region needs grammar tokenization, it transfers those buffers into a
reusable grammar workspace, allocating that workspace once. Both producers share
source-backed value construction and decoding without depending on each other's
representation.

The Markdown serializer writes recursively into one output string. Output ranges
provide preceding context without copying parent strings; reusable scratch handles
indentation and table escaping. Formatting and escaping use byte scanners and static scope rules; regex is a test-only dependency. The escape matcher retains non-overlapping neighbor consumption and uses ASCII digits for ordered-list markers.

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

Run formatting, linting, and tests from the workspace root. The standard test workflow requires 100% source-region coverage for the published `core-markdown` package across all targets and features, then runs the workspace test task. This runs the package's tests again with default features and covers the private Bake and generator packages, which are outside the coverage gate. The crate exposes optional `json`, `log`, and `serde` features. The `generate/` program refreshes generated CommonMark and Unicode data used by the tests.
