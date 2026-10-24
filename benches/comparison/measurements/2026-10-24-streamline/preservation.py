"""Check saved outputs through direct, checked-event, and arena paths."""
from pathlib import Path
import collections
import hashlib
import json
import subprocess
import sys

out = Path(__file__).resolve().parent
comparison = out.parents[1]
work = Path('/tmp/core-markdown-streamline-20261024')
old = Path('/tmp/core-markdown-comparison')
cases = [('commonmark', 'commonmark', comparison / 'data/commonmark.json'),
         ('gfm-official', 'gfm', comparison / 'data/gfm-official.json'),
         ('gfm-spec', 'gfm', old / 'data/gfm-spec.json')]
cases += [('custom', profile, comparison / 'fixtures/custom.json')
          for profile in ['default', 'commonmark', 'gfm', 'gfm-footnotes']]
labels = sys.argv[1:] or ['inline', 'blocks', 'inlining', 'encoding']
preservation = json.loads((out / 'screening-preservation.json').read_text()) if (out / 'screening-preservation.json').exists() else []
preservation = [row for row in preservation if row['candidate'] not in labels]
for label in labels:
    for name, profile, path in cases:
        expected = json.loads((old / 'results' / f'{name}-{profile}-core-markdown.json').read_text())
        for engine in ['core-markdown', 'core-stream', 'core-arena']:
            actual = json.loads(subprocess.check_output([str(work / label), 'render', engine, profile, str(path)], cwd=comparison, text=True))
            assert actual == expected, (label, name, profile, engine)
        preservation.append(dict(candidate=label, corpus=name, profile=profile,
                                 outputs=len(expected), all_three_paths_byte_identical=True))
(out / 'screening-preservation.json').write_text(json.dumps(preservation, indent=2) + '\n')
print(f'{labels} preserve all 2,117 saved outputs through direct, checked, and arena paths.', flush=True)
