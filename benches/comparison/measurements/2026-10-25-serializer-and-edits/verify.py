"""Compare corpus/probe outputs, complete edited arenas, and allocation diagnostics outside timing."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
import subprocess

parser = argparse.ArgumentParser(__doc__)
parser.add_argument('--work', type=Path, required=True)
args = parser.parse_args()
study = Path(__file__).resolve().parent
comparison = study.parents[1]
work = args.work.resolve()
sha = lambda data: hashlib.sha256(data).hexdigest()
cohorts = []
gfm_spec = work / 'gfm-spec.json'
gfm_spec.write_bytes(gzip.decompress((study.with_name('2026-10-25-normalize-identifiers') / 'gfm-spec.json.gz').read_bytes()))
paths = {'commonmark': comparison / 'data/commonmark.json', 'gfm-official':comparison / 'data/gfm-official.json',
         'gfm-spec':gfm_spec, 'custom':comparison / 'fixtures/custom.json'}
for row in json.loads((study.with_name('2026-10-25-event-delivery') / 'preservation.json').read_text())['corpus']:
    path = paths[row['corpus']]
    assert sha(path.read_bytes()) == row['input_sha256']
    outputs = [subprocess.check_output([str(work / (engine+'-html')),'render',row['mode'],row['profile'],str(path)]) for engine in ['baseline','candidate']]
    assert outputs[0] == outputs[1], row
    cohorts.append(row | dict(output_sha256=sha(outputs[0])))
timed_outputs = []
for profile, filename in [('commonmark','bench-commonmark.json'),('gfm','bench-gfm.json')]:
    path = comparison / 'data' / filename
    for mode in ['core-markdown','core-stream','core-arena','core-markdown-output']:
        outputs = [subprocess.check_output([str(work/(engine+'-html')),'render',mode,profile,str(path)]) for engine in ['baseline','candidate']]
        assert outputs[0] == outputs[1], (filename,mode)
        timed_outputs.append(dict(input=filename,profile=profile,mode=mode,count=len(json.loads(outputs[0])),output_sha256=sha(outputs[0])))
probes = []
for folder, filename in [('2026-10-25-single-parser','cases.jsonl.gz'),
                         ('2026-10-25-single-parser','generated-cases.jsonl.gz'),
                         ('2026-10-25-normalize-identifiers','label-boundaries.jsonl.gz'),
                         ('2026-10-25-normalize-identifiers','boundary-cases.jsonl.gz'),
                         ('2026-10-25-inline-destinations','destination-boundaries.jsonl.gz')]:
    data = gzip.decompress((study.with_name(folder) / filename).read_bytes())
    outputs = [subprocess.check_output([str(work/(engine+'-driver'))],input=data) for engine in ['baseline','candidate']]
    assert outputs[0] == outputs[1],filename
    rows = [json.loads(line) for line in outputs[0].splitlines()]
    assert all(not row.get('panic') and row['html']==row['checked']==row['arena'] for row in rows)
    probes.append(dict(input=f'../{folder}/{filename}',count=len(rows),input_sha256=sha(data),output_sha256=sha(outputs[0])))
outputs = [subprocess.check_output([str(work/(engine+'-bench')),'check','replace',str(study/'edit-probes.json')]) for engine in ['baseline','candidate']]
assert outputs[0] == outputs[1], 'edited arenas'
report = dict(corpus=cohorts,timed_outputs=timed_outputs,probes=probes,edited_arenas=dict(count=len(outputs[0].splitlines()),output_sha256=sha(outputs[0])))
(work/'preservation.json').write_text(json.dumps(report,indent=2)+'\n')
allocations = []
for mode, filename in [('html','original.json'),('markdown','original.json'),('replace','replace.json'),('move','move.json')]:
    results = {engine:[json.loads(line) for line in subprocess.check_output([str(work/(engine+'-allocations')),'alloc',mode,str(study/filename)]).splitlines()] for engine in ['baseline','candidate']}
    for before,after in zip(results['baseline'],results['candidate']):
        assert before['document']==after['document'] and before['counts']['result']==after['counts']['result']
        allocations.append(dict(mode=mode,document=before['document'],baseline=before['counts'],candidate=after['counts'],
            deltas={key:after['counts'][key]-value for key,value in before['counts'].items()}))
(work/'allocations.json').write_text(json.dumps(allocations,indent=2)+'\n')
print('Preserved',sum(row['outputs'] for row in cohorts),'corpus outputs,',sum(row['count'] for row in probes),'probes, and',report['edited_arenas']['count'],'complete edited arenas.')
print('Collected',len(allocations),'allocation results.')
