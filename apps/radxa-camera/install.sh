#!/bin/bash
# Run on the target board, not on the Mac. See README.md.
set -euo pipefail
cd -- "$(dirname -- "$(readlink -f -- "$0")")"
[[ $EUID == 0 ]] || { echo 'Run with sudo bash install.sh --user USER [--reboot]' >&2; exit 1; }
account=''
reboot_after=false
while (($#)); do
    case "$1" in
        --user) account=${2:?Missing account}; shift 2 ;;
        --reboot) reboot_after=true; shift ;;
        *) echo "Unknown argument: $1" >&2; exit 1 ;;
    esac
done
[[ $account =~ ^[a-z_][a-z0-9_-]*$ && $account != root ]] || { echo 'Specify a regular local account using --user.' >&2; exit 1; }
user_home=$(getent passwd "$account" | cut -d: -f6)
[[ -d $user_home ]] || { echo 'User home is missing.' >&2; exit 1; }
. /etc/os-release
[[ ${ID:-} == debian && ${VERSION_ID:-} == 13 && $(uname -m) == aarch64 && $(uname -r) == 6.1.115-vendor-rk35xx ]] || {
    echo 'Supported: Debian 13, ARM64, kernel 6.1.115-vendor-rk35xx. Revalidate other images before installing.' >&2; exit 1;
}
tr '\0' '\n' < /proc/device-tree/compatible | grep -q '^radxa,zero3'
grep -qx 'fdtfile=rockchip/rk3566-radxa-zero3.dtb' /boot/armbianEnv.txt
[[ -f /boot/dtb/rockchip/overlay/radxa-zero3-rpi-camera-v2.dtbo ]]
[[ $(systemctl is-enabled rkaiq_3A.service 2>/dev/null || true) != masked ]] || { echo 'Service is masked; review first.' >&2; exit 1; }
if pgrep -x gst-launch-1.0 >/dev/null || pgrep -x mediad >/dev/null; then
    echo 'Close all camera streams before restoring. No processes have been killed.' >&2; exit 1
fi
# Do not silently downgrade/replace other engine versions.
for spec in 'camera-engine-rkaiq-rk3568=5.0x1.2-rc2' 'rockchip-iqfiles-rk356x=0.1.16'; do
    package=${spec%%=*}; version=${spec#*=}
    if [[ $(dpkg-query -W -f='${Status}' "$package" 2>/dev/null || true) == 'install ok installed' ]]; then
        [[ $(dpkg-query -W -f='${Version}' "$package") == "$version" ]] || {
            echo "Unexpected installed version of $package; review before replacing." >&2; exit 1;
        }
    fi
done
# python3-minimal is normally present on Armbian; explicit preflight avoids partial boot edits.
command -v python3 >/dev/null || { echo 'Install python3 first: sudo apt-get install python3' >&2; exit 1; }
python3 assets.py --check
python3 configure.py boot /boot/armbianEnv.txt
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
dpkg-deb -x cache/rockchip-iqfiles.deb "$work/iq"
python3 configure.py iq "$work/iq/usr/share/rockchip-iqfiles-rk356x/imx219_rpi-camera-v2_default.json" --output "$work/tuning.json"
backup="/var/backups/radxa-camera/$(date +%Y%m%d-%H%M%S)-$$"
mkdir -p "$backup/files"
paths=(/boot/armbianEnv.txt /etc/iqfiles/imx219_rpi-camera-v2_default.json
       /etc/systemd/system/rkaiq_3A.service /usr/local/lib/rkaiq-imx219
       "$user_home/.local/bin/radxa-camera-stream.py")
printf '%s\n' "${paths[@]}" > "$backup/paths"
for path in "${paths[@]}"; do
    if [[ -e $path || -L $path ]]; then
        mkdir -p "$backup/files$(dirname "$path")"
        cp -a -- "$path" "$backup/files$path"
    fi
done
systemctl is-active rkaiq_3A.service > "$backup/active" || true
systemctl is-enabled rkaiq_3A.service > "$backup/enabled" || true
dpkg-query -W > "$backup/packages-before.txt"
cp rollback.sh "$backup/rollback.sh"
printf 'Backup: %s\nRollback: sudo bash %s/rollback.sh\n' "$backup" "$backup"
trap 'echo "Restore failed; inspect output. Configuration backup: $backup" >&2' ERR
apt-get update
apt-get install -y --no-install-recommends gcc libc6-dev libstdc++6 libdrm2 python3 device-tree-compiler \
    v4l-utils psmisc gstreamer1.0-tools gstreamer1.0-plugins-base gstreamer1.0-plugins-good
fdtoverlay -i /boot/dtb/rockchip/rk3566-radxa-zero3.dtb -o "$work/merged.dtb" \
    /boot/dtb/rockchip/overlay/radxa-zero3-rpi-camera-v2.dtbo
gcc -shared -fPIC -O2 -Wall -Wextra -Werror -o "$work/rkaiq_modinfo_shim.so" rkaiq-modinfo-shim.c -ldl -pthread
# Do not replace ISP settings while another process owns its output node.
if fuser /dev/video0 >/dev/null 2>&1; then
    echo 'A process owns /dev/video0; close it and retry.' >&2; exit 1
fi
systemctl stop rkaiq_3A.service 2>/dev/null || true
apt-get install -y --no-install-recommends ./cache/camera-engine-rkaiq.deb ./cache/rockchip-iqfiles.deb
systemctl stop rkaiq_3A.service 2>/dev/null || true
ldconfig
if ldd /usr/bin/rkaiq_3A_server | grep -F 'not found'; then
    echo 'Vendor engine has unresolved runtime libraries; inspect ldd output.' >&2; exit 1
fi
install -d /usr/local/lib/rkaiq-imx219 /etc/iqfiles
install -m 755 "$work/rkaiq_modinfo_shim.so" /usr/local/lib/rkaiq-imx219/
install -m 644 rkaiq-modinfo-shim.c /usr/local/lib/rkaiq-imx219/
install -m 755 prepare-camera.sh /usr/local/lib/rkaiq-imx219/
python3 configure.py iq "$work/tuning.json" --output /etc/iqfiles/imx219_rpi-camera-v2_default.json
install -m 644 rkaiq_3A.service /etc/systemd/system/
# Keep the parent home directories' existing ownership and permissions.
runuser -u "$account" -- mkdir -p "$user_home/.local/bin"
install -o "$account" -g "$(id -gn "$account")" -m 755 radxa-camera-stream.py "$user_home/.local/bin/"
usermod -a -G video "$account"
python3 configure.py boot /boot/armbianEnv.txt --output /boot/armbianEnv.txt
printf '%s\n' "$backup" > /usr/local/lib/rkaiq-imx219/installed-backup
systemctl daemon-reload
systemctl disable rkaiq_3A.service 2>/dev/null || true
systemctl enable rkaiq_3A.service
if media-ctl -d /dev/media0 -p 2>/dev/null | grep -q 'm00_b_imx219 2-0010'; then
    systemctl restart rkaiq_3A.service
    sleep 2
    systemctl is-active --quiet rkaiq_3A.service
    echo 'Installed; service active. Reconnect SSH before streaming for video group membership.'
else
    echo 'Installed; reboot required to load overlay. If still absent after reboot, inspect cable and kernel log.'
fi
if $reboot_after; then
    echo 'Reboot scheduled in 5 seconds.'
    systemd-run --unit="radxa-camera-reboot-$$" --on-active=5s /usr/sbin/reboot
fi
