# Round-trip Fuzz Harness

`tests/roundtrip_fuzz.rs` checks that normalizing and serializing Markdown keeps its meaning, on generated corpora with fixed seeds. Use it, rather than an ad-hoc harness, to measure a serializer or normalizer change, so that author and reviewer figures come from the same inputs.

## What it checks

Each input is parsed with `ParseOptions::normalize()`, the syntax `normalize_markdown` reads, or built by hand as a tree. It is normalized, serialized with the default Markdown options, parsed with the same options and normalized again. An input fails as:

- `normalized`: either document is not normalized after `Document::normalize`.
- `html`: the HTML of the two documents differs. The final line ending is ignored, since a hand-built tree has no source for it, and so is the line ending after a hard break in a parsed document, which is part of the break. A hand-built hard break has no line ending after it, so a line ending there in its tree is text and is compared.
- `tree`: the HTML is the same, but the canonical trees of `support::fixture::canonical` differ. These count line endings that the HTML comparison ignores, so dropping one is not a fix, and structure the HTML flattens, such as links in image text.
- `unstable`: serializing the second document gives different Markdown.
- `error`: serializing a parsed document returns an error. A hand-built tree that the serializer rejects is counted as rejected instead.

A panic fails the test whatever the baseline.

## Corpora

| Corpus | Inputs |
| --- | --- |
| `realistic` | Prose-like pieces: words, markers, escapes, references, links, code, block starts, tables and literal autolinks. |
| `soup` | Single characters from `*_~a. \n\`. |
| `edge` | Markers next to `&#x20;`, `&#xA;`, `&#x9;`, encoded letters and escaped markers, sometimes in a heading, table cell, item or quote. |
| `strike` | Tilde runs of every length among letters, digits, attention, escapes, code and links. |
| `tree-asterisk`, `tree-underscore` | The same hand-built paragraphs, headings and table cells of nested emphasis, strong, strikethrough, links, code and breaks, serialized with `*` and with `_` as the emphasis and strong marker. A hard break is never the last child of a block or attention, where it has no source, and never in a table cell, which GFM cannot write. |

Each corpus has its own seed and a small xorshift generator, so the inputs and counts are the same on every platform, and the first inputs of a larger run are the inputs of a smaller one.

## Baseline

At the default size of 10,000 inputs per corpus, `tests/roundtrip_fuzz_baseline.txt` records each corpus's failures, rejected trees and a 64-bit FNV-1a digest of its sorted failing inputs, the lines of `<corpus>.txt`. The test fails on any difference, up or down, and prints the change in each count, so a fix cannot hide a new failure in the same corpus. When a change is intended, rewrite the baseline with `FUZZ_BLESS=1`, which prints what changed, and commit it with the change: the diff shows the new counts in review.

The report also buckets failures by known issue, using simple rules on the input, the output and both HTML renderings. The buckets are approximate: use them to see what moved, and the failing inputs to see why.

## Usage

```sh
# The CI size, with the report.
cargo test --test roundtrip_fuzz -- --nocapture

# A larger run, in release.
FUZZ_INPUTS=1000000 cargo test --release --test roundtrip_fuzz round_trip_fuzz -- --nocapture

# Rewrite the baseline after an intended change; only at the default size.
FUZZ_BLESS=1 cargo test --test roundtrip_fuzz round_trip_fuzz
```

`FUZZ_OUTPUT=directory` writes, for each corpus, `<corpus>.txt`, its sorted and deduplicated failing inputs as one Rust string literal per line, and `<corpus>-details.txt`, each failure with its kind, bucket, output and both HTML renderings, separated by tabs. The files are large for the tree corpora at 1,000,000 inputs: about 220 MB of details and 140 MB of inputs each, against at most 20 MB for the other corpora.

## Comparing a branch with main

Run the same size on both builds, writing the failing inputs to two directories, then compare the sorted lists. Lines only in the branch's list are new failures, and lines only in main's list are fixed.

```sh
git worktree add /tmp/fuzz-main origin/main
(cd /tmp/fuzz-main && FUZZ_INPUTS=1000000 FUZZ_OUTPUT=/tmp/fuzz/main cargo test --release --test roundtrip_fuzz round_trip_fuzz -- --nocapture)
FUZZ_INPUTS=1000000 FUZZ_OUTPUT=/tmp/fuzz/branch cargo test --release --test roundtrip_fuzz round_trip_fuzz -- --nocapture

export LC_ALL=C # The lists are sorted bytewise.
for corpus in realistic soup edge strike tree-asterisk tree-underscore; do
	echo "$corpus: $(comm -13 /tmp/fuzz/main/$corpus.txt /tmp/fuzz/branch/$corpus.txt | wc -l) new, $(comm -23 /tmp/fuzz/main/$corpus.txt /tmp/fuzz/branch/$corpus.txt | wc -l) fixed"
done
comm -13 /tmp/fuzz/main/edge.txt /tmp/fuzz/branch/edge.txt | head
```

Use `grep -F` with a listed input on `<corpus>-details.txt` to see its output and HTML. If main predates the harness, copy `tests/roundtrip_fuzz.rs`, `tests/roundtrip_fuzz_baseline.txt` and `tests/support/` into the main worktree first; the harness uses only public API.
