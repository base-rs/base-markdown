"""Generate a focused compact/grammar transition workload with forward references."""
import json
from pathlib import Path
import sys

out = Path(sys.argv[1])
out.mkdir(parents=True, exist_ok=True)
unit = "compact [a]\n\nfirst <i>HTML</i> [a]\n\ncompact again [a]\n\nsecond <b>HTML</b> [a]\n\n"
cases = [dict(name=f"inline-fallback-{size}", markdown=unit * count + "[a]: /a\n")
         for size, count in [("small", 1), ("100k", 100_000 // len(unit))]]
(out / 'fallback.json').write_text(json.dumps(cases))
