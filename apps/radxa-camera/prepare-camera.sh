#!/bin/bash
set -euo pipefail
# Preserve the full-resolution sensor mode used by the existing 1280x960 stream.
for ((attempt=0; attempt<40; attempt++)); do
    if media-ctl -d /dev/media0 -p 2>/dev/null | grep -q 'm00_b_imx219 2-0010'; then
        media-ctl -d /dev/media0 --set-v4l2 '"m00_b_imx219 2-0010":0[fmt:SRGGB10_1X10/3280x2464]'
        exec python3 - <<'PY'
import json
from pathlib import Path
p = Path('/etc/iqfiles/imx219_rpi-camera-v2_default.json')
obj = json.loads(p.read_bytes())
count = 0
def check(x):
    global count
    if isinstance(x, dict):
        if 'scene_isp21' in x:
            s = x['scene_isp21']
            ae, wb = s['ae_calib']['CommCtrl'], s['wb_v21']['control']
            assert ae['Enable'] == 1 and ae['AecOpType'] == 'RK_AIQ_OP_MODE_AUTO'
            assert wb['byPass'] == 0 and wb['mode'] == 'CALIB_WB_MODE_AUTO'
            count += 1
        for y in x.values(): check(y)
    elif isinstance(x, list):
        for y in x: check(y)
check(obj)
assert count, 'No ISP21 scene'
print('IMX219 3280x2464; AE and AWB automatic configuration verified.')
PY
    fi
    sleep 0.25
done
echo 'IMX219 not detected on /dev/media0; check overlay/cable.' >&2
exit 1
