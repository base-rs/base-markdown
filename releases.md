# Releases

See [socketry-markdown release history](releases/socketry-markdown.md) for releases before the rename.

## Unreleased

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

## v0.1.0

- Publish as `core-markdown` in the `core-rs` organization, with version numbering restarted at `0.1.0`.
- Require a `Release: vVERSION` Git trailer on the final release commit; version changes alone no longer trigger publication.
