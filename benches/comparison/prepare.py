"""Generate deterministic Markdown workloads shared by every parser."""
from pathlib import Path
import json

P = Path(__file__).resolve().parent
D = P / "data"
D.mkdir(exist_ok=True)


def save(name, data):
    (D / f"{name}.json").write_text(json.dumps(data, ensure_ascii=False, indent=2))


readme = (P / "fixtures/readme.md").read_text()
paragraph = (
    "Markdown parsing includes **strong emphasis**, *emphasis*, `inline code`, "
    "[links](https://example.com/docs?q=x&n=2), escaped \\*stars\\*, Unicode 世界, "
    "and &amp; entities.\nSoft line breaks remain part of the paragraph.\n"
)
block = (
    "## Section {i}\n\n" + paragraph
    + '\n> A block quote with **bold** text.\n>\n> Another paragraph.\n\n'
    + '1. First item\n   - Nested item\n2. Second item\n\n'
    + '```rust\nfn main() {{ println!("hello"); }}\n```\n\n'
)
gfm = (
    "| Feature | Status |\n| :--- | ---: |\n| ~~old~~ | **new** |\n"
    "| link | https://example.com |\n\n- [x] Complete\n- [ ] Pending\n\n"
    "Visit www.example.com or email user@example.com.\n\n"
)

def document(minimum_bytes, extension=""):
    parts = []
    size = 0
    index = 0
    while size < minimum_bytes:
        part = block.format(i=index) + extension
        parts.append(part)
        size += len(part.encode())
        index += 1
    return "".join(parts)


save("bench-commonmark", [
    {"name": "readme", "markdown": readme},
    {"name": "mixed-100k", "markdown": document(100_000)},
    {"name": "mixed-1m", "markdown": document(1_000_000)},
])
save("bench-gfm", [{"name": "gfm-100k", "markdown": document(100_000, gfm)}])
