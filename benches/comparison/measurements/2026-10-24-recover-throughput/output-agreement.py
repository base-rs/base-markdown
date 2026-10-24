"""Compare immutable baseline/candidate binaries and pinned Inkmark output."""
from pathlib import Path
import hashlib
import html
import json
import os
import subprocess

out = Path(__file__).resolve().parent
comparison = out.parents[1]
work = Path('/tmp/core-markdown-regain-20261024')
extras = Path('/tmp/core-markdown-simplify-20261024/data')
env = dict(os.environ, GEM_HOME='/tmp/core-markdown-comparison/gems', GEM_PATH='/tmp/core-markdown-comparison/gems')
records = []
for fixture in ['commonmark', 'gfm', 'references', 'scanners', 'definitions', 'entities', 'html']:
    profile = 'gfm' if fixture in ['gfm', 'html'] else 'commonmark'
    path = extras / f'{fixture}.json' if fixture in ['entities', 'html'] else comparison / f'data/bench-{fixture}.json'
    outputs = {}
    for label, binary in [('baseline', '/tmp/core-markdown-unify-20261024/selected'), ('candidate', str(work / 'final'))]:
        outputs[label] = json.loads(subprocess.check_output([binary, 'render', 'core-markdown', profile, str(path)], cwd=comparison, text=True))
    outputs['inkmark'] = json.loads(subprocess.check_output(['bundle', 'exec', 'ruby', 'adapter.rb', 'inkmark', 'render', profile, str(path)], cwd=comparison, env=env, text=True))
    assert outputs['baseline'] == outputs['candidate']
    cases = json.loads(path.read_text())
    assert len(cases) == len(outputs['candidate']) == len(outputs['inkmark'])
    for case, ours, theirs in zip(cases, outputs['candidate'], outputs['inkmark']):
        records.append(dict(document=case['name'], profile=profile,
                            input_sha256=hashlib.sha256(case['markdown'].encode()).hexdigest(),
                            baseline_equal=True, inkmark_byte_equal=ours == theirs,
                            inkmark_equal_after_entity_decoding=html.unescape(ours) == html.unescape(theirs),
                            output_sha256=dict(candidate=hashlib.sha256(ours.encode()).hexdigest(), inkmark=hashlib.sha256(theirs.encode()).hexdigest())))
(out / 'benchmark-output-agreement.json').write_text(json.dumps(records, indent=2) + '\n')
print('Compared', len(records), 'benchmark outputs.')
