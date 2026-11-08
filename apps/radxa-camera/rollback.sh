#!/bin/bash
# Configuration rollback only: dependencies/video membership remain installed.
set -euo pipefail
[[ $EUID == 0 ]] || { echo 'Run with sudo.' >&2; exit 1; }
cd -- "$(dirname -- "$(readlink -f -- "$0")")"
[[ -f paths && -d files ]] || { echo 'Run the copy inside the printed backup directory.' >&2; exit 1; }
if fuser /dev/video0 >/dev/null 2>&1; then
    echo 'Close the camera stream before rollback.' >&2; exit 1
fi
systemctl disable --now rkaiq_3A.service 2>/dev/null || true
while IFS= read -r path; do
    case "$path" in
        /boot/armbianEnv.txt|/etc/iqfiles/imx219_rpi-camera-v2_default.json|/etc/systemd/system/rkaiq_3A.service|/usr/local/lib/rkaiq-imx219|*/.local/bin/radxa-camera-stream.py) ;;
        *) echo "Unexpected backup path: $path" >&2; exit 1 ;;
    esac
    rm -rf -- "$path"
    if [[ -e files$path || -L files$path ]]; then
        mkdir -p "$(dirname "$path")"
        cp -a -- "files$path" "$path"
    fi
done
systemctl daemon-reload
if grep -qx enabled enabled; then systemctl enable rkaiq_3A.service; fi
if grep -qx active active; then systemctl start rkaiq_3A.service; fi
echo 'Previous configuration restored. Reboot to restore the saved boot overlay state.'
echo 'APT packages and video group membership retained; packages-before.txt records previous versions.'
