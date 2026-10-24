"""Fetch pinned, attributed real documents and build adapter inputs without rewriting bytes."""
import hashlib
import json
from pathlib import Path
from urllib.request import urlopen

root = Path(__file__).resolve().parent
cache = root / "data/real-documents"
cache.mkdir(parents=True, exist_ok=True)
profiles = {}
for source in json.loads((root / "real-documents.json").read_text()):
    path = cache / (source["name"] + ".md")
    data = path.read_bytes() if path.exists() else urlopen(source["url"], timeout=60).read()
    if len(data) != source["bytes"] or hashlib.sha256(data).hexdigest() != source["sha256"]:
        raise ValueError("Source changed or cache is corrupt: " + source["name"])
    path.write_bytes(data)
    profiles.setdefault(source["profile"], []).append(dict(name=source["name"], markdown=data.decode("utf-8")))
for profile, documents in profiles.items():
    path = root / "data" / ("bench-real-" + profile + ".json")
    path.write_text(json.dumps(documents, ensure_ascii=False, indent=2) + "\n")
    print(path.name, len(documents), hashlib.sha256(path.read_bytes()).hexdigest())
