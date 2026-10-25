#!/usr/bin/env python3
import hashlib,itertools,json,pathlib,subprocess
import argparse
parser=argparse.ArgumentParser(description="Compare resolved output, authored syntax and source spans.")
parser.add_argument('--work',type=pathlib.Path,required=True)
parser.add_argument('--candidate',default='final')
args=parser.parse_args()
root=args.work.resolve()
atoms=['-\n','  x\n','\n','  \n','\t\n','  >\n','  [x]: /u\n']
cases=[]
for length in range(5):
 for parts in itertools.product(atoms,repeat=length):
  raw=''.join(parts)
  for ending in ['\n','\r','\r\n']:
   cases.append(dict(name=f'container-{len(cases)}',profile='commonmark',markdown=raw.replace('\n',ending)))
for parts in itertools.product(atoms,repeat=3):
 raw=''.join('> '+part for part in parts)
 for ending in ['\n','\r','\r\n']:
  cases.append(dict(name=f'quoted-{len(cases)}',profile='commonmark',markdown=raw.replace('\n',ending)))
probes=[dict(markdown=value) for value in [
 '- > - a\n  >\n  > - b\n  >\n  > -\n\noutside\n',
 '- > - a\n  >\n  > -\n\noutside\n',
 '- > - a\n  >\n  > -\n  outer content\n\noutside\n',
]]
for probe in probes:
 for tail in ['outside\n','- outside\n','[r]: /u\n','# outside\n']:
  for ending in ['\n','\r','\r\n']:
   cases.append(dict(name=f'probe-{len(cases)}',profile='commonmark',markdown=probe['markdown'].replace('outside\n',tail).replace('\n',ending)))
path=root/'data/containers.json';path.write_text(json.dumps(cases,ensure_ascii=False))
rows=[]
for data in [root/'data/original.json',root/'data/real.json',root/'data/focused.json',root/'data/lists.json',path]:
 outputs={label:subprocess.check_output([str(root/(label+'-diagnostics')),'check',str(data)]) for label in ['baseline',args.candidate]}
 a=outputs['baseline'].splitlines();b=outputs[args.candidate].splitlines();assert len(a)==len(b)
 mismatches=[]
 for before,after in zip(a,b):
  if before!=after:
   x=json.loads(before);y=json.loads(after);mismatches.append(dict(name=x['name'],fields=[key for key in x if x[key]!=y[key]],baseline=x,candidate=y))
 row=dict(input=data.name,input_sha256=hashlib.sha256(data.read_bytes()).hexdigest(),cases=len(a),baseline_output_sha256=hashlib.sha256(outputs['baseline']).hexdigest(),candidate_output_sha256=hashlib.sha256(outputs[args.candidate]).hexdigest(),mismatches=len(mismatches));rows.append(row)
 print(row,flush=True)
 if mismatches:
  (root/(data.stem+'-mismatches.json')).write_text(json.dumps(mismatches,indent=2));raise SystemExit('mismatches')
(root/('preservation-'+args.candidate+'.json')).write_text(json.dumps(rows,indent=2)+'\n')
