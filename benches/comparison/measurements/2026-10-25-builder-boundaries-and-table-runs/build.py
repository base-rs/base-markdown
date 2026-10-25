"""Reuse the paired build machinery with this study's driver and added fixtures."""
import argparse
from pathlib import Path
import subprocess
import sys
sys.dont_write_bytecode = True
from prepare import prepare

root = Path(__file__).resolve().parent
parser = argparse.ArgumentParser(description=__doc__, add_help=False)
parser.add_argument('--work', type=Path, required=True)
args, _ = parser.parse_known_args()
subprocess.run([sys.executable, "-B", str(root.parent / '2026-10-25-deferred-values/build.py'),
    '--driver', str(root / 'driver.rs'), *sys.argv[1:]], check=True)
prepare(args.work.resolve())
