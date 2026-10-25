"""Generate delimiter controls without changing the established benchmark driver."""
import json
import subprocess
import sys
from pathlib import Path
s = Path(__file__).resolve().parent
previous = s.with_name('2026-10-25-footnotes-indexes-and-escape-context')
subprocess.run([sys.executable, str(previous / 'prepare.py')], check=True)
features = json.loads((previous / 'features.json').read_text())
for count in [1, 256]:
    features.append(dict(name=f'footnotes-unused-{count}', markdown=''.join(
        f'[^note-{n}]: unused {n}\n\n' for n in range(count))))
features.append(dict(name='footnotes-before-calls-256', markdown=''.join(
    f'[^note-{n}]: details {n}\n\n' for n in range(256)) + ''.join(
    f'one[^note-{n}] two[^note-{n}]\n\n' for n in reversed(range(256)))))
(s / 'features.json').write_text(json.dumps(features) + '\n')
controls = []
for marker, kind in [('`', 'code'), ('$', 'math')]:
    for name, value in [
        ('plain', 'ordinary é prose ' * 8),
        ('sparse', ('x' + marker + 'y' + marker * 3 + 'z ') * 64),
        ('dense-64', ''.join(marker * n + ' ' for n in range(1, 65))),
        ('dense-1024', ''.join(marker * n + ' ' for n in range(1, 1025))),
        ('dense-4096', ''.join(marker * n + ' ' for n in range(1, 4097))),
    ]:
        controls.append(dict(name=f'{kind}-{name}', leaves=[[kind, value]]))
    controls.append(dict(name=f'{kind}-many-small', repeat=256,
        leaves=[[kind, f'x{marker}y'], ['text', ' ']]))
(s / 'delimiter-controls.json').write_text(json.dumps(controls) + '\n')
probes = []
for marker, kind in [('`', 'code'), ('$', 'math')]:
    for minimum in [1, 2]:
        for count in [0, 1, 2, 3, 63, 64, 65, 127, 128, 129]:
            for gap in [0, 1, 2, 63, 64, 65, 127, 128]:
                value = ''.join(marker * n + 'é ' for n in range(1, count + 1) if n != gap)
                probes.append(dict(name=f'{kind}-{minimum}-{count}-{gap}',
                    double_math=minimum == 2, leaves=[[kind, value]]))
(s / 'delimiter-probes.json').write_text(json.dumps(probes) + '\n')
print('Prepared', len(controls), 'controls and', len(probes), 'delimiter probes.')

sparse = []
for size in [256, 4096]:
    definitions = ''.join(f'[^note-{n}]: details {n}\n\n' for n in range(size))
    for count in [1, 2, 4, 16]:
        calls = ''.join(f'one[^note-{n}] ' for n in range(size-count, size))
        sparse.append(dict(name=f'footnotes-{count}-calls-{size}-definitions', markdown=calls + '\n\n' + definitions))
(s / 'sparse-calls.json').write_text(json.dumps(sparse) + '\n')
