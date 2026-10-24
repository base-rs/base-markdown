"""Generate focused entity and HTML fixtures without committing repeated input."""
import json
from pathlib import Path
import sys

def repeated(name, text):
    return dict(name=name, markdown=text * (100_000 // len(text.encode()) + 1))

out = Path(sys.argv[1])
out.mkdir(parents=True, exist_ok=True)
(out / 'entities.json').write_text(json.dumps([repeated('entities-100k',
    'Prose &amp; &NotEqualTilde; &#65; &#x1F600; [a &amp; b](/a?b=1&amp;c=2 "a &quot; b").\n\n')]))
(out / 'html.json').write_text(json.dumps([
    repeated('html-safe-100k', '<div>\n<p>ordinary é😀 HTML &amp; text</p>\n</div>\n\n'),
    repeated('html-filtered-100k', '<div>\n<SCRIPT>alert(1)</SCRIPT><iframe src="x"></iframe>é😀\n</div>\n\n'),
]))
