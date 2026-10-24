"""Compare final benchmark output; entity decoding alone is not a conformance proof."""
from pathlib import Path
import hashlib
import html
import json
import os
import subprocess

out = Path(__file__).resolve().parent
comparison = out.parents[1]
work = Path('/tmp/core-markdown-streamline-20261024')
env = dict(os.environ, GEM_HOME='/tmp/core-markdown-comparison/gems', GEM_PATH='/tmp/core-markdown-comparison/gems')
records = []
for fixture in ['commonmark', 'gfm', 'references', 'scanners', 'definitions']:
    profile = 'gfm' if fixture == 'gfm' else 'commonmark'
    path = comparison / f'data/bench-{fixture}.json'
    cases = json.loads(path.read_text())
    outputs = {}
    for label in ['baseline', 'final']:
        outputs[label] = json.loads(subprocess.check_output([str(work / label), 'render', 'core-markdown', profile, str(path)], cwd=comparison, text=True))
    outputs['inkmark'] = json.loads(subprocess.check_output(['bundle', 'exec', 'ruby', 'adapter.rb', 'inkmark', 'render', profile, str(path)], cwd=comparison, env=env, text=True))
    assert outputs['baseline'] == outputs['final']
    for case, ours, theirs in zip(cases, outputs['final'], outputs['inkmark']):
        records.append(dict(document=case['name'], profile=profile,
                            input_sha256=hashlib.sha256(case['markdown'].encode()).hexdigest(),
                            baseline_equal=True, inkmark_byte_equal=ours == theirs,
                            inkmark_equal_after_entity_decoding=html.unescape(ours) == html.unescape(theirs),
                            output_sha256=dict(candidate=hashlib.sha256(ours.encode()).hexdigest(), inkmark=hashlib.sha256(theirs.encode()).hexdigest())))
(out / 'benchmark-output-agreement.json').write_text(json.dumps(records, indent=2) + '\n')
print(json.dumps(records, indent=2))
