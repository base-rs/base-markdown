"""Check exact direct-HTML preservation against the pinned upstream parser."""

from pathlib import Path
import argparse
import hashlib
import html
import json
import re
import subprocess
import urllib.request

arguments = argparse.ArgumentParser(description=__doc__)
arguments.add_argument("--all", action="store_true", help="Also record output agreement for all six engines")
options = arguments.parse_args()

P = Path(__file__).resolve().parent
D = P / "data"
D.mkdir(exist_ok=True)


def download(name, url, digest):
    path = D / name
    if not path.exists():
        with urllib.request.urlopen(url, timeout=60) as response:
            path.write_bytes(response.read())
    data = path.read_bytes()
    if hashlib.sha256(data).hexdigest() != digest:
        raise ValueError(f"Unexpected source hash for {url}; inspect before updating it")
    return data.decode("utf-8")


def strip_markup(source):
    return html.unescape(re.sub(r"<[^>]*>", "", source))


commonmark = json.loads(download(
    "commonmark.json",
    "https://spec.commonmark.org/0.31.2/spec.json",
    "d431b29d97b6f73e69d547109cf5081578fac931e72afe95639ebe766c1b2a20",
))
gfm_source = download(
    "gfm-official.html",
    "https://github.github.com/gfm/",
    "b153d814fdfc8624bb6da7449162c1cd707a637f7d1c1b636eb44b9cf63fa220",
)
starts = list(re.finditer(r'<div class="example" id="example-(\d+)">', gfm_source))
gfm = []
for index, start in enumerate(starts):
    end = starts[index + 1].start() if index + 1 < len(starts) else len(gfm_source)
    blocks = re.findall(
        r'<pre><code class="language-(markdown|html)">(.*?)</code></pre>',
        gfm_source[start.end():end],
        re.S,
    )[:2]
    case = {language: strip_markup(code).replace("→", "\t") for language, code in blocks}
    assert set(case) == {"markdown", "html"}
    case["example"] = int(start.group(1))
    gfm.append(case)
assert len(commonmark) == 652 and len(gfm) == 677
gfm_path = D / "gfm-official.json"
gfm_path.write_text(json.dumps(gfm, ensure_ascii=False), encoding="utf-8")

subprocess.run([
    "cargo", "build", "--release", "--locked", "--manifest-path", str(P / "Cargo.toml")
], check=True)
runner = str(P / "target/release/markdown-comparison")


def render(parser, profile, path):
    if parser in ["markly", "commonmarker", "inkmark"]:
        command = ["bundle", "exec", "ruby", str(P / "adapter.rb"), parser, "render", profile, str(path)]
    else:
        command = [runner, "render", parser, profile, str(path)]
    return json.loads(subprocess.check_output(command, cwd=P))


agreement = []
for profile, cases, path in [
    ("commonmark", commonmark, D / "commonmark.json"),
    ("gfm", gfm, gfm_path),
]:
    actual = render("core-markdown", profile, path)
    arena = render("core-arena", profile, path)
    upstream = render("markdown-rs", profile, path)
    assert arena == actual, f"{profile}: arena traversal differs from direct rendering"
    assert len(actual) == len(upstream) == len(cases)
    differences = [case["example"] for case, a, b in zip(cases, actual, upstream) if a != b]
    assert not differences, f"{profile}: upstream differences in examples {differences}"
    if profile == "commonmark":
        differences = [case["example"] for case, a in zip(cases, actual) if a != case["html"]]
        assert not differences, f"CommonMark reference differences in examples {differences}"
    print(f"{profile}: {len(cases)} byte-identical upstream outputs")
    if options.all:
        for parser in ["core-markdown", "markdown-rs", "comrak", "markly", "commonmarker", "inkmark"]:
            output = render(parser, profile, path)
            assert len(output) == len(cases)
            result = {
                "parser": parser,
                "profile": profile,
                "examples": len(cases),
                "core_exact": sum(a == b for a, b in zip(actual, output)),
            }
            if profile == "commonmark":
                result["reference_exact"] = sum(a == case["html"] for a, case in zip(output, cases))
            agreement.append(result)
            (P / "results").mkdir(exist_ok=True)
            (P / "results" / f"{parser}-{profile}-output.json").write_text(
                json.dumps(output, ensure_ascii=False), encoding="utf-8"
            )
            print(json.dumps(result))
if options.all:
    (P / "results" / "consistency.json").write_text(json.dumps(agreement, indent=2), encoding="utf-8")

custom_path = P / "fixtures/custom.json"
custom = json.loads(custom_path.read_text(encoding="utf-8"))
for profile in ["default", "commonmark", "gfm", "gfm-footnotes"]:
    actual = render("core-markdown", profile, custom_path)
    arena = render("core-arena", profile, custom_path)
    upstream = render("markdown-rs", profile, custom_path)
    assert arena == actual, f"{profile}: targeted arena traversal differs from direct rendering"
    assert len(actual) == len(upstream) == len(custom)
    differences = [case["name"] for case, a, b in zip(custom, actual, upstream) if a != b]
    assert not differences, f"{profile}: targeted upstream differences in {differences}"
    print(f"{profile}: {len(custom)} byte-identical targeted upstream outputs")
