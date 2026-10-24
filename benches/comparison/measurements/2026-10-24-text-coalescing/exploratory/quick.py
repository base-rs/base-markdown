from pathlib import Path
import hashlib,json,os,shutil,subprocess,sys
p=Path('/Users/samuel/Developer/core-rs/core-markdown/benches/comparison');out=p/'results/text-coalescing';out.mkdir(exist_ok=True)
stage=sys.argv[1];binaries={'baseline':Path(sys.argv[2]),'candidate':p/'target/release/markdown-comparison'};launch=Path('/tmp/core-markdown-coalesce-run')
env=dict(os.environ,GEM_HOME='/tmp/core-markdown-comparison/gems',GEM_PATH='/tmp/core-markdown-comparison/gems')
for pair in range(2):
 for name in (['baseline','candidate'] if pair%2==0 else ['candidate','baseline']):
  shutil.copyfile(binaries[name],launch);launch.chmod(0o755)
  for fixture,mode in [('commonmark','core-markdown'),('gfm','core-markdown'),('references','core-markdown'),('scanners','core-markdown'),('mixed-1m','core-events')]:
   path=out/'mixed-1m.json' if fixture=='mixed-1m' else p/f'data/bench-{fixture}.json';profile='gfm' if fixture=='gfm' else 'commonmark'
   raw=subprocess.check_output([str(launch),'bench',mode,profile,str(path)],cwd=p,env=env,text=True)
   (out/f'{stage}-{pair}-{name}-{fixture}-{mode}.jsonl').write_text(raw)
   for r in map(json.loads,raw.splitlines()):print(stage,pair,name,mode,r['document'],r['median_seconds']*1000,flush=True)
(out/f'{stage}-binaries.json').write_text(json.dumps({k:hashlib.sha256(v.read_bytes()).hexdigest() for k,v in binaries.items()},indent=2)+'\n')
