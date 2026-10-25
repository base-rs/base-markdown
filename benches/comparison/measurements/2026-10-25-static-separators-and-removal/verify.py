"""Replay established preservation checks and add affected-mode output/allocation comparisons."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys

parser=argparse.ArgumentParser(__doc__)
parser.add_argument('--work',type=Path,required=True)
args=parser.parse_args()
study=Path(__file__).resolve().parent
work=args.work.resolve()
previous=study.with_name('2026-10-25-serializer-and-edits')
subprocess.run([sys.executable,str(previous/'verify.py'),'--work',str(work)],check=True)
report=json.loads((work/'preservation.json').read_text())
report['affected_modes']=[]
allocations=json.loads((work/'allocations.json').read_text())
for mode,filename in [('join','join.json'),('unwrap','unwrap.json'),('unwrap','unwrap-probes.json'),('remove','remove.json')]:
    path=study/filename
    output=[subprocess.check_output([str(work/(engine+'-bench')),'check',mode,str(path)]) for engine in ['baseline','candidate']]
    assert output[0]==output[1],(mode,filename)
    report['affected_modes'].append(dict(mode=mode,input=filename,cases=len(output[0].splitlines()),input_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),output_sha256=hashlib.sha256(output[0]).hexdigest()))
    if filename=='unwrap-probes.json': continue
    counts={engine:[json.loads(line) for line in subprocess.check_output([str(work/(engine+'-allocations')),'alloc',mode,str(path)]).splitlines()] for engine in ['baseline','candidate']}
    for before,after in zip(counts['baseline'],counts['candidate']):
        assert before['document']==after['document'] and before['counts']['result']==after['counts']['result']
        allocations.append(dict(mode=mode,document=before['document'],baseline=before['counts'],candidate=after['counts'],deltas={key:after['counts'][key]-value for key,value in before['counts'].items()}))
(work/'preservation.json').write_text(json.dumps(report,indent=2)+'\n')
(work/'allocations.json').write_text(json.dumps(allocations,indent=2)+'\n')
print('Preserved',sum(row['cases'] for row in report['affected_modes']),'affected-mode outputs/edited arenas.')
print('Collected',len(allocations),'total allocation records.')
