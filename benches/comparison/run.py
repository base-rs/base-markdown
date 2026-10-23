"""Run in-process Markdown-to-HTML timings; write raw samples to results/."""
from pathlib import Path
import json
import subprocess
import sys

P = Path(__file__).resolve().parent
subprocess.run([sys.executable, str(P / "prepare.py")], check=True)
subprocess.run(["cargo", "build", "--release", "--locked", "--manifest-path", str(P / "Cargo.toml")], check=True)
(P / "results").mkdir(exist_ok=True)
parsers = sys.argv[1:] or ["core-markdown", "markdown-rs", "comrak", "markly", "commonmarker", "inkmark"]
for parser in parsers:
    for profile in ["commonmark", "gfm"]:
        path = P / "data" / ("bench-commonmark.json" if profile == "commonmark" else "bench-gfm.json")
        if parser in ["markly", "commonmarker", "inkmark"]:
            command = ["bundle", "exec", "ruby", str(P / "adapter.rb"), parser, "bench", profile, str(path)]
        else:
            command = [str(P / "target/release/markdown-comparison"), "bench", parser, profile, str(path)]
        output = subprocess.check_output(command, cwd=P, text=True)
        (P / "results" / f"{parser}-{profile}.jsonl").write_text(output)
        for line in output.splitlines():
            result = json.loads(line)
            print(f"{parser:14} {result['document']:12} {result['median_seconds'] * 1000:9.3f} ms", flush=True)
