#!/usr/bin/env python3
"""Mac/Linux entry point: stage the kit, install via sudo, optionally reboot and check."""
import argparse
import re
import shlex
import subprocess as S
import time
from bundle import bundle

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--host', default='devuser@radxa-zero3')
parser.add_argument('--reboot', action='store_true', help='Reboot after installation and verify the new boot')
args = parser.parse_args()
if not re.fullmatch(r'[a-z_][a-z0-9_-]*@[A-Za-z0-9][A-Za-z0-9.:-]*', args.host):
    parser.error('Use user@hostname or user@IPv4; custom port can be set in ~/.ssh/config')
account = args.host.split('@')[0]
ssh = ['ssh', '-S', 'none', '-o', 'ControlMaster=no', '-o', 'BatchMode=yes',
       '-o', 'StrictHostKeyChecking=yes', '-o', 'ConnectTimeout=8',
       '-o', 'ServerAliveInterval=5', '-o', 'ServerAliveCountMax=2']

def read(command):
    return S.check_output(ssh + ['-T', args.host, command], text=True, timeout=20).strip()

# Authenticate before downloads. No automatic key acceptance or host-key removal.
try:
    before = read('cat /proc/sys/kernel/random/boot_id')
except S.SubprocessError:
    parser.exit(1, 'SSH unavailable: verify host key and set up SSH key authentication first (see README).\n')
archive = bundle()
stage = read('mkdir -p "$HOME/.cache" && mktemp -d "$HOME/.cache/radxa-camera-recovery.XXXXXXXX"')
if not stage.startswith('/') or '\n' in stage:
    parser.exit(1, 'Unexpected remote staging path\n')
q = shlex.quote
with archive.open('rb') as stream:
    S.run(ssh + ['-T', args.host, f'tar -xzf - -C {q(stage)}'], stdin=stream, check=True)
kit = stage + '/radxa-camera'
command = f'cd {q(kit)} && sudo bash ./install.sh --user {q(account)}'
if args.reboot:
    command += ' --reboot'
print('Enter the board sudo password when prompted. Staged kit:', kit, flush=True)
S.run(ssh + ['-t', args.host, command], check=True)
if args.reboot:
    print('Waiting up to 180 seconds for a different boot ID…', flush=True)
    deadline = time.monotonic() + 180
    while time.monotonic() < deadline:
        time.sleep(3)
        try:
            if read('cat /proc/sys/kernel/random/boot_id') != before:
                break
        except (S.SubprocessError, OSError):
            pass
    else:
        parser.exit(1, 'Board did not return with a new boot ID. Inspect power/network; kit and backup remain on board.\n')
    # Wait for the camera unit, not merely for the SSH port.
    for attempt in range(10):
        if S.run(ssh + ['-T', args.host, 'systemctl is-active --quiet rkaiq_3A.service']).returncode == 0:
            break
        time.sleep(2)
    S.run(ssh + ['-T', args.host, f'cd {q(kit)} && bash ./check.sh --capture'], check=True)
    print('Restore and capture check passed. Start watch-camera.command to inspect color and AE/AWB.')
else:
    print('Installation finished. If the overlay was newly enabled, reboot before streaming.')
print(f'Viewer: ./watch-camera.command --host {q(args.host)}')
