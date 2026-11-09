// Released under the MIT License.
// Copyright, 2022, by Bernhard Berger.
// Copyright, 2022, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use std::fs;

fn readme(c: &mut Criterion) {
    let doc = fs::read_to_string("readme.md").unwrap();

    c.bench_with_input(BenchmarkId::new("readme", "readme"), &doc, |b, s| {
        b.iter(|| base_markdown::to_html(s));
    });
}

// A deep nest of footnote definitions. A footnote definition cannot start in a heading or an
// image, and the HTML renderer counts the open headings and images rather than looking through
// every open frame at each start, which made such a nest quadratic (#171). Looking through the
// frames reads no node and allocates nothing, so no count that a test reads shows it, and this
// measures it instead: 8 times as deep should take about 8 times as long.
fn nested_footnote_definitions(c: &mut Criterion) {
    let mut group = c.benchmark_group("nested footnote definitions");
    for depth in [1 << 9, 1 << 12] {
        let source = "- [^1]: ".repeat(depth);
        group.bench_with_input(BenchmarkId::from_parameter(depth), &source, |b, s| {
            b.iter(|| base_markdown::to_html(s));
        });
    }
    group.finish();
}

// fn one_and_a_half_mb(c: &mut Criterion) {
//     let doc = fs::read_to_string("../a-dump-of-markdown/markdown.md").unwrap();
//     let mut group = c.benchmark_group("giant");
//     group.sample_size(10);
//     group.bench_with_input(BenchmarkId::new("giant", "1.5 mb"), &doc, |b, s| {
//         b.iter(|| base_markdown::to_html(s));
//     });
//     group.finish();
// }
// , one_and_a_half_mb

criterion_group!(benches, readme, nested_footnote_definitions);
criterion_main!(benches);
