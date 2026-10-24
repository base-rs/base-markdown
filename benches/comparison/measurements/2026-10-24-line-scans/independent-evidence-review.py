from pathlib import Path
import json,statistics,hashlib,math,collections
p=Path('benches/comparison/measurements/2026-10-24-line-scans')
manifest=json.loads((p/'run-manifest.json').read_text()); metadata=json.loads((p/'metadata.json').read_text()); groups=collections.defaultdict(list)
def near(a,b):assert math.isclose(a,b,rel_tol=1e-13,abs_tol=1e-14),(a,b)
for m in manifest:
 f=p/m['file'];assert f.exists()
 assert hashlib.sha256(Path(m['command'][-1]).read_bytes()).hexdigest()==m['input_sha256'],m
 if m['mode']=='allocations':continue
 if m['engine']!='inkmark':assert m['binary_sha256']==metadata['binary_sha256'][m['engine']]
 for r in map(json.loads,f.read_text().splitlines()):
  near(statistics.median(r['samples_seconds']),r['median_seconds'])
  groups[(m['mode'],r['document'],m['engine'])].append(r['median_seconds']*1000)
count=0
for doc,values in json.loads((p/'summary.json').read_text()).items():
 for engine in ['baseline','candidate','inkmark']:
  actual=groups[('core-markdown',doc,engine)]; recorded=values[engine]
  assert len(actual)==len(recorded['process_medians_ms'])
  for a,b in zip(actual,recorded['process_medians_ms']):near(a,b)
  near(statistics.median(actual),recorded['median_ms']);count+=1
 near(values['baseline']['median_ms']/values['candidate']['median_ms'],values['baseline_over_candidate'])
 near(values['inkmark']['median_ms']/values['candidate']['median_ms'],values['inkmark_over_candidate'])
for mode,docs in json.loads((p/'phase-summary.json').read_text()).items():
 for doc,values in docs.items():
  for engine,recorded in values.items():
   actual=groups[(mode,doc,engine)]
   assert len(actual)==len(recorded['process_medians_ms'])
   for a,b in zip(actual,recorded['process_medians_ms']):near(a,b)
   near(statistics.median(actual),recorded['median_ms']);count+=1
for profile in ['commonmark','gfm','references']:
 before=list(map(json.loads,(p/f'allocations-baseline-{profile}.jsonl').read_text().splitlines()))
 after=list(map(json.loads,(p/f'allocations-candidate-{profile}.jsonl').read_text().splitlines()))
 assert before==after,profile
summary={'manifest_entries_checked':len(manifest),'timing_summary_groups_recomputed':count,'allocation_records_identical':True,'input_hashes_verified':True,'source_sha256_verified':hashlib.sha256(Path('src/block_parser/mod.rs').read_bytes()).hexdigest()==metadata['block_parser_sha256']}
Path('/tmp/core-markdown-line-scans-review-evidence.json').write_text(json.dumps(summary,indent=2)+'\n');print(json.dumps(summary,indent=2))
