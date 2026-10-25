"""Pin the reviewed sources, frozen executables, inputs, drivers and validation logs."""
import argparse
import hashlib
import json
import platform
from pathlib import Path
import re
import subprocess

p=argparse.ArgumentParser(__doc__)
p.add_argument('--work',type=Path,required=True)
a=p.parse_args()
s=Path(__file__).resolve().parent
root=s.parents[3]
w=a.work.resolve()
sha=lambda path:hashlib.sha256(path.read_bytes()).hexdigest()
command=lambda *args:subprocess.check_output(args,text=True).strip()
sources={engine:dict(commit=command('git','-C',str(source),'rev-parse','HEAD'),
    src_tree=command('git','-C',str(source),'rev-parse','HEAD:src'))
    for engine,source in [('baseline',w/'baseline-source'),('candidate',root)]}
subprocess.run(['git','-C',str(root),'diff','--exit-code','--','src','tests'],check=True)
binaries={f'{engine}-{kind}':dict(sha256=sha(w/f'{engine}-{kind}'),bytes=(w/f'{engine}-{kind}').stat().st_size)
    for engine in ['baseline','candidate'] for kind in ['bench','allocations','html','driver']}
previous=s.with_name('2026-10-25-footnotes-indexes-and-escape-context')
inputs=[previous/name for name in ['original.json','controls.json','fence-controls.json','features.json','probes.json']]
inputs += [s/'targeted.json',s/'probes.json']
input_records={str(path.relative_to(s.parent)):dict(sha256=sha(path),cases=len(json.loads(path.read_text()))) for path in inputs}
for engine in ['baseline','candidate']:
    crate=w/(engine+'-driver-source')
    for compiled,authored in [('main.rs',s/'bench.rs'),('previous.rs',previous/'bench.rs'),
        ('established.rs',s.with_name('2026-10-25-serializer-and-edits')/'bench.rs')]:
        assert sha(crate/'src'/compiled)==sha(authored)
metadata=dict(sources=sources,source_review=sources['candidate'],
    compiler=command('rustc','-Vv'),validation_compiler=command('rustup','run','stable','rustc','-Vv'),
    host=dict(platform=platform.platform(),machine=command('sysctl','-n','machdep.cpu.brand_string'),
        isolation='Shared host without affinity/isolation; unrelated processes left running. Own builds, tests, diagnostics and review ran outside timing.'),
    binaries=binaries,inputs=input_records,
    driver_files={str(path.relative_to(s.parent)):sha(path) for path in [s/name for name in ['bench.rs','build.py','prepare.py','verify.py','record.py']]+[previous/'bench.rs',previous/'build.py',previous/'measure.py',s.with_name('2026-10-25-serializer-and-edits')/'bench.rs']},
    locks={str(path):sha(path) for path in [root/'Cargo.lock',root/'benches/comparison/Cargo.lock',w/'baseline-driver-source/Cargo.lock',w/'candidate-driver-source/Cargo.lock']})
(s/'metadata.json').write_text(json.dumps(metadata,indent=2)+'\n')
coverage=(w/'coverage-complete.log').read_text()
match=re.search(r'Region coverage passed: (\d+)/(\d+) measured source regions covered \(excluded: (\d+)',coverage)
assert match
checks=[('coverage-complete.log','CARGO_INCREMENTAL=0 rustup run stable cargo bake --locked test:coverage --package core-markdown --all-targets true --all-features true','native runtime'),
 ('test-final.log','cargo bake --locked test','default workspace'),
 ('clippy-final.log','cargo clippy --workspace --all-targets --all-features --locked -- -D warnings','lint'),
 ('linux-check.log','rustup run stable cargo check --lib --all-features --target x86_64-unknown-linux-gnu --locked','compile-only'),
 ('fmt.log','cargo fmt --all -- --check','formatting'),
 ('verify.log','python3 benches/comparison/measurements/2026-10-25-scopes-frontmatter-and-anchors/verify.py --work WORK','preservation')]
validation=dict(source=sources['candidate'],coverage=dict(covered=int(match[1]),measured=int(match[2]),unreachable_exclusions=int(match[3])),
 checks=[dict(command=cmd,scope=scope,passed=True,log_sha256=sha(w/log)) for log,cmd,scope in checks])
(s/'validation.json').write_text(json.dumps(validation,indent=2)+'\n')
for name in ['preservation.json','allocations.json','primary.jsonl','primary-summary.json','targeted.jsonl','targeted-summary.json']:
    (s/name).write_bytes((w/name).read_bytes())
