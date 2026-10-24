"""Generate long ASCII fallback and dense Unicode definition controls."""
import json
from pathlib import Path

rows = []
for kind in ["long-uppercase", "long-spaced", "unicode-dense", "unicode-long", "unicode-upper-long", "unicode-lower-long", "unicode-upper-tail"]:
    labels = [
        f"label-{index}-" + "abcd" * 180 + ("END" if kind == "long-uppercase" else "  end")
        if kind.startswith("long-") else
        ("δοκιμήéß世界" * 20 + "Σ" if kind == "unicode-upper-tail" else
         "ΔΟΚΙΜΉϴẞİ世界" * 20 if kind == "unicode-upper-long" else
         "δοκιμήéß世界" * 20 if kind == "unicode-lower-long" else
         "ΔοκιμήÉİß世界" * (20 if kind == "unicode-long" else 1)) + f"-{index}"
        for index in range(1024)
    ]
    source = " ".join(f"[{labels[index]}]" for index in [0, 511, 1023]) + "\n\n"
    source += "".join(f"[{label}]: /{index}\n" for index, label in enumerate(labels))
    rows.append(dict(name=f"keys-{kind}-1024", markdown=source))
path = Path(__file__).resolve().parents[2] / "data/bench-key-controls.json"
path.write_text(json.dumps(rows, ensure_ascii=False, indent=2) + "\n")
