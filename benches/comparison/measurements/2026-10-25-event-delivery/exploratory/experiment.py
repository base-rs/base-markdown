import argparse, hashlib, json, shutil, statistics, subprocess
from pathlib import Path
from datetime import datetime, timezone
p=argparse.ArgumentParser()
p.add_argument('name')
p.add_argument('--engines',nargs='+',required=True)
p.add_argument('--modes',nargs='+',default=['core-events','core-markdown'])
p.add_argument('--inputs',nargs='+',default=['bench-commonmark.json','bench-gfm.json'])
p.add_argument('--rounds',type=int,default=2)
a=p.parse_args();work=Path(__file__).resolve().parent
comparison=Path('/Users/samuel/Developer/core-rs/core-markdown/benches/comparison')
records=[];stamp=datetime.now(timezone.utc).isoformat()
for round_number in range(1,a.rounds+1):
 order=a.engines if round_number%2 else list(reversed(a.engines))
 for engine in order:
  binary=work/engine
  for mode in a.modes:
   for filename in a.inputs:
    profile='gfm' if 'gfm' in filename else 'commonmark'
    launch=work/'run';launch.unlink(missing_ok=True);shutil.copyfile(binary,launch);launch.chmod(0o755)
    result=subprocess.check_output([str(launch),'bench',mode,profile,str(work/filename)],cwd=comparison,text=True)
    row=dict(experiment=a.name,run_id=stamp,round=round_number,engine=engine,mode=mode,profile=profile,input=filename,input_sha256=hashlib.sha256((work/filename).read_bytes()).hexdigest(),binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),results=[json.loads(line) for line in result.splitlines()])
    records.append(row)
    with (work/(a.name+'.jsonl')).open('a') as f:f.write(json.dumps(row)+'\n')
    print(round_number,engine,mode,[(r['document'],round(r['median_seconds']*1000,6)) for r in row['results']],flush=True)
for mode in a.modes:
 for document in sorted({r['document'] for row in records for r in row['results']}):
  times={engine:statistics.median(r['median_seconds'] for row in records if row['engine']==engine and row['mode']==mode for r in row['results'] if r['document']==document) for engine in a.engines}
  control=times[a.engines[0]]
  print(mode,document,{e:dict(ms=round(t*1000,6),percent=round((t/control-1)*100,2)) for e,t in times.items()},flush=True)
