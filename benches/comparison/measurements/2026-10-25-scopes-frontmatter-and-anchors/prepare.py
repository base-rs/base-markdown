"""Reuse established fixtures and add heading collisions and frontmatter boundaries."""
from itertools import product
import json
from pathlib import Path
import subprocess
import sys

study = Path(__file__).resolve().parent
previous = study.with_name('2026-10-25-footnotes-indexes-and-escape-context')
subprocess.run([sys.executable, str(previous/'prepare.py')], check=True)
controls = []
for kind, titles in [('unique', [f'Heading {n}' for n in range(512)]),
                     ('repeated', ['Same']*512),
                     ('suffixes', [title for n in range(128) for title in ['A', f'A-{n+2}', 'A-2', '']])]:
    source = ''.join(f'# {title}\n\n' for title in titles)
    for operation in ['html', 'headings']:
        controls.append(dict(name=f'{operation}-{kind}-512',markdown=source,operation=operation,headings=True))
for opening, closing, name in [('---','---','dash'),('~~~json','~~~','tilde'),('```yaml','````','backtick')]:
    for size in [32, 100_000]:
        controls.append(dict(name=f'frontmatter-{name}-{size}',operation='html',frontmatter=True,
                             markdown=opening+'\n'+('x'*(size-1))+'\n'+closing+'\n\ntext\n'))
(study/'targeted.json').write_text(json.dumps(controls)+'\n')
probes=[]
for titles in product(['A','a-2','a-3','','-2'], repeat=4):
    probes.append(dict(name=f'headings-{len(probes)}',markdown=''.join(f'# {title}\n\n' for title in titles),operation='html',headings=True))
for marker, length, info, newline, indent, extra in product(['-','+','`','~'],[2,3,4],['','yaml',' yaml meta','a`b',' é '],['\n','\r','\r\n'],['',' ','   ','    ','\t'],[0,1]):
    probes.append(dict(name=f'frontmatter-{len(probes)}',operation='html',frontmatter=True,
        markdown=marker*length+info+newline+'body'+newline+indent+marker*(length+extra)+' \t'+newline))
(study/'probes.json').write_text(json.dumps(probes)+'\n')
print('Prepared',len(controls),'targeted controls and',len(probes),'additional probes.')
