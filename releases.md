# Releases

See [socketry-markdown release history](releases/socketry-markdown.md) for releases before the rename.

## Unreleased

- Improve Markdown parsing throughput and large-document scaling by indexing and batching event edits, scanning ordinary byte runs, compacting internal events and states, and reusing tokenizer buffers. Preserve existing HTML output and extension support.

## v0.1.0

- Publish as `core-markdown` in the `core-rs` organization, with version numbering restarted at `0.1.0`.
- Require a `Release: vVERSION` Git trailer on the final release commit; version changes alone no longer trigger publication.
