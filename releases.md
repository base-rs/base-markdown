# Releases

See [socketry-markdown release history](releases/socketry-markdown.md) for releases before the rename.

## Unreleased

- Add a shared pull-based semantic event contract, direct HTML writer, source-backed arena document, and incremental AST builder. Use the same event writer for the default HTML API and document traversal. Ordinary source values retain byte ranges; editing a value detaches it into owned storage.

- Use normalized semantic text when generating opt-in heading IDs, including multiline code and math spans. Balance mixed-list closing tags against their semantic openings, correcting previously mismatched closures after unclosed fences. Restore line positions after failed multiline frontmatter attempts, and keep adjacent setext headings balanced when earlier underline text joins a paragraph. Stop retained HTML blank lines at their enclosing container boundary, and preserve sibling list items when HTML consumes the separating newline. Keep collapsed spaces significant in reference labels, so `[a b]` and `[ab]` resolve independently.

- Prepare ordinary CommonMark blocks directly in a compact source-backed arena, deferring inline work until consumption. Correct sibling lists after unclosed fences, container indentation tabs, and empty unclosed code bodies on this path. Use the document line-ending style for separators inserted before container closing tags. Definitions, raw HTML blocks, and extension configurations still use the compatibility producer.

- Resolve ordinary inline syntax in reusable compact storage with indexed code delimiters, bounded emphasis searches, and source-backed link metadata. Scan inline candidates and HTML escapes with bounded AArch64 NEON lookups and a portable scalar fallback. Keep ordinary leading links on the compact path, and reuse compact inline resolution in eligible grammar-backed paragraphs. Correct emphasis adjacent to escaped delimiter characters.

- Improve Markdown parsing throughput and large-document scaling by indexing and batching event edits, scanning ordinary byte runs, compacting internal events and states, and reusing tokenizer buffers. Retain extension support and ordinary HTML output, with the corrections described above.

## v0.1.0

- Publish as `core-markdown` in the `core-rs` organization, with version numbering restarted at `0.1.0`.
- Require a `Release: vVERSION` Git trailer on the final release commit; version changes alone no longer trigger publication.
