#!/usr/bin/env python3
"""Fetch immutable vendor inputs, or validate a previously saved local cache."""
import argparse
import hashlib
from pathlib import Path
import urllib.request

ASSETS = {
    'camera-engine-rkaiq.deb': (
        'https://radxa-repo.github.io/bullseye/pool/main/c/camera-engine-rkaiq/camera_engine_rkaiq_rk3568_arm64-fixed.deb',
        '5c03ca9b857d53a3c6917402efc3e37e130d0e0cfad701b9b2b96a31eaf01081'),
    'rockchip-iqfiles.deb': (
        'https://radxa-repo.github.io/bullseye/pool/main/r/rockchip-iqfiles/rockchip-iqfiles-rk356x_0.1.16_all.deb',
        '9a52cff23f336ba0671588edcaf803f1ffb3f695e466137622316ffceeda8eda'),
}
ROOT = Path(__file__).resolve().parent

def fetch(check_only=False):
    cache = ROOT / 'cache'
    if not check_only:
        cache.mkdir(exist_ok=True)
    for name, (url, checksum) in ASSETS.items():
        path = cache / name
        if not path.exists() and not check_only:
            print('Downloading', name, flush=True)
            with urllib.request.urlopen(url, timeout=60) as response:
                data = response.read()
            if hashlib.sha256(data).hexdigest() != checksum:
                raise SystemExit(f'Checksum mismatch: {name}')
            temp = path.with_suffix('.tmp')
            temp.write_bytes(data)
            temp.replace(path)
        if not path.exists() or hashlib.sha256(path.read_bytes()).hexdigest() != checksum:
            raise SystemExit(f'Missing or corrupt asset: {path}; remove corrupt file and fetch again.')
        print('SHA256 OK:', name)

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    fetch(parser.parse_args().check)
