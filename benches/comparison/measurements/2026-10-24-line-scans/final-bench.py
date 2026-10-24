from pathlib import Path
from datetime import datetime, timezone
import hashlib,json,os,shutil,statistics,subprocess
p=Path('/Users/samuel/Developer/core-rs/core-markdown/benches/comparison'); repo=p.parent.parent
out=p/'measurements/2026-10-24-line-scans'; out.mkdir(exist_ok=True)
work=Path('/tmp/core-markdown-lines-final');work.mkdir(exist_ok=True);launch=work/'run'
binaries={'baseline':Path('/tmp/core-markdown-lines-baseline'),'candidate':p/'target/release/markdown-comparison'}
env=dict(os.environ,GEM_HOME='/tmp/core-markdown-comparison/gems',GEM_PATH='/tmp/core-markdown-comparison/gems')
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
inputs={}
original=json.loads((p/'measurements/2026-10-23/metadata.json').read_text())
for fixture in ['commonmark','gfm','references','definitions','scanners']:
 for c in json.loads((p/f'data/bench-{fixture}.json').read_text()):
  digest=hashlib.sha256(c['markdown'].encode()).hexdigest()
  if fixture in ['commonmark','gfm']:assert digest==original['inputs'][c['name']]['sha256']
  inputs[c['name']]={'bytes':len(c['markdown'].encode()),'sha256':digest}
metadata={'timestamp_utc':datetime.now(timezone.utc).isoformat(),'candidate_head':subprocess.check_output(['git','rev-parse','HEAD'],cwd=repo,text=True).strip(),'baseline_head':'ad900980b070239db52c90d145dc0d77f54d4d24','binary_sha256':{k:sha(v) for k,v in binaries.items()},'block_parser_sha256':sha(repo/'src/block_parser/mod.rs'),'machine':subprocess.check_output(['sysctl','-n','machdep.cpu.brand_string'],text=True).strip(),'rust':subprocess.check_output(['rustc','--version'],text=True).strip(),'ruby':subprocess.check_output(['ruby','--version'],text=True).strip(),'inkmark':subprocess.check_output(['bundle','exec','ruby','-rinkmark','-e','print Gem.loaded_specs["inkmark"].version'],cwd=p,env=env,text=True),'inputs':inputs,'method':'Three alternating baseline/candidate/Inkmark runs of original four workloads, references, and scanner regressions. Same launch path and environment for Rust revisions, seven calibrated samples per process, 200ms warmup. Definitions are single-process regression observations. Preparation, public event consumption, and arena parse/render also use three alternating pairs. Sequential with no concurrent project builds/tests/review workloads; no machine-wide isolation. GFM HTML differs between engines; see output agreement. Small mixed-workload differences remain process-layout sensitive.'}
if not (out/'metadata.json').exists(): (out/'metadata.json').write_text(json.dumps(metadata,indent=2)+'\n')
else:
 saved=json.loads((out/'metadata.json').read_text());assert saved['binary_sha256']==metadata['binary_sha256'] and saved['candidate_head']==metadata['candidate_head']
manifest=json.loads((out/'run-manifest.json').read_text()) if (out/'run-manifest.json').exists() else []; records=[]
for entry in manifest:
 for line in (out/entry['file']).read_text().splitlines(): records.append(dict(run=Path(entry['file']).stem,engine=entry['engine'],**json.loads(line)))
def run(name,engine,fixture,mode='core-markdown',allocation=False):
 profile='gfm' if fixture=='gfm' else 'commonmark';input=p/f'data/bench-{fixture}.json'; dest=out/f'{name}.jsonl'
 if dest.exists():return
 if engine=='inkmark':cmd=['bundle','exec','ruby','adapter.rb','inkmark','bench',profile,str(input)];binary=None
 else:
  binary=binaries[engine]
  if allocation:binary=Path('/tmp/core-markdown-lines-baseline-allocations') if engine=='baseline' else p/'target/release/allocations'
  launch.unlink(missing_ok=True);shutil.copyfile(binary,launch);launch.chmod(0o755)
  cmd=[str(launch),profile,str(input)] if allocation else [str(launch),'bench',mode,profile,str(input)]
 print('START',name,flush=True);raw=subprocess.check_output(cmd,cwd=p,env=env,text=True);dest.write_text(raw)
 for line in raw.splitlines():
  row=json.loads(line);records.append(dict(run=name,engine=engine,**row));print(name,row['document'],row.get('median_seconds',row.get('html_total')),flush=True)
 manifest.append({'file':dest.name,'engine':engine,'mode':'allocations' if allocation else mode,'command':cmd,'input_sha256':sha(input),'binary_sha256':sha(binary) if binary else None})
 (out/'run-manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
for pair in range(1,4):
 for engine in (['baseline','candidate','inkmark'] if pair%2 else ['inkmark','candidate','baseline']):
  for fixture in ['commonmark','gfm','references','scanners']:run(f'paired-{pair}-{engine}-{fixture}',engine,fixture)
for fixture in ['definitions']:
 for engine in ['baseline','candidate','inkmark']:run(f'{fixture}-{engine}',engine,fixture)
for pair in range(1,4):
 for engine in (['baseline','candidate'] if pair%2 else ['candidate','baseline']):
  for fixture in ['commonmark','gfm']:
   for mode in ['core-prepare','core-events','core-arena']:run(f'phase-{pair}-{engine}-{fixture}-{mode}',engine,fixture,mode)
for engine in ['baseline','candidate']:
 for fixture in ['commonmark','gfm','references']:run(f'allocations-{engine}-{fixture}',engine,fixture,allocation=True)
summary={}
for name in inputs:
 row={}
 for engine in ['baseline','candidate','inkmark']:
  values=[r['median_seconds']*1000 for r in records if r['engine']==engine and r['document']==name and (r['run'].startswith('paired-') or r['run']==f'definitions-{engine}' or r['run']==f'scanners-{engine}')]
  if values:row[engine]={'median_ms':statistics.median(values),'process_medians_ms':values}
 if row:
  row['baseline_over_candidate']=row['baseline']['median_ms']/row['candidate']['median_ms'];row['inkmark_over_candidate']=row['inkmark']['median_ms']/row['candidate']['median_ms'];summary[name]=row
(out/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print('SUMMARY',json.dumps(summary,indent=2),flush=True)
