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
| Parsing Markdown into a node | `Document::parse_children(parent, markdown, &ParseOptions)` replaces the parent's children |
| `replace_heading_section` | `Document::replace_heading_section(parent, index, &replacement, replace_heading, remove_subsections)` |
| Code-fence helpers | `EventKind::CodeBlock::fence` and `NodeSyntax::Frontmatter` |

Events and AST nodes express resolved Markdown meaning. References contain their resolved URL/title; images contain semantic alternative-text children. Tables have explicit head/body containers and per-cell alignment. A task item's checkbox is `Item { checked }`, and frontmatter is the root's `Tag::Document { frontmatter }`, so neither is a node. Soft breaks and output separators are explicit leaves. Parsed text forms one node per source slice: it is split only at backslash escapes and character references, whose values are not a slice of the source. `Document::normalize` joins adjacent text, removes empty text, merges adjacent emphasis, strong or strikethrough of the same kind, unwraps a link inside a link unless it is an autolink, and gives a table the parser's shape, in linear time; `Document::is_normalized` reports whether it would change anything. Normalize a parsed or edited tree before serializing it to Markdown. Direct-child indices, including the section helper's heading index, include separator nodes. Use IDs or filtered child traversal when source whitespace should not affect selection.

Source spans are half-open byte ranges, rather than legacy unist positions. Parsed ATX and setext heading spans include the terminating source line ending when present: use `node.span.end` directly to locate the heading body, without skipping another CR/LF. Frontmatter spans end at the closing fence before its terminating line ending. Value ranges identify semantic text and can differ from the enclosing node span; use `Text::resolve` and `Document::frontmatter` instead of slicing node spans for decoded values. Resolved reference links already honor first-definition precedence, so consumers should inspect their destinations rather than search definitions again.

Markdown serialization preserves supported Markdown structure rather than every possible valid tree. For example, Markdown tables require a header: normalizing a table without its head makes its first row the header, and gives every row as many cells as the widest. Use HTML rendering to preserve such custom table structure.

## Ownership and editing

Node IDs are scoped to their document. Arena slots remain stable across edits and are not reclaimed on detachment. A detached subtree can be inspected, rendered, serialized or reattached. The arena holds parent/first-child/next-sibling links; traversal needs neither a heap stack nor cloned strings. Normalizing invalidates the IDs of the text and attention it joins into a preceding node; other IDs stay valid. Appending in document order, each node after the subtree before it is complete, takes amortized constant time per child: the last node appended leads to the parent's last child, and a list item's ending is refreshed from the appended child alone. Appending elsewhere scans the parent's children; inserting or moving children may scan sibling chains; editing avoids permanently enlarging every node with editing-only links or indices.

`text_mut` detaches a source-backed value on its first edit. `set_kind` changes semantic content while retaining provenance and structure, and clears authored-syntax inference for that node. Checked edits reject malformed values, cycles and invalid roots.

## Parsing into a node

`Document::parse_children(parent, markdown, &options)` parses a fragment and replaces all of the parent's children with it; the old children are detached as `extract_children` leaves them. The parent decides the parse: one that holds phrasing (paragraph, heading, table cell, attention, link, image) takes one paragraph's content, and a fragment that parses into anything else, such as `# h`, `a\n\nb`, `- x`, a setext heading or a definition, is `StreamError::InvalidNesting`; an empty or blank fragment empties the parent. One that holds blocks (root, block quote, item, footnote definition) takes a document's blocks. A list, table, head, body or row is `InvalidNesting`, a leaf `Unbalanced`. The fragment is parsed as though it were not at the start of a document, so frontmatter is never read and the document's own is kept.

References and footnote calls resolve as the written document will read them: against this document's definitions, found by a walk of the blocks outside phrasing and tables, seeded into the parse as `IncrementalParser` seeds its earlier definitions (`definitions::Seed`). For each label the first definition in document order wins, so a block fragment's references resolve first to a definition before the parent, then to the fragment's own, then to one after it; definitions among the replaced children no longer count, and the fragment's definitions become ordinary nodes. Unresolved ones stay text. Under a link, footnote definitions are left out of the seed, so a call stays text and the content model's rule holds, as the parser leaves a link's brackets text where a call is inside them. The parsed fragment is copied with `copy_subtree`, as for `replace_heading_section`, which owns its values and syntax and refreshes paragraph and item context; unlike a replacement section, a parsed reference keeps its reference syntax, as it resolves to the same definition here. The walk for definitions runs only when the fragment has a `[`.

## Content model

A tree is either valid or not built: one content model in `stream.rs`, `TagKind::holds`, decides which children each container holds, and every path that adds nodes checks it against the parent alone: `AstBuilder`, `EventHtmlRenderer`, `append_child`, `attach`, `replace_heading_section` and serde validation. `parse_children` inserts only what the parser built for a parent of the same content. The document root, block quotes, items and footnote definitions hold blocks and separators; paragraphs, headings, table cells, attention, links and images hold phrasing; a list holds items; a table holds heads and bodies, and they hold rows, with separators between them; a row holds cells. A root whose first child is phrasing holds only phrasing, an inline document, and one whose first child is a block holds no phrasing, so the two never share a parent. The one rule on ancestors is that a footnote reference cannot be inside a link, which `attach` checks by scanning the moved subtree. A violation is `StreamError::InvalidNesting`; a heading level outside one through six is `StreamError::InvalidValue`. `render_node` renders a subtree in fragment mode, so its root's children need not fit a document.

`set_kind` changes a node in place only where its children and its place stay valid without a check: a container keeps its kind, except that emphasis, strong and strikethrough are interchangeable, and a leaf keeps its variant, with HTML staying a block or inline. Any other change creates a node, moves the children with `attach` and detaches the old one. Detaching or moving a subtree out always leaves a valid tree, if not a normalized one.

Validity is what can be built; being normalized is what is written exactly. Shapes that every tree passes through while being built, such as empty containers, rows of different widths, a table without a head or a link in a link, are valid, and `normalize` fixes them. Reordering does not rely on arena allocation order matching tree order.

`NodeSyntax` preserves formatting needed for Markdown output without increasing the event or node size. Unmodified parsed syntax is inferred lazily from the original source extent. Explicit overrides live in a sparse map; `set_syntax(id, None)` disconnects inferred metadata. Item spread, link form and frontmatter fences remain separate from resolved semantic values. A reference's kind and label, and a footnote's label, are part of the event, beside the resolved destination, title or identifier (#192). Frontmatter's body accessor and first text edit exclude its fences. Replacement sections copied from another document materialize their values and metadata so that the other source can be dropped. Copied reference links/images become inline resources: this preserves resolved URLs/titles even when the destination has conflicting definitions or the source definition was outside the copied subtree. Same-document moves and document cloning retain references.

The source stays immutable while ranges depend on it. Integer offsets survive moving or reallocating an unchanged buffer but do not track text insertions. Use AST edits for semantic changes, or reparse when replacing the original source.

## Serialization

The `serde` feature serializes the source, arena slots and sparse syntax overrides. Deserialization constructs the arena directly and validates the complete forest, including detached subtrees, before exposing it. Validation rejects invalid UTF-8 ranges, broken or cyclic links, duplicate ownership, illegal roots, invalid event values, children the content model does not hold and incompatible metadata. It does not collect a complete event stream as an intermediate representation.

Source ranges stay ranges. Literal and owned values share the serialized `Value` representation; deserialization owns those strings. Node IDs retain their indices within the restored document. This arena format replaces mdast JSON and is not the mdast interchange schema.

## Diagnostics

`Message` retains its role as the serializer error and custom producer diagnostic. Its `reason` is now `Cow<'static, str>` (`"literal".into()` or `format!("... {value}").into()`), while `source` and `rule_id` are static strings. Use `span: Some(start..end)` for a source byte range, or `span: None` for an option error. This replaces boxed strings and `place: Option<Box<Place>>`. `unist::{Point, Position}`, `message::Place`, and the hidden `Location` utility have been removed; document and event spans remain the location contract.
