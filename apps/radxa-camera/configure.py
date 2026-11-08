#!/usr/bin/env python3
"""Pure transforms used by the installer; fail before replacing invalid input."""
import argparse
import json
import os
from pathlib import Path
import tempfile

OVERLAY = 'radxa-zero3-rpi-camera-v2'
IQ_NAME = 'imx219_rpi-camera-v2_default.json'

def boot_config(text):
    lines = text.splitlines()
    positions = [i for i, line in enumerate(lines) if line.startswith('overlays=')]
    if len(positions) > 1:
        raise ValueError('Multiple overlays entries; review /boot/armbianEnv.txt')
    values = lines[positions[0]].split('=', 1)[1].split() if positions else []
    if any(any(s in v.lower() for s in ('camera', 'imx', 'ov5647')) and v != OVERLAY for v in values):
        raise ValueError('Conflicting camera overlay; review boot configuration')
    if OVERLAY in values:
        return text
    values.append(OVERLAY)
    entry = 'overlays=' + ' '.join(values)
    if positions:
        lines[positions[0]] = entry
    else:
        lines.append(entry)
    return '\n'.join(lines) + '\n'

def tuning(raw):
    for old, new in [
        (b'AECV2_STRATEGY_MODE_LOWLIGHT_PRIOR', b'AECV2_STRATEGY_MODE_LOWLIGHT'),
        (b'CALIB_AWB_HDR_FRAME_CHOOSE_MODE_AUTO', b'CALIB_AWB_HDR_FR_CH_AUTO'),
    ]:
        raw = raw.replace(old, new)
    data = json.loads(raw)
    scenes = []
    def visit(obj):
        if isinstance(obj, dict):
            if 'scene_isp21' in obj:
                scene = obj['scene_isp21']
                ae, wb = scene['ae_calib']['CommCtrl'], scene['wb_v21']['control']
                if not (ae['Enable'] == 1 and ae['AecOpType'] == 'RK_AIQ_OP_MODE_AUTO'
                        and wb['byPass'] == 0 and wb['mode'] == 'CALIB_WB_MODE_AUTO'):
                    raise ValueError('Tuning must retain automatic exposure and white balance')
                scenes.append(scene)
            for v in obj.values():
                visit(v)
        elif isinstance(obj, list):
            for v in obj:
                visit(v)
    visit(data)
    if not scenes:
        raise ValueError('Missing ISP21 scene')
    return raw

def atomic_write(path, data):
    fd, name = tempfile.mkstemp(prefix='.camera-', dir=path.parent)
    try:
        with os.fdopen(fd, 'wb') as out:
            out.write(data)
            out.flush()
            os.fsync(out.fileno())
        os.chmod(name, 0o644)
        os.replace(name, path)  # Replace IQ symlink, never modify the package's target.
    finally:
        if os.path.exists(name):
            os.unlink(name)

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode', choices=['boot', 'iq'])
    parser.add_argument('source', type=Path)
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    result = (boot_config(args.source.read_text()).encode() if args.mode == 'boot'
              else tuning(args.source.read_bytes()))
    if args.output:
        atomic_write(args.output, result)
    else:
        print('Configuration validated:', args.mode)
