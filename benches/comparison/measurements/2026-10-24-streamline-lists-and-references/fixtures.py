"""Deterministic list-scaling and retained grammar-fallback workloads."""
import json
from pathlib import Path
import sys

out = Path(sys.argv[1])
out.mkdir(parents=True, exist_ok=True)
lists = [dict(name=f'ordered-list-{count}', markdown=''.join(
    f'{number}. value\n' for number in range(9, count + 9)))
    for count in [256, 2048, 8192]]
(out / 'lists.json').write_text(json.dumps(lists))

grammar = []
for count in [256, 1024, 4096]:
    references = ' '.join(f'[r{number}]' for number in range(count))
    definitions = ''.join(f'[r{number}]: /{number}\n' for number in range(count))
    grammar.append(dict(name=f'grammar-definitions-{count}', markdown=(
        'before <i\n title=x> ' + references + '\n\n' + definitions)))
count = 4096
references = ' '.join(f'[r{number}]' for number in range(count))
definitions = ''.join(f'[r{number}]: /{number}\n "title"\n' for number in range(count))
grammar.append(dict(name='grammar-full-definitions-4096', markdown=references + '\n\n' + definitions))
grammar.append(dict(name='grammar-full-no-links-4096', markdown=definitions))
unit = 'compact [a]\n\nfirst <i\n title=x>HTML</i> [a]\n\ncompact [a]\n\nlast <b\n title=x>HTML</b> [a]\n\n'
grammar.append(dict(name='grammar-reuse-100k', markdown=unit * (100_000 // len(unit)) + '[a]: /a\n'))
(out / 'grammar.json').write_text(json.dumps(grammar))

text = 'plain é text ' * (100_000 // len('plain é text '.encode()))
(out / 'line-ending.json').write_text(json.dumps([
    dict(name='no-line-ending-100k', markdown=text),
    dict(name='late-inline-fallback-100k', markdown=text + '<i\n title=x>HTML</i>\n'),
]))
