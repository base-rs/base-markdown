# Document AST and migration

`Document<S>` is the single source-backed arena AST, constructed incrementally from semantic events and rendered through the same HTML writer as direct parsing. `S: AsRef<str>` can own or borrow a complete UTF-8 buffer. Ordinary text and attributes retain offsets; decoded constants and constructed values use literal or owned storage.

## Migrating from mdast

The legacy recursive AST, its compiler and separate HTML renderer have been removed. This deliberately changes the public API and serialized AST representation.

| Previous API | Document API |
| --- | --- |
| `to_mdast(source, options)` | `Document::parse(source, options)` |
| `Node` enum and child vectors | `DocumentNode::kind`, `NodeId`, `Document::children(id)` |
| `walk` | `Document::descendants(id)`, including the starting node |
| `text_content()` | `Document::text_content(id)` |
| `HtmlRenderer` and `render_with` | `Document::render_html`, `render_node`, or `render_events(document.source(), document.events(), ...)` |
| Custom `Renderer` | A consumer of semantic events, or a traversal of document node IDs |
| `Node::to_markdown`, `MarkdownRenderer` | `to_markdown(&document)`, `to_markdown_with_options`, or `to_markdown_node` |
| `Headings::extract` | `Document::headings(&HeadingOptions)` |
| `extract_children` / `Fragment` | `Document::extract_children(parent)` returns detached IDs in the same arena |
| Child-vector mutation | `append_child`, `detach`, and `attach(parent, child, before)` |
| `replace_heading_section` | `Document::replace_heading_section(parent, index, &replacement, replace_heading, remove_subsections)` |
| Code-fence helpers | `EventKind::CodeBlock::fence` and `NodeSyntax::Frontmatter` |

Events and AST nodes express resolved Markdown meaning. References contain their resolved URL/title; images contain semantic alternative-text children. Tables have explicit head/body containers and per-cell alignment. Task markers, soft breaks, and output separators are explicit leaves. Node boundaries follow source chunks rather than guaranteeing one text node per run. Direct-child indices, including the section helper's heading index, include separator nodes. Use IDs or filtered child traversal when source whitespace should not affect selection.

Source spans are half-open byte ranges, rather than legacy unist positions. Parsed ATX and setext heading spans include the terminating source line ending when present: use `node.span.end` directly to locate the heading body, without skipping another CR/LF. Frontmatter spans end at the closing fence before its terminating line ending. Value ranges identify semantic text and can differ from the enclosing node span; use `Text::resolve` and `frontmatter_value` instead of slicing node spans for decoded values. Resolved reference links already honor first-definition precedence, so consumers should inspect their destinations rather than search definitions again.

Markdown serialization preserves supported Markdown structure rather than every possible custom event tree. For example, Markdown tables require a header: a manually edited table without its head is serialized with its first remaining row as the header. Use HTML rendering to preserve such custom table structure. Unsupported phrasing, list, or table child shapes return errors.

## Ownership and editing

Node IDs are scoped to their document. Arena slots remain stable across edits and are not reclaimed on detachment. A detached subtree can be inspected, rendered, serialized or reattached. The arena holds parent/first-child/next-sibling links; traversal needs neither a heap stack nor cloned strings. Appending or moving children may scan sibling chains; editing avoids permanently enlarging every node with editing-only links or indices.

`text_mut` detaches a source-backed value on its first edit. `set_kind` changes semantic content while retaining provenance and structure, and clears authored-syntax inference for that node. Checked edits reject malformed values, cycles, invalid roots, and capture nesting unsupported by the shared HTML writer. Reordering does not rely on arena allocation order matching tree order.

`NodeSyntax` preserves formatting needed for Markdown output without increasing the event or node size. Unmodified parsed syntax is inferred lazily from the original source extent. Explicit overrides live in a sparse map; `set_syntax(id, None)` disconnects inferred metadata. Reference style/labels, footnote spelling, full code info, item spread, and frontmatter fences remain separate from resolved semantic values. Frontmatter's body accessor and first text edit exclude its fences. Replacement sections copied from another document materialize their values and metadata so that the other source can be dropped. Copied reference links/images become inline resources: this preserves resolved URLs/titles even when the destination has conflicting definitions or the source definition was outside the copied subtree. Same-document moves and document cloning retain reference syntax.

The source stays immutable while ranges depend on it. Integer offsets survive moving or reallocating an unchanged buffer but do not track text insertions. Use AST edits for semantic changes, or reparse when replacing the original source.

## Serialization

The `serde` feature serializes the source, arena slots and sparse syntax overrides. Deserialization constructs the arena directly and validates the complete forest, including detached subtrees, before exposing it. Validation rejects invalid UTF-8 ranges, broken or cyclic links, duplicate ownership, illegal roots, invalid event values and incompatible metadata. It does not collect a complete event stream as an intermediate representation.

Source ranges stay ranges. Literal and owned values share the serialized `Value` representation; deserialization owns those strings. Node IDs retain their indices within the restored document. This arena format replaces mdast JSON and is not the mdast interchange schema.

## Diagnostics

`Message` retains its role as the serializer error and custom producer diagnostic. Its `reason` is now `Cow<'static, str>` (`"literal".into()` or `format!("... {value}").into()`), while `source` and `rule_id` are static strings. Use `span: Some(start..end)` for a source byte range, or `span: None` for an option error. This replaces boxed strings and `place: Option<Box<Place>>`. `unist::{Point, Position}`, `message::Place`, and the hidden `Location` utility have been removed; document and event spans remain the location contract.
