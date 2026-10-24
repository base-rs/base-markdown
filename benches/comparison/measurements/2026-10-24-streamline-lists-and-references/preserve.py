"""Save exact HTML and Markdown results from each immutable comparison adapter."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

parser=argparse.ArgumentParser(__doc__)
parser.add_argument('--binary', required=True, type=Path)
parser.add_argument('--output', required=True, type=Path)
parser.add_argument('--compare', type=Path)
args=parser.parse_args()
comparison=Path(__file__).resolve().parents[2]
cases=[('commonmark','commonmark',comparison/'data/commonmark.json'),
       ('gfm-official','gfm',comparison/'data/gfm-official.json'),
       ('gfm-spec','gfm',Path('/tmp/core-markdown-comparison/data/gfm-spec.json'))]
cases += [('custom',profile,comparison/'fixtures/custom.json') for profile in ['default','commonmark','gfm','gfm-footnotes']]
rows=[]
for corpus,profile,path in cases:
    for mode in ['core-markdown','core-stream','core-arena','core-markdown-output']:
        outputs=json.loads(subprocess.check_output([str(args.binary),'render',mode,profile,str(path)],cwd=comparison,text=True))
        rows.append(dict(corpus=corpus,profile=profile,mode=mode,input_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),outputs=outputs))
if args.compare:
    expected=json.loads(args.compare.read_text())
    assert len(expected)==len(rows)
    for old,new in zip(expected,rows):
        assert {k:v for k,v in old.items() if k!='outputs'} == {k:v for k,v in new.items() if k!='outputs'}
        for index,(before,after) in enumerate(zip(old['outputs'],new['outputs'])):
            assert before==after,(new['corpus'],new['profile'],new['mode'],index,before,after)
        assert len(old['outputs'])==len(new['outputs'])
args.output.write_text(json.dumps(rows,ensure_ascii=False)+'\n')
print('Saved',sum(len(row['outputs']) for row in rows),'results.')
