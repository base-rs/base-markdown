"""Prepare deterministic maintenance benchmarks from the established original four inputs."""
import json
from pathlib import Path

study = Path(__file__).resolve().parent
comparison = study.parents[1]
original = []
for profile, path in [('commonmark', 'bench-commonmark.json'), ('gfm', 'bench-gfm.json')]:
    original.extend(row | dict(profile=profile) for row in json.loads((comparison / 'data' / path).read_text()))
(study / 'original.json').write_text(json.dumps(original) + '\n')

prefix = '# Keep\n\n' + '\n\n'.join('retained paragraph ' + str(i) for i in range(64))
target = prefix + '\n\n# Replace\n\nold\n\n# End\n\nsuffix'
replacements = []
for size in [64, 256, 1024]:
    flat = '\n\n'.join(f'paragraph {i} with *emphasis* and [a link](/url "title")' for i in range(size))
    replacements.append(dict(name=f'flat-{size}', markdown=flat, target=target))
for size in [32, 128]:
    nested = '\n\n'.join(f'> a {i} *b **c** d* e\n>\n> > nested\n>\n> tail\n\n- a\n  - b\n- c' for i in range(size))
    replacements.append(dict(name=f'nested-{size}', markdown=nested, target=target))
(study / 'replace.json').write_text(json.dumps(replacements) + '\n')
(study / 'move.json').write_text(json.dumps([
    dict(name=f'list-{size}',markdown='\n'.join(f'> - item {i} with *emphasis*' for i in range(size)))
    for size in [64, 256, 1024]
]) + '\n')

# Small structural controls vary nesting, siblings, retained prefixes and insertion boundaries.
shapes = ['', 'text', '*a **b** c* d', '[a](/u "title")', '- a\n  - b\n- c',
          '> a\n>\n> > b\n>\n> c', '```rust meta\nx\n```',
          'a | b\n-- | --\nc | d', '[ref]: /u\n\n[ref]', '# Heading\n\nbody']
probes = []
for index, shape in enumerate(shapes):
    for count in [1, 2, 4]:
        for prefix in ['', '# Keep\n\nretained\n\n']:
            for suffix in ['', '# End\n\nsuffix']:
                probes.append(dict(name=f'copy-{len(probes)}', profile='gfm',
                    markdown='\n\n'.join([shape] * count), target=prefix+'# Replace\n\nold\n\n'+suffix))
(study / 'edit-probes.json').write_text(json.dumps(probes)+'\n')
