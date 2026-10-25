"""Reuse established fixtures and prepare controls for static separators, unwrapping and range removal."""
import itertools
import json
from pathlib import Path
import shutil
import subprocess
import sys

study = Path(__file__).resolve().parent
previous = study.with_name('2026-10-25-serializer-and-edits')
subprocess.run([sys.executable, str(previous / 'prepare.py')], check=True)
for name in ['original.json','replace.json','move.json','edit-probes.json']:
    shutil.copyfile(previous / name, study / name)
(study/'join.json').write_text(json.dumps([
    dict(name=f'tight-list-{size}',markdown='\n'.join(f'- item {i}' for i in range(size)))
    for size in [64,256,1024]
])+'\n')
unwrap = json.loads((study/'original.json').read_text())
for name, unit in [('plain','ordinary text é α '),('multiline','ordinary text \t\r\n \tmore text\n'),('unicode-lines','é\u00a0 \r\n \tα\n\r\n')]:
    unwrap.append(dict(name=name+'-100k',markdown=unit*((100000+len(unit.encode())-1)//len(unit.encode())),synthetic=True))
unwrap.append(dict(name='late-break-100k',markdown=next(case['markdown'] for case in unwrap if case['name']=='plain-100k')+'\n',synthetic=True))
(study/'unwrap.json').write_text(json.dumps(unwrap)+'\n')
removals=[]
for prefix, count, suffix in [(0,64,True),(64,64,True),(64,1024,True),(1024,1024,True),(1024,1024,False)]:
    before = '# Keep\n\n'+'\n\n'.join(f'prefix {i}' for i in range(prefix))+'\n\n' if prefix else ''
    middle = '\n\n'.join(f'old {i} *emphasis*' for i in range(count))
    after = '\n\n# End\n\nsuffix' if suffix else ''
    removals.append(dict(name=f'prefix-{prefix}-remove-{count}'+('' if suffix else '-to-end'),markdown=before+'# Replace\n\n'+middle+after))
(study/'remove.json').write_text(json.dumps(removals)+'\n')
tokens=['a','é',' ','\t','\r','\n','\u00a0','\0']
probes=[]
for size in range(5):
    for parts in itertools.product(tokens,repeat=size):
        probes.append(dict(name=f'unwrap-{len(probes)}',markdown=''.join(parts),synthetic=True))
(study/'unwrap-probes.json').write_text(json.dumps(probes)+'\n')
