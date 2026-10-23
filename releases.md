# Releases

See [socketry-markdown release history](releases/socketry-markdown.md) for releases before the rename.

## Unreleased

- Add a shared pull-based semantic event contract, direct HTML writer, source-backed arena document, and incremental AST builder. Use the same event writer for the default HTML API and document traversal. Ordinary source values retain byte ranges; editing a value detaches it into owned storage.

- Use normalized semantic text when generating opt-in heading IDs, including multiline code and math spans. Balance mixed-list closing tags against their semantic openings, correcting previously mismatched closures after unclosed fences. Restore line positions after failed multiline frontmatter attempts.

- Improve Markdown parsing throughput and large-document scaling by indexing and batching event edits, scanning ordinary byte runs, compacting internal events and states, and reusing tokenizer buffers. Retain extension support and ordinary HTML output, with the corrections described above.

## v0.1.0

- Publish as `core-markdown` in the `core-rs` organization, with version numbering restarted at `0.1.0`.
- Require a `Release: vVERSION` Git trailer on the final release commit; version changes alone no longer trigger publication.
