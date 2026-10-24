from pathlib import Path
from html.parser import HTMLParser
import hashlib,json,os,subprocess
p=Path('/Users/samuel/Developer/core-rs/core-markdown/benches/comparison');out=p/'measurements/2026-10-24-text-coalescing'
env=dict(os.environ,GEM_HOME='/tmp/core-markdown-comparison/gems',GEM_PATH='/tmp/core-markdown-comparison/gems')
class Tokens(HTMLParser):
 def __init__(self):super().__init__(convert_charrefs=True);self.tokens=[]
 def handle_starttag(self,tag,attrs):self.tokens.append(('start',tag,sorted(attrs)))
 def handle_endtag(self,tag):self.tokens.append(('end',tag))
 def handle_startendtag(self,tag,attrs):self.tokens.append(('empty',tag,sorted(attrs)))
 def handle_data(self,data):self.tokens.append(('data',data))
 def handle_comment(self,data):self.tokens.append(('comment',data))
 def handle_decl(self,data):self.tokens.append(('decl',data))
def tokens(s):p=Tokens();p.feed(s);p.close();return p.tokens
rows=[]
for fixture in ['commonmark','gfm','references','definitions','scanners']:
 profile='gfm' if fixture=='gfm' else 'commonmark';path=p/f'data/bench-{fixture}.json';cases=json.loads(path.read_text());outputs={}
 for engine in ['baseline','candidate','inkmark']:
  if engine=='inkmark':cmd=['bundle','exec','ruby','adapter.rb','inkmark','render',profile,str(path)]
  else:cmd=[str(p/'target/release/markdown-comparison') if engine=='candidate' else '/tmp/core-markdown-coalesce-baseline','render','core-markdown',profile,str(path)]
  outputs[engine]=json.loads(subprocess.check_output(cmd,cwd=p,env=env,text=True))
 for case,old,new,ink in zip(cases,outputs['baseline'],outputs['candidate'],outputs['inkmark']):
  assert old==new,(case['name'],'baseline output changed')
  rows.append({'document':case['name'],'baseline_byte_identical':old==new,'inkmark_byte_identical':ink==new,'inkmark_equal_html_tokens_after_entity_decoding':tokens(ink)==tokens(new),'candidate_sha256':hashlib.sha256(new.encode()).hexdigest(),'inkmark_sha256':hashlib.sha256(ink.encode()).hexdigest(),'candidate_www_link': 'href="http://www.example.com"' in new,'inkmark_www_link':'href="http://www.example.com"' in ink})
(out/'benchmark-output-agreement.json').write_text(json.dumps(rows,indent=2)+'\n')
print(json.dumps(rows,indent=2))
