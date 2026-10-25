"""Build established diagnostics, then freeze the adapter with targeted controls."""
import argparse
from pathlib import Path
import shutil
import subprocess
import sys

parser=argparse.ArgumentParser(__doc__)
parser.add_argument('--baseline',type=Path,required=True)
parser.add_argument('--candidate',type=Path,required=True)
parser.add_argument('--work',type=Path,required=True)
a=parser.parse_args()
s=Path(__file__).resolve().parent
previous=s.with_name('2026-10-25-footnotes-indexes-and-escape-context')
w=a.work.resolve()
subprocess.run([sys.executable,str(previous/'build.py'),'--baseline',str(a.baseline.resolve()),
    '--candidate',str(a.candidate.resolve()),'--work',str(w)],check=True)
for engine in ['baseline','candidate']:
    crate=w/(engine+'-driver-source')
    shutil.copyfile(previous/'bench.rs',crate/'src/previous.rs')
    shutil.copyfile(s/'bench.rs',crate/'src/main.rs')
    for label,features in [('bench',[]),('allocations',['--features','allocations'])]:
        subprocess.run(['cargo','build','--release','--locked','--manifest-path',str(crate/'Cargo.toml'),
            '--target-dir',str(w/'driver-target'),*features],check=True)
        destination=w/(engine+'-'+label)
        destination.unlink(missing_ok=True)
        shutil.copy2(w/'driver-target/release/event-contract-probe',destination)
