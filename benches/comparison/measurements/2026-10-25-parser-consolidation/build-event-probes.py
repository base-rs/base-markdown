"""Build frozen event-contract probes against two source checkouts."""
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
for name, source in [('baseline', args.baseline), ('candidate', args.candidate)]:
    crate = work / (name + '-event-driver')
    (crate / 'src').mkdir(parents=True, exist_ok=True)
    shutil.copyfile(study / 'event-probe.rs', crate / 'src/main.rs')
    shutil.copyfile(study / 'event-probe.lock', crate / 'Cargo.lock')
    (crate / 'Cargo.toml').write_text(
        '[package]\nname = "event-contract-probe"\nversion = "0.1.0"\nedition = "2021"\n'
        '[dependencies]\ncore-markdown = { path = ' + json.dumps(str(source.resolve())) +
        ', features = ["serde"] }\nserde_json = "1"\n[workspace]\n')
    subprocess.run(['cargo', 'build', '--release', '--locked', '--manifest-path', str(crate / 'Cargo.toml')], check=True)
    output = work / (name + '-events')
    output.unlink(missing_ok=True)
    shutil.copy2(crate / 'target/release/event-contract-probe', output)
