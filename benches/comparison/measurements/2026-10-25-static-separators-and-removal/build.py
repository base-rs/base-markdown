"""Build and freeze timing/diagnostic executables against two source checkouts."""
import argparse
import json
from pathlib import Path
import shutil
import subprocess

parser = argparse.ArgumentParser(__doc__)
parser.add_argument('--baseline', type=Path, required=True)
parser.add_argument('--candidate', type=Path, required=True)
parser.add_argument('--work', type=Path, required=True)
args = parser.parse_args()
study = Path(__file__).resolve().parent
work = args.work.resolve()
work.mkdir(parents=True, exist_ok=True)
lock = study.with_name('2026-10-25-parser-consolidation') / 'event-probe.lock'
probe = study.with_name('2026-10-25-single-parser') / 'probe-driver.rs'

def freeze(source, destination):
    destination.unlink(missing_ok=True)
    shutil.copy2(source, destination)

for name, source in [('baseline', args.baseline.resolve()), ('candidate', args.candidate.resolve())]:
    crate = work / (name + '-driver-source')
    (crate / 'src').mkdir(parents=True, exist_ok=True)
    (crate / 'Cargo.toml').write_text(
        '[package]\nname="event-contract-probe"\nversion="0.1.0"\nedition="2021"\n'
        '[features]\nallocations=[]\n[dependencies]\ncore-markdown={path=' + json.dumps(str(source)) +
        ',features=["serde"]}\nserde_json="1"\n[workspace]\n')
    shutil.copyfile(lock, crate / 'Cargo.lock')
    shutil.copyfile(study.with_name('2026-10-25-serializer-and-edits') / 'bench.rs', crate / 'src/established.rs')
    for label, driver, features in [('bench', study / 'bench.rs', []),
                                   ('allocations', study / 'bench.rs', ['--features', 'allocations']),
                                   ('driver', probe, [])]:
        shutil.copyfile(driver, crate / 'src/main.rs')
        subprocess.run(['cargo', 'build', '--release', '--locked', '--manifest-path', str(crate / 'Cargo.toml'),
                        '--target-dir', str(work / 'driver-target'), *features], check=True)
        freeze(work / 'driver-target/release/event-contract-probe', work / (name + '-' + label))
    subprocess.run(['cargo', 'build', '--release', '--locked', '--manifest-path',
                    str(source / 'benches/comparison/Cargo.toml'), '--target-dir', str(work / 'comparison-target')], check=True)
    freeze(work / 'comparison-target/release/markdown-comparison', work / (name + '-html'))
