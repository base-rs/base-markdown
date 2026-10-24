from pathlib import Path
import subprocess,json,struct,hashlib,random,itertools,collections,time
prefix='/tmp/core-markdown-line-scans-review'
cases={}
def add(s,kind):cases.setdefault(s,kind)
spaces=[''.join(s) for n in range(7) for s in itertools.product(' \t',repeat=n)]
containers=['','> ','- ','1. ','> - ','- > ','  >  - ','   1. > ']
bodies=['','x','世界 é🙂','*x* [x](target "title")','-','+','*','---','- - -','***','123456789. x','1234567890. x','1.x','-x','>','>> x','# x','####### x','```rust','~~~','    x','[a]: /x "t"','[a]: <>','[a]: /x','[^a]: footnote','<!-- x -->','<div>x</div>','<script>x</script>','<custom>','a | b','- | :-:','\\*x']
endings=['\n','\r','\r\n']
for wi,w in enumerate(spaces):
 for pi,p in enumerate(containers):
  for bi,b in enumerate(bodies):
   ending=endings[(wi+pi+bi)%3]
   add(p+w+b,'single')
   if wi<15:
    add(p+w+b+ending+p+w,'blank-eof')
blocks=['[a]: /target "title"\n\n[a]', '[a]: /target\n "title"\n\n[a]', '[a]: /target\n   ---\n\n[a]', '<!-- x -->\n\nx', '<!--\nx\n-->\n\nx', '<script>\nx\n</script>', '<div>\nx\n\nnext', '<?x\n?>','<![CDATA[\nx\n]]>','<!DOCTYPE x>','a | b\n- | :-:\nc | d', '[a]: /x | b\n- | -\nx | y', '```rust\n\t<x>\n```','~~~\nx\n~~~','---\n\n# x','[^a]: x\n\n[^a]']
for w,p,b,ending in itertools.product(spaces[:31],containers,blocks,endings):
 add(ending.join(p+w+line for line in b.split('\n')),'structured-provenance')
starts=['- x',' - x','1. x','   1. x','> x','> - x','- > x','> > x',' -','> -','> [a]: /x','- <!-- x -->','> a | b\n> - | -']
tails=['x','- y','> y','# y','[a]: /x "t"','<!-- x -->','<div>','---','```','a | b\n- | -','\t\tx','', '>\t', '-\t']
for head,w,tail in itertools.product(starts,spaces[:31],tails):
 for ending in endings:
  for middle in ['',ending+' \t']:
   add(head.replace('\n',ending)+ending+middle+w+tail.replace('\n',ending)+'\n\n[a]','continuation')
for first,second,p in itertools.product(spaces[:31],spaces[:31],containers):
 add(p+'x\n'+p+first+'\n'+p+second+'y\n'+p,'blank-tightness')
for ctrl,p,b in itertools.product(['\x0b','\x0c','\u00a0','\u2003','\ufeff','\x00'],containers,bodies):
 add(p+ctrl+b+'\n'+p+'x','whitespace-boundary')
rng=random.Random(106240)
for _ in range(12000):
 lines=[]
 for _ in range(rng.randrange(1,8)):
  lines.append(rng.choice(containers)+rng.choice(spaces)+rng.choice(bodies)+rng.choice(endings))
 add(''.join(lines).removesuffix(rng.choice(endings)),'random')
for depth in [8,16,32,64]:
 for p in ['> ','- ','1. ']:
  for w in spaces[:7]:
   add((p*depth)+w+'x\n'+(p*depth)+w+'\n'+(p*depth)+w+'[a]: /x\n\n[a]','deep')
records=[{'name':f'{kind}-{i}','markdown':s} for i,(s,kind) in enumerate(cases.items())]
content=(json.dumps(records,ensure_ascii=False)+'\n').encode();Path(prefix+'-cases.json').write_bytes(content)
counts=collections.Counter(cases.values())
print('sources',len(records),'categories',dict(counts),flush=True)
changed=[];panics=[];parity=[];errors=[];oldhash=hashlib.sha256();newhash=hashlib.sha256();total_lines=0
start=time.monotonic()
for first in range(0,len(records),256):
 batch=records[first:first+256]
 data=b''.join(struct.pack('<I',len(r['markdown'].encode()))+r['markdown'].encode() for r in batch)
 outputs=[]
 for label in ['baseline','candidate']:
  out=subprocess.run([prefix+'-'+label],input=data,stdout=subprocess.PIPE,stderr=subprocess.PIPE,check=True,timeout=45)
  assert not out.stderr,out.stderr
  outputs.append(out.stdout)
 oldhash.update(outputs[0]);newhash.update(outputs[1])
 old=outputs[0].decode().splitlines();new=outputs[1].decode().splitlines()
 assert len(old)==len(new)==len(batch)*3
 total_lines+=len(new)
 for before,after in zip(old,new):
  i,mode,direct,checked,arena,events,ast=after.split('\t');i=int(i)+first
  details={'index':i,'mode':int(mode),'case':records[i]}
  if before!=after:changed.append({**details,'before':before,'after':after})
  if 'PANIC' in [direct,checked,arena,events,ast]:panics.append({**details,'output':after})
  if direct!=checked or direct!=arena:parity.append({**details,'output':after})
  if 'Err(' in direct+checked+arena+events+ast:errors.append({**details,'output':after})
 if first%12800==0:print('completed',min(first+256,len(records)),round(time.monotonic()-start,1),'s',flush=True)
summary={'baseline_commit':'ad90098','candidate_commit':'01d053f8c97cc39520ef94046855626d5382fd84','source_file_sha256':hashlib.sha256(Path('src/block_parser/mod.rs').read_bytes()).hexdigest(),'source_count':len(records),'categories':dict(counts),'source_option_combinations':total_lines,'input_sha256':hashlib.sha256(content).hexdigest(),'baseline_output_sha256':oldhash.hexdigest(),'candidate_output_sha256':newhash.hexdigest(),'full_debug_records_identical':oldhash.digest()==newhash.digest() and not changed,'changed_records':len(changed),'panics':len(panics),'rendering_path_disagreements':len(parity),'error_records':len(errors),'elapsed_seconds':round(time.monotonic()-start,2)}
Path(prefix+'.json').write_text(json.dumps(summary,indent=2)+'\n')
for name,values in [('changes',changed),('panics',panics),('parity',parity),('errors',errors)]:Path(prefix+'-'+name+'.json').write_text(json.dumps(values,indent=2)+'\n')
print(json.dumps(summary,indent=2),flush=True)
