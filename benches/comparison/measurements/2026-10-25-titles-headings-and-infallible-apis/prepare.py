"""Focused title normalization and semantic heading workloads."""
import json
from pathlib import Path
study = Path(__file__).resolve().parent
rows = []
for name, title in [
    ('single-line', 'plain title'),
    ('multiline', 'first\n  second\n\tthird'),
    ('decoded-multiline', 'first &amp; \\*\n  second &NewLine;  next'),
    ('numeric-multiline', '&#x1F980; &#65;\n  &#x20AC; &#128512;'),
]:
    for repeat in [1, 256]:
        rows.append(dict(name=f'title-{name}-{repeat}', operation='html',
            markdown=(f'[text](/url "{title}")\n\n' * repeat)))
heading = '# *Heading* `code` &amp; [link](/url)\n\n'
for operation in ['html', 'headings']:
    rows.append(dict(name=f'headings-{operation}-256', operation=operation,
        headings=True, markdown=heading * 256))
rows.append(dict(name='inline-html-multiline-256', operation='html',
    markdown='<i\n  title="plain">text</i>\n\n' * 256))
(study / 'features.json').write_text(json.dumps(rows) + '\n')
