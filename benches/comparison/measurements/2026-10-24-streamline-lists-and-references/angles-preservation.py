"""Compare deterministic valid/malformed angle syntax through all output modes."""
import argparse
import hashlib
import json
from pathlib import Path
import random
import subprocess

parser = argparse.ArgumentParser(__doc__)
parser.add_argument('--work', type=Path, required=True)
args = parser.parse_args()
out = Path(__file__).resolve().parent
rng = random.Random(604)
forms = ['<a{}>', '</a{}>', '<a x={} y=z>', '<a x="{}">', "<a x='{}'>",
         '<!--{}-->', '<?{}?>', '<!{}>', '<![CDATA[{}]]>', '<ab:{}>',
         '<{}@example.test>', '<user@{}>']
alphabet = 'abcABC09-_: /<>?!=\'"[]\t\r\n\0é&;'
cases = []
for index in range(12000):
    fragment = ''.join(rng.choices(alphabet, k=rng.randrange(0, 35)))
    value = forms[index % len(forms)].format(fragment)
    source = rng.choice(['x {} y', '[{}](outer)', '![{}](image)', '> x {} y\n',
                         'x {} [a]\n\n[a]: /a']).format(value)
    cases.append(dict(name=f'angle-{index}', markdown=source))
path = args.work / 'data/angle-differential.json'
path.parent.mkdir(parents=True, exist_ok=True)
path.write_text(json.dumps(cases))
records = []
for mode in ['core-markdown', 'core-stream', 'core-arena', 'core-markdown-output']:
    results = [json.loads(subprocess.check_output([
        str(args.work / label), 'render', mode, 'commonmark', str(path)]))
        for label in ['baseline', 'final']]
    assert len(results[0]) == len(results[1]) == len(cases)
    differences = [i for i, (a, b) in enumerate(zip(*results)) if a != b]
    assert not differences, [(cases[i], results[0][i], results[1][i])
                             for i in differences[:5]]
    records.append(dict(mode=mode, cases=len(cases), output_sha256=hashlib.sha256(
        json.dumps(results[1], ensure_ascii=False).encode()).hexdigest()))
(out / 'angles-preservation.json').write_text(json.dumps(dict(
    seed=604, input_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
    records=records), indent=2) + '\n')
print('Preserved 48000 generated angle outputs.')
