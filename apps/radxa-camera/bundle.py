#!/usr/bin/env python3
"""Create a portable recovery archive with the two SHA256-pinned vendor packages."""
import hashlib
from pathlib import Path
import tarfile
from assets import ROOT, ASSETS, fetch

VERSION = '1.0.0'

def bundle():
    fetch()
    dest = ROOT / 'dist'
    dest.mkdir(exist_ok=True)
    out = dest / f'radxa-camera-{VERSION}.tar.gz'
    with tarfile.open(out, 'w:gz') as archive:
        for file in sorted(ROOT.rglob('*')):
            relative = file.relative_to(ROOT)
            if not file.is_file() or any(p in ('dist', 'cache', '__pycache__', '.git') for p in relative.parts):
                continue
            archive.add(file, arcname=str(Path('radxa-camera') / relative), recursive=False)
        for name in ASSETS:
            archive.add(ROOT / 'cache' / name, arcname=f'radxa-camera/cache/{name}')
    digest = hashlib.sha256(out.read_bytes()).hexdigest()
    out.with_name(out.name + '.sha256').write_text(f'{digest}  {out.name}\n')
    return out

if __name__ == '__main__':
    print(bundle())
