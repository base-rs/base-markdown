"""Prepare focused inputs for shared Markdown formatting and HTML number output."""
import json
from pathlib import Path

study = Path(__file__).resolve().parent
serializer = [
    dict(name='delimited-256', markdown='*emphasis* **strong** ~~deleted~~ ' * 256),
    dict(name='resources-256', markdown='[text *label*](/url "title") ![alt *text*](/image "title") ' * 256),
    dict(name='references-256', markdown='[text *label*][ref] ![alt *text*][ref] [ref][] [ref] ' * 256 + '\n\n[ref]: /url "title"'),
]
numbers = [
    # CommonMark source list markers allow at most nine digits. The Rust number
    # formatter tests also cover values beyond that source syntax's range.
    dict(name='ordered-lists-256', markdown=''.join(f'{999999744 + n}. item {n}\n\nseparator\n\n' for n in range(256))),
    dict(name='footnote-single', markdown='one[^n]\n\n[^n]: details'),
    dict(name='footnote-repeated-64', markdown='one[^n] ' * 64 + '\n\n[^n]: details'),
    dict(name='footnotes-256', markdown=''.join(f'one[^n{n}] two[^n{n}] three[^n{n}]\n\n' for n in range(256))
         + ''.join(f'[^n{n}]: details {n}\n\n' for n in range(256))),
]
for filename, rows in [('serializer.json', serializer), ('numbers.json', numbers)]:
    (study / filename).write_text(json.dumps(rows) + '\n')
