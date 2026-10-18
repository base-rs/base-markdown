# Markdown Parser Architecture

This crate implements a Markdown state machine that tokenizes source, resolves constructs, and can either compile events directly to HTML or build an mdast syntax tree.

## Main paths

- `to_html` and `to_html_with_options` stream parser events into the built-in HTML compiler. This path does not construct an AST.
- `to_mdast` builds a `mdast::Node` tree. The `Node` enum includes a transparent `Fragment` variant for detached child lists.
- `Renderer` is the AST-to-output interface. `HtmlRenderer` and `MarkdownRenderer` are built in; a custom renderer controls its own output and escaping behavior.
- AST helpers include traversal, `text_content`, code-fence metadata, `extract_children`, and heading lookup and section operations. Keep these helpers working for fragments as well as document nodes.

The Markdown serializer lives in `src/markdown/`. Keep its construct handlers and safety logic separate from the parser's event-to-HTML compiler. Serializer changes should preserve references, extensions, and formatting options when possible.

`MarkdownOptions::line_wrapping` controls soft source line breaks in text. `Preserve` is the default and keeps round-trip behavior; `Unwrap` joins those breaks with spaces. Explicit hard-break nodes, code, and block boundaries are not unwrapped. The serializer does not currently enforce a line width.

## Code fences

`Node::code_fence()` returns `Option<CodeFence>` with `character`, `length`, and `indent`, following Markly's `Node::Fence` structure at [436de44](https://github.com/socketry/markly/blob/436de44bebc5aeeb3684cf8ac6e625fbb3fb6e7b/ext/markly/markly.c). The corresponding regression examples are in [`test/markly/node.rb`](https://github.com/socketry/markly/blob/436de44bebc5aeeb3684cf8ac6e625fbb3fb6e7b/test/markly/node.rb).

Ordinary `Code` nodes retain their opening fence in `fence: Option<CodeFence>` independently of source positions. Indented code and manually constructed or older deserialized code without metadata use `None`. Fence length describes the opening sequence even if the closer is longer or absent. Indentation is measured in columns relative to the container, excluding list and blockquote prefixes; tabs, including partially consumed container tabs, contribute their expanded width. Markdown serialization continues to choose ordinary code formatting from `MarkdownOptions`, so this metadata describes the source rather than the rendered output.

The Rust helper also exposes frontmatter's stored marker and opening length, with zero indentation. Inline code and other nodes return `None`. Keep fence information separate from language and info strings.

## Frontmatter

`Constructs::frontmatter` recognizes closed fences only at the beginning of a document, outside containers and without opening indentation. Every form produces `Node::Frontmatter`: untagged `---` and `+++`, tagged delimiter forms (including `---yaml`), and language-tagged backtick or tilde fences.

The node follows Markly/cmarkly's format-agnostic model: `info` is the entire opaque info string, trimmed only at its outer spaces and tabs; `value` is the raw body, including its trailing line ending. `language()` and `Node::code_language()` return the first word of the info string, defaulting to `yaml` for untagged dash delimiters and `toml` for untagged plus delimiters. `Node::code_info()` returns the authored info string, which is empty when the language is implicit. Callers decode the body using the appropriate language parser. Opening and closing fence lengths are retained. Closing code fences follow the usual marker, minimum length, and indentation rules; the Rust extension requires a closing fence for every form.

Both HTML paths omit frontmatter. All frontmatter uses one Markdown serialization handler, which preserves info and body while normalizing fence spacing. It lengthens code fences when edited content could close them, and switches conflicting dash/plus delimiters to tildes, making an implicit language explicit in the replacement fence. Edited bodies without a trailing line ending gain one before the closing fence. Keep these safety checks in the Markdown handler, separate from the public AST type.

## Tests and generated data

Integration tests are organized by Markdown construct in `tests/`; keep regressions close to the behavior they cover. `tests/commonmark.rs` exercises the CommonMark corpus, and `tests/html_renderer.rs` and `tests/markdown_renderer.rs` cover AST rendering. `tests/markdown_serializer_*.rs` contains the Markdown serializer corpus.

Run formatting, linting, and tests from the workspace root. The standard test workflow requires 100% source-region coverage for the published `core-markdown` package across all targets and features, then runs the workspace test task. This runs the package's tests again with default features and covers the private Bake and generator packages, which are outside the coverage gate. The crate exposes optional `json`, `log`, and `serde` features. The `generate/` program refreshes generated CommonMark and Unicode data used by the tests.
