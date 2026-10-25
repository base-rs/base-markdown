#!/usr/bin/env python3
"""Run frozen binaries sequentially, alternating process order each round."""
import argparse
import datetime
import hashlib
import json
import platform
import statistics
import shutil
import subprocess
from pathlib import Path

root = Path(__file__).resolve().parent
p = argparse.ArgumentParser()
p.add_argument("--binary", action="append", required=True, help="LABEL=/absolute/path/to/frozen-binary")
p.add_argument("--data", type=Path, action="append")
p.add_argument("--mode", nargs="+", default=["html", "ast-html", "markdown"])
p.add_argument("--rounds", type=int, default=7)
p.add_argument("--warmup-ms", type=int, default=100)
p.add_argument("--out", type=Path, default=root / "timings.json")
p.add_argument("--check", action="store_true", help="Compare HTML, AST HTML, Markdown, semantic events/spans and headings")
p.add_argument("--alloc", action="store_true", help="Requires binaries built with --features allocations")
p.add_argument("--case", action="append", help="Optional exact case names")
args = p.parse_args()
binaries = [(label, Path(path).resolve()) for label, path in (s.split("=", 1) for s in args.binary)]
assert len({label for label, _ in binaries}) == len(binaries), "duplicate labels"
datasets = args.data or [root / "data" / f"{name}.json" for name in ("original", "real", "focused")]
metadata = {"utc": datetime.datetime.now(datetime.timezone.utc).isoformat(), "platform": platform.platform(),
    "binaries": {label: {"path": str(path), "sha256": hashlib.sha256(path.read_bytes()).hexdigest()} for label, path in binaries},
    "datasets": {str(path): hashlib.sha256(path.read_bytes()).hexdigest() for path in datasets},
    "rounds": args.rounds, "warmup_ms": args.warmup_ms, "order": "alternating per round"}
result = {"metadata": metadata, "samples": [], "summary": [], "checks": [], "allocations": []}

def save():
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")

def invoke(binary, command):
    launch = args.out.parent / "run"
    launch.unlink(missing_ok=True)
    shutil.copy2(binary, launch)
    completed = subprocess.run([str(launch), *map(str, command)], check=True, capture_output=True, text=True)
    return [json.loads(line) for line in completed.stdout.splitlines() if line.strip()]

for data in datasets:
    cases = json.loads(data.read_text())
    for case in cases:
        name = case["name"]
        if args.case and name not in args.case:
            continue
        if args.check:
            baseline = None
            for label, binary in binaries:
                diagnostic = invoke(binary, ["check", data, name])[0]
                diagnostic_path = args.out.parent / (args.out.stem + "-diagnostics") / label / (name + ".json")
                diagnostic_path.parent.mkdir(parents=True, exist_ok=True)
                diagnostic_path.write_text(json.dumps(diagnostic, ensure_ascii=False, indent=2) + "\n")
                if baseline is None:
                    baseline = diagnostic
                differences = [key for key in baseline if baseline.get(key) != diagnostic.get(key)]
                result["checks"].append({"label": label, "data": str(data), "name": name,
                    "matches_baseline": not differences, "different_fields": differences, "diagnostic": str(diagnostic_path)})
                save()
                if differences:
                    raise SystemExit(f"Correctness mismatch: {label} {name}: {differences}")
            print(f"check {name}: all agree", flush=True)
            continue
        for mode in args.mode:
            if args.alloc:
                for label, binary in binaries:
                    row = invoke(binary, ["alloc", mode, data, name])[0]
                    result["allocations"].append(dict(row, label=label, data=str(data)))
                save()
                continue
            count = 0
            for round_index in range(args.rounds):
                order = binaries if round_index % 2 == 0 else list(reversed(binaries))
                for position, (label, binary) in enumerate(order):
                    row = invoke(binary, ["bench", mode, data, name, count, args.warmup_ms])[0]
                    assert not row["allocation_instrumented"], "Use non-instrumented binaries for timing"
                    assert not row["diagnostics_enabled"], "Use default core features for timing"
                    count = row["iterations"]
                    result["samples"].append(dict(row, label=label, data=str(data), round=round_index, order=position))
                    save()
            summaries = []
            for label, _ in binaries:
                samples = [row["seconds_per_iteration"] for row in result["samples"]
                    if row["label"] == label and row["data"] == str(data) and row["name"] == name and row["mode"] == mode]
                summaries.append({"label": label, "data": str(data), "name": name, "mode": mode,
                    "median_seconds": statistics.median(samples), "min_seconds": min(samples), "max_seconds": max(samples),
                    "samples_seconds": samples})
            base = summaries[0]["median_seconds"]
            for row in summaries:
                row["ratio_to_baseline"] = row["median_seconds"] / base
            result["summary"].extend(summaries)
            save()
            print(f"{mode} {name}: " + ", ".join(f"{row['label']} {row['median_seconds']*1e6:.2f} us ({row['ratio_to_baseline']:.4f}x)" for row in summaries), flush=True)
save()
