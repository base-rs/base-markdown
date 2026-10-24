"""Rebuild deterministic reference-key shape controls."""
import json
from pathlib import Path
import random

rows = []
count = 1024
for kind in ["uppercase", "unicode", "spaced", "random"]:
    labels = [
        f"LABEL-{index}" if kind == "uppercase" else
        f"clé-{index}" if kind == "unicode" else
        f"label  {index}" if kind == "spaced" else f"label-{index}"
        for index in range(count)
    ]
    order = list(range(count))
    random.Random(20261025).shuffle(order)
    source = " ".join(f"[{labels[index]}]" for index in [0, 511, 1023]) + "\n\n"
    source += "".join(
        f'[{labels[index]}]: /{index} "title {index}"\n' for index in order
    )
    rows.append(dict(name=f"keys-{kind}-1024", markdown=source))

Path(__file__).with_name("key-shapes.json").write_text(
    json.dumps(rows, ensure_ascii=False, indent=2) + "\n"
)
