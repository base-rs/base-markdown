"""Prepare original inputs and generated serializer controls without duplicating source fixtures."""
import itertools
import json
from pathlib import Path
import shutil
import subprocess
import sys

study = Path(__file__).resolve().parent
previous = study.with_name('2026-10-25-static-separators-and-removal')
subprocess.run([sys.executable, str(previous / 'prepare.py')], check=True)
shutil.copyfile(previous / 'original.json', study / 'original.json')
def sized(unit, minimum):
    return unit*((minimum+len(unit.encode())-1)//len(unit.encode()))
controls = [
    dict(name='code-plain', leaves=[['code','ordinary é α code'],['text',' ']],repeat=2048),
    dict(name='code-marker-runs', leaves=[['code','one ` two `` three ````'],['text',' ']],repeat=2048),
    dict(name='code-multiline', leaves=[['code','one\r\n# two\n12. three'],['text',' ']],repeat=1024),
    dict(name='math-plain', leaves=[['math','x + α'],['text',' ']],repeat=2048),
    dict(name='math-flow-100k',container='root',leaves=[['math-flow',sized('α + β $$ x\n',100000)]]),
    dict(name='fenced-code-1m',markdown='```rust\n'+sized('ordinary é α code\n',1000000)+'```\n'),
    dict(name='frontmatter-100k',markdown='```yaml\n'+sized('key: é α ` value\n',100000)+'```\n'),
    dict(name='table-code-256',markdown='| a | b |\n| - | - |\n'+('| `a\\|b` | ``c ` d`` |\n'*256)),
    dict(name='adjacent-ascii',leaves=[['text','ordinary text ']],repeat=2048),
    dict(name='adjacent-unicode',leaves=[['text','é α 世界 ']],repeat=2048),
    dict(name='adjacent-punctuation',leaves=[['text','* alpha'],['text','\\ beta'],['text',' space']],repeat=512),
]
(study/'controls.json').write_text(json.dumps(controls)+'\n')
small = [case | dict(name=case['name']+'-8',repeat=8) for case in controls
         if case['name'] in ['code-plain','math-plain','adjacent-ascii','adjacent-unicode','adjacent-punctuation']]
(study/'small-controls.json').write_text(json.dumps(small)+'\n')
(study/'all-controls.json').write_text(json.dumps(controls+small)+'\n')
probes = []
def add(**case):
    probes.append(dict(name=f'probe-{len(probes)}',**case))
tokens = ['a','é','`','$',' ','\t','\r','\n','#','>','1','.','|','\\']
for size in range(4):
    for parts in itertools.product(tokens,repeat=size):
        for kind in ['code','math']:
            add(leaves=[[kind,''.join(parts)]],double_math=(len(probes)//2)%2==0)
for first in [chr(n) for n in range(128)]+['','é','α','世','\u00a0']:
    for after in ['', 'ordinary', '\\!', '\r\n# two']:
        for container in ['paragraph','heading','quote','footnote','link','image','task']:
            for unwrap in [False,True]:
                for kind in ['text','soft']:
                    add(container=container,unwrap=unwrap,leaves=[['text','before\\'],[kind,first+after]])
for size in range(68):
    for kind,marker in [('code','`'),('math','$')]:
        add(leaves=[[kind,'é'+marker*size+' α '+marker*(size+2)]])
for before in ['text','code','math','emphasis','strong']:
    for tail in ['text','item','emphasis','strong']:
        for invalid in ['quote','bullet','emphasis','strong','fence']:
            add(leaves=[[before,'*a*'],[tail,'ordinary']],invalid=invalid)
for ticks in range(1,9):
    for slashes in range(5):
        content='`'*ticks+'a'+'\\'*slashes+'|b'+'`'*ticks
        add(markdown='| a | b |\n| - | - |\n| '+content+' | tail |\n')
(study/'probes.json').write_text(json.dumps(probes)+'\n')
print('Prepared',len(controls),'controls and',len(probes),'output/diagnostic probes.')
