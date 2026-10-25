"""Record source, binary, fixture and validation identities and archive completed results."""
import argparse
import hashlib
import json
import platform
from pathlib import Path
import re
import subprocess

parser = argparse.ArgumentParser(__doc__)
parser.add_argument('--work', type=Path, required=True)
parser.add_argument('--baseline', type=Path, required=True)
a = parser.parse_args()
s = Path(__file__).resolve().parent
root = s.parents[3]
w = a.work.resolve()
previous = s.with_name('2026-10-25-footnotes-indexes-and-escape-context')
sha = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
command = lambda *args: subprocess.check_output(args, text=True).strip()
sources = {engine: dict(commit=command('git','-C',str(path),'rev-parse','HEAD'),
    src_tree=command('git','-C',str(path),'rev-parse','HEAD:src'),
    tests_tree=command('git','-C',str(path),'rev-parse','HEAD:tests'))
    for engine,path in [('baseline',a.baseline.resolve()),('candidate',root)]}
subprocess.run(['git','diff','--exit-code','--','src','tests'], cwd=root, check=True)
inputs = [previous / name for name in ['original.json','controls.json','fence-controls.json','probes.json']]
inputs += [s / name for name in ['delimiter-controls.json','delimiter-probes.json','features.json','sparse-calls.json']]
for engine in ['baseline','candidate']:
    crate = w / (engine + '-driver-source')
    assert sha(crate/'src/established.rs') == sha(s.with_name('2026-10-25-serializer-and-edits')/'bench.rs')
    assert sha(crate/'src/main.rs') == sha(s.with_name('2026-10-25-single-parser')/'probe-driver.rs')
    # The final build step leaves the probe as main.rs; frozen timing and allocation binaries use the unchanged feature adapter.
metadata = dict(sources=sources,compiler=command('rustc','-Vv'),
    validation_compiler=command('rustup','run','stable','rustc','-Vv'),
    host=dict(platform=platform.platform(),machine=command('sysctl','-n','machdep.cpu.brand_string'),
        isolation='Shared host, no affinity/isolation. Own builds, tests, diagnostics and reviews ran outside final timing.'),
    binaries={f'{engine}-{kind}':dict(sha256=sha(w/f'{engine}-{kind}'),bytes=(w/f'{engine}-{kind}').stat().st_size)
        for engine in ['baseline','candidate'] for kind in ['bench','allocations','html','driver']},
    inputs={str(p.relative_to(s.parent)):dict(sha256=sha(p),cases=len(json.loads(p.read_text()))) for p in inputs},
    drivers={str(p.relative_to(s.parent)):sha(p) for p in [s/'prepare.py',s/'verify.py',s/'record.py',
        previous/'build.py',previous/'bench.rs',previous/'measure.py',s.with_name('2026-10-25-serializer-and-edits')/'bench.rs',
        s.with_name('2026-10-25-single-parser')/'probe-driver.rs',s.with_name('2026-10-25-normalize-identifiers')/'summarize.py']},
    locks={str(p):sha(p) for p in [root/'Cargo.lock',root/'benches/comparison/Cargo.lock',
        w/'baseline-driver-source/Cargo.lock',w/'candidate-driver-source/Cargo.lock']})
(s/'metadata.json').write_text(json.dumps(metadata,indent=2)+'\n')
log_prefix = w.parent / (w.name + '-')
logs = {name:Path(str(log_prefix)+name+'.log') for name in ['coverage','tests','clippy','linux','fmt','verify']}
match = re.search(r'Region coverage passed: (\d+)/(\d+) measured source regions covered \(excluded: (\d+)',logs['coverage'].read_text())
assert match
assert 'Cargo workspace tests passed' in logs['tests'].read_text()
checks = [('coverage','CARGO_INCREMENTAL=0 rustup run stable cargo bake --locked test:coverage --package core-markdown --all-features true --all-targets true','native runtime'),
    ('tests','cargo bake --locked test','default workspace'),
    ('clippy','cargo clippy --workspace --all-features --all-targets --locked -- -D warnings','lint'),
    ('linux','rustup run stable cargo check --lib --all-features --target x86_64-unknown-linux-gnu --locked','compile-only'),
    ('fmt','cargo fmt --all -- --check','formatting'),
    ('verify','python3 benches/comparison/measurements/2026-10-25-text-delimiters-and-footnotes/verify.py --work WORK','preservation')]
(s/'validation.json').write_text(json.dumps(dict(source=sources['candidate'],
    coverage=dict(covered=int(match[1]),measured=int(match[2]),unreachable_exclusions=int(match[3])),
    checks=[dict(command=cmd,scope=scope,passed=True,log_sha256=sha(logs[name])) for name,cmd,scope in checks]),indent=2)+'\n')
for name in ['preservation.json','allocations.json','primary.jsonl','primary-summary.json','targeted.jsonl','targeted-summary.json','sparse-calls.jsonl','sparse-calls-summary.json']:
    (s/name).write_bytes((w/name).read_bytes())
for name in ['exploratory-map','exploratory-definition-index']:
    origin = w/name
    destination = s/name
    destination.mkdir(exist_ok=True)
    data = json.loads((origin/'metadata.json').read_text())
    patch = subprocess.check_output(['git','diff','174813e',data['commit'],'--','src','tests'],cwd=root)
    (destination/'source.patch').write_bytes(patch)
    data['base'] = sources['baseline']['commit']
    data['patch_sha256'] = sha(destination/'source.patch')
    (destination/'metadata.json').write_text(json.dumps(data,indent=2)+'\n')
    for path in origin.glob('*.jsonl'):
        (destination/path.name).write_bytes(path.read_bytes())
    for path in origin.glob('*-summary.json'):
        (destination/path.name).write_bytes(path.read_bytes())
