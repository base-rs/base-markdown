from pathlib import Path
import json,os,shutil,subprocess,statistics
p=Path('/Users/samuel/Developer/core-rs/core-markdown/benches/comparison');out=p/'measurements/2026-10-24-commonmark-events';launch=Path('/tmp/core-markdown-commonmark-events-final/run')
binaries={'baseline':Path('/tmp/core-markdown-commonmark-events-baseline'),'spans':Path('/tmp/core-markdown-commonmark-events-spans'),'candidate':p/'target/release/markdown-comparison'}
env=dict(os.environ,GEM_HOME='/tmp/core-markdown-comparison/gems',GEM_PATH='/tmp/core-markdown-comparison/gems');records={}
for pair in range(1,4):
 for name in (['baseline','spans','candidate'] if pair%2 else ['candidate','spans','baseline']):
  shutil.copyfile(binaries[name],launch);launch.chmod(0o755)
  raw=subprocess.check_output([str(launch),'bench','core-markdown','commonmark',str(p/'data/bench-scanners.json')],cwd=p,env=env,text=True)
  (out/f'scanner-check-{pair}-{name}.jsonl').write_text(raw)
  for r in map(json.loads,raw.splitlines()):records.setdefault(r['document'],{}).setdefault(name,[]).append(r['median_seconds']*1000);print(pair,name,r['document'],r['median_seconds']*1000,flush=True)
print(json.dumps(records,indent=2))
