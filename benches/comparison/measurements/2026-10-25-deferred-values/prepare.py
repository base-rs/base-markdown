#!/usr/bin/env python3
"""Restore the exact frozen benchmark inputs into a study work directory."""
import argparse
import gzip
import json
from pathlib import Path


def prepare(work):
    root = Path(__file__).resolve().parent
    groups = json.loads(gzip.decompress((root / "inputs.json.gz").read_bytes()))
    data = work / "data"
    data.mkdir(parents=True, exist_ok=True)
    for name in ("original", "real", "focused"):
        path = data / f"{name}.json"
        contents = json.dumps(groups[name], ensure_ascii=False, indent=2) + "\n"
        path.write_bytes(contents.encode("utf-8"))
        print(f"{path}: {len(groups[name])} cases")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--work", type=Path, default=Path(__file__).resolve().parent / "work")
    prepare(parser.parse_args().work.resolve())
