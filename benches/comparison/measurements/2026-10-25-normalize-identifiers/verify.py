"""Compare frozen adapters, replay preserved probes, and collect separate allocation diagnostics."""
import argparse
import gzip
import hashlib
import html
import json
import os
from pathlib import Path
import subprocess

parser = argparse.ArgumentParser(__doc__)
parser.add_argument('--work', type=Path, required=True, help='Contains baseline/candidate, their -driver and -allocations binaries')
args = parser.parse_args()
work = args.work.resolve()
study = Path(__file__).resolve().parent
comparison = study.parents[1]
sha = lambda data: hashlib.sha256(data).hexdigest()
env = dict(os.environ, GEM_HOME='/tmp/core-markdown-comparison/gems', GEM_PATH='/tmp/core-markdown-comparison/gems')
report = dict(source_commit=subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(), corpus=[], probes=[])
paths = {'commonmark': comparison / 'data/commonmark.json',
         'gfm-official': comparison / 'data/gfm-official.json',
         'gfm-spec': work / 'gfm-spec.json',
         'custom': comparison / 'fixtures/custom.json'}
paths['gfm-spec'].write_bytes(gzip.decompress((study / 'gfm-spec.json.gz').read_bytes()))
previous = study.parent / '2026-10-25-event-delivery/preservation.json'
for row in json.loads(previous.read_text())['corpus']:
    path = paths[row['corpus']]
    assert sha(path.read_bytes()) == row['input_sha256']
    outputs = [subprocess.check_output([str(work / engine), 'render', row['mode'], row['profile'], str(path)], cwd=comparison)
               for engine in ['baseline', 'candidate']]
    assert json.loads(outputs[0]) == json.loads(outputs[1]), row
    report['corpus'].append(row | dict(output_sha256=sha(outputs[1])))

probes = [('cases', study.parent / '2026-10-25-single-parser/cases.jsonl.gz'),
          ('generated-cases', study.parent / '2026-10-25-single-parser/generated-cases.jsonl.gz'),
          ('label-boundaries', study / 'label-boundaries.jsonl.gz'),
          ('boundary-cases', study / 'boundary-cases.jsonl.gz')]
for name, path in probes:
    data = gzip.decompress(path.read_bytes())
    outputs = [subprocess.check_output([str(work / (engine + '-driver'))], input=data) for engine in ['baseline', 'candidate']]
    assert outputs[0] == outputs[1], name
    rows = [json.loads(line) for line in outputs[1].splitlines()]
    assert all(not row.get('panic') and row['html'] == row['checked'] == row['arena'] for row in rows), name
    report['probes'].append(dict(name=name, count=len(rows), input_sha256=sha(data), output_sha256=sha(outputs[1]), byte_differences=0))
(work / 'preservation.json').write_text(json.dumps(report, indent=2) + '\n')

agreement = []
allocations = []
for entry in json.loads((study / 'inputs.json').read_text()):
    profile, path = entry['profile'], (study / entry['path']).resolve()
    cases = json.loads(path.read_text())
    outputs = {}
    for engine, mode in [('baseline', 'core-markdown'), ('candidate', 'core-markdown'), ('candidate', 'core-stream'), ('candidate', 'core-arena')]:
        outputs[(engine, mode)] = json.loads(subprocess.check_output([str(work / engine), 'render', mode, profile, str(path)]))
    outputs['inkmark'] = json.loads(subprocess.check_output(['bundle', 'exec', 'ruby', 'adapter.rb', 'inkmark', 'render', profile, str(path)], cwd=comparison, env=env))
    counts = {engine: [json.loads(line) for line in subprocess.check_output([str(work / (engine + '-allocations')), profile, str(path)]).splitlines()]
              for engine in ['baseline', 'candidate']}
    for index, case in enumerate(cases):
        actual = outputs[('candidate', 'core-markdown')][index]
        assert all(value[index] == actual for key, value in outputs.items() if key != 'inkmark'), case['name']
        other = outputs['inkmark'][index]
        first = next((i for i, (a, b) in enumerate(zip(actual, other)) if a != b), min(len(actual), len(other)))
        agreement.append(dict(document=case['name'], profile=profile,
            markdown_sha256=sha(case['markdown'].encode()), core_sha256=sha(actual.encode()), inkmark_sha256=sha(other.encode()),
            baseline_candidate_equal=True, core_consumers_equal=True, inkmark_byte_equal=actual == other,
            inkmark_entity_decoded_equal=html.unescape(actual) == html.unescape(other),
            difference=None if actual == other else dict(character_offset=first, core=actual[max(0, first-40):first+100], inkmark=other[max(0, first-40):first+100])))
        before, after = counts['baseline'][index], counts['candidate'][index]
        for key in ['document', 'events', 'nodes', 'node_bytes']:
            assert before[key] == after[key], (case['name'], key)
        deltas = {operation: {key: after[operation][key] - value for key, value in before[operation].items()}
                  for operation in before if isinstance(before[operation], dict)}
        allocations.append(dict(document=case['name'], baseline=before, candidate=after, deltas=deltas))
(work / 'output-agreement.json').write_text(json.dumps(agreement, indent=2) + '\n')
(work / 'allocations.json').write_text(json.dumps(allocations, indent=2) + '\n')
print('Preserved', sum(row['outputs'] for row in report['corpus']), 'corpus outputs and', sum(row['count'] for row in report['probes']), 'probes.')
print('Compared outputs and allocation diagnostics for', len(agreement), 'timed documents.')
