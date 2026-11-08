#!/bin/bash
# Read-only checks by default; --capture briefly owns the camera for 60 frames.
set -euo pipefail
cd -- "$(dirname -- "$(readlink -f -- "$0")")"
[[ $# == 0 || ( $# == 1 && $1 == --capture ) ]] || { echo 'Usage: bash check.sh [--capture]' >&2; exit 1; }
printf 'Kernel: '; uname -r
systemctl is-enabled rkaiq_3A.service
systemctl is-active rkaiq_3A.service
media-ctl -d /dev/media0 -p | grep -F 'm00_b_imx219 2-0010'
python3 configure.py iq /etc/iqfiles/imx219_rpi-camera-v2_default.json
pid=$(systemctl show -p MainPID --value rkaiq_3A.service)
[[ $pid =~ ^[1-9][0-9]*$ ]]
stats_ready=false
for ((attempt=0; attempt<20; attempt++)); do
    if grep -l '^xc:isp_3a_stats$' /proc/"$pid"/task/*/comm >/dev/null 2>&1; then
        stats_ready=true
        break
    fi
    sleep 0.25
done
if ! $stats_ready; then
    echo 'Missing ISP statistics thread: check journalctl -u rkaiq_3A.service.' >&2; exit 1
fi
for spec in 'camera-engine-rkaiq-rk3568=5.0x1.2-rc2' 'rockchip-iqfiles-rk356x=0.1.16'; do
    [[ $(dpkg-query -W -f='${Version}' "${spec%%=*}") == "${spec#*=}" ]]
done
if [[ ${1:-} == --capture ]]; then
    if fuser /dev/video0 >/dev/null 2>&1; then
        echo 'Camera busy; close the existing player before capture check.' >&2; exit 1
    fi
    timeout --signal=INT --kill-after=3s 25s gst-launch-1.0 -q -e \
        v4l2src device=/dev/video0 io-mode=4 num-buffers=60 \
        ! video/x-raw,format=NV12,width=1280,height=960 \
        ! jpegenc quality=75 ! fakesink
    echo 'PASS: 60 frames captured and JPEG-encoded at 1280x960.'
fi
echo 'PASS: service, sensor, automatic tuning flags and ISP statistics thread.'
echo 'Inspect live exposure recovery and neutral colors separately; service health alone is not image-quality proof.'
