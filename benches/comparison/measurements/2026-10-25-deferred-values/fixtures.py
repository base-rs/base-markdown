#!/usr/bin/env python3
"""Freeze the four original workloads, six real documents and focused workloads."""
import argparse
import json
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("--repo", type=Path, default=Path("/Users/samuel/Developer/core-rs/core-markdown"))
args = parser.parse_args()
root = Path(__file__).resolve().parent
data = root / "data"
data.mkdir(exist_ok=True)
source = args.repo / "benches/comparison/data"

def read(name, profile):
    return [dict(case, profile=profile) for case in json.loads((source / name).read_text())]

groups = {
    "original": read("bench-commonmark.json", "commonmark") + read("bench-gfm.json", "gfm"),
    "real": read("bench-real-commonmark.json", "commonmark") + read("bench-real-gfm.json", "gfm"),
    "focused": [
        {"name": "images-simple", "markdown": ('![Image alt](images/a.png "A title")\n\n' * 2048), "profile": "commonmark"},
        {"name": "images-rich-alt", "markdown": ('![a *bold* **strong** `code` &amp; \\* ![nested](b.png)](a.png "Title &amp; \\*")\n\n' * 1024), "profile": "commonmark"},
        {"name": "titles-and-definitions", "markdown": ('[text](https://example.com "\\*title &amp; more")\n\n[ref][label]\n\n[label]: https://example.com "\\*title &amp; more"\n\n' * 768), "profile": "commonmark"},
        {"name": "metadata-leading-escapes", "markdown": ('[link](\\*path "\\*title")\n\n[ref][label]\n\n[label]: \\*path "\\*title"\n\n' * 1024), "profile": "commonmark"},
        {"name": "leading-escapes", "markdown": ('\\*word\n\n\\!\n\n\\*\\!\n\na\\*b &amp; &#42;\n\n' * 2048), "profile": "commonmark"},
        {"name": "headings", "markdown": ('# Heading *emphasis* `code` &amp; ![image alt](a.png)\n\n## Unicode 世界 🦀\n\n' * 1024), "profile": "commonmark", "heading_ids": True},
        {"name": "tables-no-pipes", "markdown": ('| Feature | Status |\n| :--- | ---: |\n| **Fast** | `ready` |\n| Image ![alt](a.png) | escaped \\* |\n\n' * 1024), "profile": "gfm"},
        {"name": "tables-pipe-heavy", "markdown": ('| Feature | Status |\n| :--- | ---: |\n| a \\| b \\| c | `x\\|y` |\n| ![a \\| b](a.png) | d \\| e |\n\n' * 1024), "profile": "gfm"},
        {"name": "headings-forced-setext", "markdown": ('# Heading *emphasis* `code` &amp; ![image alt](a.png)\n\n## Unicode 世界 🦀\n\n' * 1024), "profile": "commonmark", "setext": True},
        {"name": "headings-multiline", "markdown": ('Heading *emphasis*\nwith `code` and ![image](a.png)\n========\n\nAnother heading\nwith two lines\n--------\n\n' * 1024), "profile": "commonmark"},
    ],
}
for name, cases in groups.items():
    path = data / f"{name}.json"
    path.write_text(json.dumps(cases, ensure_ascii=False, indent=2) + "\n")
    print(f"{path}: {len(cases)} cases, {sum(len(c['markdown'].encode()) for c in cases)} bytes")
