"""Reuse frozen fixture generators and add footnote/index and context-boundary controls."""
import json
from pathlib import Path
import shutil
import subprocess
import sys
s=Path(__file__).resolve().parent
previous=s.with_name('2026-10-25-serializer-rewrites-and-lookahead')
subprocess.run([sys.executable,str(previous/'prepare.py')],check=True)
for name in ['original.json','all-controls.json','small-controls.json']:
    shutil.copyfile(previous/name,s/name)
shutil.copyfile(previous/'all-controls.json',s/'controls.json')
shutil.copyfile(previous/'small-controls.json',s/'stage-controls.json')
features=[
 dict(name='footnote-single',markdown='one[^note].\n\n[^note]: details\n'),
 dict(name='footnote-repeated-64',markdown=('one[^note] '*64)+'\n\n[^note]: details\n'),
 dict(name='footnotes-256',markdown=''.join(f'one[^note-{n}] two[^note-{n}] three[^note-{n}]\n\n' for n in range(256))+'\n'.join(f'[^note-{n}]: details {n}\n' for n in range(256))),
 dict(name='footnote-blocks-and-options',prefix='<&é',back_label='back \"<&',markdown='one[^É] two[^É].\r\n\r\n[^É]: first\r\n\r\n    second\r\n\r\n    ```\r\n    code\r\n    ```\r\n'),
 dict(name='references-4096',markdown=''.join(f'[{n}][note-{n}] ' for n in range(0,4096,16))+'\n\n'+''.join(f'[note-{n}]: /destination/{n} "title"\n' for n in range(4096))),
 dict(name='duplicate-reference-and-footnote-keys',markdown=''.join(f'[link][key-{n}] note[^key-{n}]\n\n' for n in range(128))+''.join(f'[key-{n}]: /first/{n}\n[KEY-{n}]: /second/{n}\n[^key-{n}]: first {n}\n[^KEY-{n}]: second {n}\n' for n in reversed(range(128)))),
]
(s/'features.json').write_text(json.dumps(features)+'\n')
probes=json.loads((previous/'probes.json').read_text())
for before in ['', 'x', '\\', '*\n*\n*\n', '-\r\n-\r\n-\r\n', '\r\n \t12', 'é α ]', '\n  ', '[', '&']:
 for value in ['', 'ordinary text ', 'é α 世界 ', '* next', '. next', ') next', '&&amp;', '\r\n # heading', '\t\r\n', '\\!']:
  for after in ['', '\n', '*', ')']:
   for container in ['paragraph','heading','link','image','task']:
    probes.append(dict(name=f'context-{len(probes)}',container=container,leaves=[['text',before],['text',value],['text',after]]))
(s/'probes.json').write_text(json.dumps(probes)+'\n')
print('Prepared',len(features),'feature controls and',len(probes),'serializer probes.')

large=json.loads((s/'controls.json').read_text())
frontmatter=next(case for case in large if case['name']=='fenced-code-1m')
fences=[frontmatter,dict(name='ordinary-code-fence-1m',markdown='intro\n\n'+frontmatter['markdown'])]
(s/'fence-controls.json').write_text(json.dumps(fences)+'\n')
