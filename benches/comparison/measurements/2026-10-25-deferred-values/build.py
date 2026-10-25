#!/usr/bin/env python3
"""Build frozen baseline/candidate drivers and optional individual stages."""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess

from prepare import prepare


def main():
    root = Path(__file__).resolve().parent
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", type=Path, required=True, help="Baseline core-markdown checkout")
    parser.add_argument("--candidate", type=Path, required=True, help="Candidate core-markdown checkout")
    parser.add_argument("--work", type=Path, required=True, help="Build outputs, frozen binaries and unpacked data")
    parser.add_argument("--stage", action="append", default=[], metavar="LABEL=PATH", help="Additional checkout; timing binary only")
    args = parser.parse_args()
    work = args.work.resolve()
    sources = [("baseline", args.baseline.resolve(), True), ("candidate", args.candidate.resolve(), True)]
    for stage in args.stage:
        label, separator, path = stage.partition("=")
        if not separator or not re.fullmatch(r"[A-Za-z0-9_-]+", label):
            parser.error("stages must use LABEL=PATH with a simple alphanumeric label")
        sources.append((label, Path(path).resolve(), False))
    if len({label for label, _, _ in sources}) != len(sources):
        parser.error("each baseline, candidate and stage label must be unique")
    for _, source, _ in sources:
        if not (source / "Cargo.toml").is_file():
            parser.error(f"missing core-markdown manifest in {source}")
    prepare(work)
    environment = dict(os.environ, CARGO_BUILD_JOBS="2", CARGO_INCREMENTAL="0")
    for label, source, all_features in sources:
        crate = work / f"{label}-driver"
        (crate / "src").mkdir(parents=True, exist_ok=True)
        shutil.copyfile(root / "driver.rs", crate / "src/main.rs")
        shutil.copyfile(root / "driver.lock", crate / "Cargo.lock")
        manifest = f'''[package]
name = "refine-study-driver"
version = "0.0.0"
edition = "2021"

[dependencies]
core-markdown = {{ path = {json.dumps(str(source))} }}
serde_json = "1"

[features]
allocations = []
diagnostics = ["core-markdown/serde"]

[profile.release]
debug = false

[workspace]
'''
        (crate / "Cargo.toml").write_text(manifest, encoding="utf-8")
        variants = [("", "bench")]
        if all_features:
            variants += [("allocations", "allocations"), ("diagnostics", "diagnostics")]
        for feature, suffix in variants:
            command = ["cargo", "build", "--release", "--locked", "--manifest-path", str(crate / "Cargo.toml"), "--target-dir", str(work / "target")]
            if feature:
                command += ["--features", feature]
            subprocess.run(command, check=True, env=environment)
            destination = work / f"{label}-{suffix}"
            shutil.copy2(work / "target/release/refine-study-driver", destination)
            print(destination, flush=True)


if __name__ == "__main__":
    main()
