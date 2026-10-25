#!/usr/bin/env python3
"""Reuse the frozen previous study inputs and add boundary/table-run workloads."""
import argparse
import importlib.util
import json
import sys
sys.dont_write_bytecode = True
from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / "2026-10-25-deferred-values"


def extra_cases():
    cases = []
    for length in (128, 800):
        for style in ("plain", "slash-heavy"):
            label = ("a" if style == "plain" else "\\") * length
            cases.append({"name": f"reference-labels-{style}-{length}", "profile": "commonmark",
                "markdown": f"[display][{label}]\n\n" * 128 + f"[{label}]: /destination \"A title\"\n"})
    short_label = "\\" * 16
    cases.append({"name": "reference-authored-forms-small", "profile": "commonmark",
        "markdown": "[display][simple]\n\n[simple][]\n\n[simple]\n\n![image][simple]\n\n"
            + f"[display][{short_label}]\n\n[{short_label}][]\n\n[{short_label}]\n\n"
            + "[display][a\\]b]\n\n[a\\]b][]\n\n[a\\]b]\n\n"
            + "[simple]: /plain\n" + f"[{short_label}]: /slashes\n" + "[a\\]b]: /bracket\n"})
    cell = "α界 " * 64 + "ordinary text " * 64 + r"\|" + "x" * 2048 + "🦀café " * 32 + r"\|" + "tail" * 128
    cases.append({"name": "tables-long-unicode-far-pipes", "profile": "gfm",
        "markdown": "| Long cell | Other |\n| :--- | ---: |\n" + f"| {cell} | ordinary |\n" * 32})
    cases.append({"name": "mixed-authored-line-endings", "profile": "commonmark", "frontmatter": True,
        "markdown": '---\r\ntitle: Mixed\n---\r\n\r\n# Heading\r\n\r\n'
            '```rust \\*metadata\r\ncode < &\nnext line\r\n```\n\n'
            '<div>\r\nraw &amp;\rline\n</div>\r\n\n'
            '[display][label]\r\n\r\n[label]: /destination "title"\r\n'})
    cases.append({"name": "partial-tab-code-fragments", "profile": "commonmark",
        "markdown": ("> \t\ta\n\n- \t\tq\n\n  ```\n\tq\n  ```\n\n" * 512)})
    for marker, name in [("first@example.com then second@example.org", "valid"), ("a@b_ nope@例え.example first@example.com", "invalid-then-valid"), ("ordinary ending", "no-marker")]:
        cases.append({"name": f"email-long-prefix-{name}", "profile": "gfm", "markdown": (("ordinary α prose " * 512) + marker + "\n\n") * 64})
    empty_cases = ["-\n  foo", "-\n\n  foo", "* a\n*\n\n  \n\t\n* b", "- 1.\n-", "> -\n> \n>   foo",
        "-\n\n", "-\n  [x]: /target\n\n  foo", "-\n  >\n\n- next", "* - +\n\n* -", "-\n  ```\n\n  ```\n\n- b"]
    for index, value in enumerate(empty_cases):
        for ending, label in [("\n", "lf"), ("\r", "cr"), ("\r\n", "crlf")]:
            cases.append({"name": f"empty-items-{index}-{label}", "profile": "commonmark", "markdown": value.replace("\n", ending)})
    return cases


def prepare(work):
    spec = importlib.util.spec_from_file_location("deferred_values_prepare", previous / "prepare.py")
    shared = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(shared)
    shared.prepare(work)
    path = work / "data/focused.json"
    focused = json.loads(path.read_text()) + extra_cases()
    path.write_bytes((json.dumps(focused, ensure_ascii=False, indent=2) + "\n").encode("utf-8"))
    lists = [
        {"name": f"list-blocks-{count}", "profile": "commonmark",
         "markdown": ("- first\n\n" + "  paragraph\n\n" * (count - 1)) * 128}
        for count in (2, 8, 32)
    ] + [
        {"name": f"list-gap-{count}", "profile": "commonmark",
         "markdown": "- first\n" + "\n" * count + "  second\n"}
        for count in (2, 4096, 65536)
    ]
    (work / "data/lists.json").write_text(json.dumps(lists, indent=2) + "\n")
    print(f"{path}: {len(focused)} cases; six list-spacing controls")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--work", type=Path, required=True)
    prepare(parser.parse_args().work.resolve())
