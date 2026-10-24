import json,subprocess,time
from pathlib import Path
root=Path('/Users/samuel/Developer/core-rs/core-markdown')
work=Path('/tmp/core-markdown-regain-20261024')
fixtures=json.loads((root/'benches/comparison/data/bench-commonmark.json').read_text())
path=work/'mixed-1m.json'
path.write_text(json.dumps([r for r in fixtures if r['name']=='mixed-1m']))
for label,binary in [('before-mdx','/tmp/core-markdown-simplify-20261024/final'),('current','/tmp/core-markdown-unify-20261024/selected')]:
    with (work/f'{label}-profile.log').open('w') as log:
        process=subprocess.Popen([binary,'profile','core-markdown','commonmark',str(path)],cwd=root/'benches/comparison',stdout=log,stderr=log)
        try:
            time.sleep(1)
            subprocess.run(['sample',str(process.pid),'5','1','-file',str(work/f'{label}-profile.txt')],check=True,stdout=log,stderr=log)
        finally:
            process.terminate()
            process.wait()
