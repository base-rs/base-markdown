from pathlib import Path
import json, subprocess
root=Path('/Users/samuel/Developer/core-rs/core-markdown')
p=root/'benches/comparison'
m=p/'measurements/2026-10-24-hot-paths'
runner=str(p/'target/release/markdown-comparison')
import hashlib
old=Path('/tmp/core-markdown-comparison')
rows=[]
cases=[('commonmark','commonmark',p/'data/commonmark.json'),('gfm-official','gfm',p/'data/gfm-official.json'),('gfm-spec','gfm',old/'data/gfm-spec.json')]
cases += [('custom',profile,p/'fixtures/custom.json') for profile in ['default','commonmark','gfm','gfm-footnotes']]
for name,profile,path in cases:
    expected=json.loads((old/'results'/f'{name}-{profile}-core-markdown.json').read_text())
    results={engine:json.loads(subprocess.check_output([runner,'render',engine,profile,str(path)],cwd=p,text=True)) for engine in ['core-markdown','core-stream','core-arena']}
    for engine,result in results.items(): assert result==expected,(name,profile,engine)
    digest=hashlib.sha256(json.dumps(results['core-markdown'],ensure_ascii=False).encode()).hexdigest()
    rows.append(dict(corpus=name,profile=profile,outputs=len(expected),byte_identical_to_original=True,checked_events_identical_to_direct=True,arena_identical_to_direct=True,input_json_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),output_array_sha256=digest))
(m/'preservation.json').write_text(json.dumps(rows,indent=2)+'\n')
print('Preserved',sum(row['outputs'] for row in rows),'saved outputs across direct, checked events, and arena rendering.',flush=True)
