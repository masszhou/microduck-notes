#!/usr/bin/env python3
"""Mac player supervising one dedicated SSH connection and its remote capture."""
import argparse
import shutil
import signal
import subprocess as S
import sys
import time

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--host", default="devuser@radxa-zero3")
args = parser.parse_args()
if args.host.startswith("-") or any(c.isspace() for c in args.host):
    parser.error("Invalid SSH host")
stopping = False
def stop(*_):
    global stopping
    stopping = True
for sig in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
    signal.signal(sig, stop)
for tool in ('ssh', 'ffplay'):
    if not shutil.which(tool):
        sys.exit(f'Missing {tool} in PATH.')

# BatchMode prevents a host-key/password prompt being obscured by player output.
# A dedicated connection makes transport teardown independent of shared SSH sessions.
remote = S.Popen([
    'ssh', '-T', '-S', 'none', '-o', 'ControlMaster=no',
    '-o', 'BatchMode=yes', '-o', 'StrictHostKeyChecking=yes',
    '-o', 'ConnectTimeout=8', '-o', 'ServerAliveInterval=5', '-o', 'ServerAliveCountMax=2',
    args.host, 'python3 "$HOME/.local/bin/radxa-camera-stream.py"',
], stdin=S.PIPE, stdout=S.PIPE, start_new_session=True)
player = None
status = 0
try:
    player = S.Popen([
        'ffplay', '-hide_banner', '-loglevel', 'warning', '-nostats',
        '-f', 'mjpeg', '-fflags', 'nobuffer', '-flags', 'low_delay', '-framedrop',
        '-window_title', 'Radxa IMX219', '-i', 'pipe:0',
    ], stdin=remote.stdout, start_new_session=True)
    remote.stdout.close()
    print('Camera viewer: close the window, press q, or press Ctrl+C to stop.', file=sys.stderr)
    next_heartbeat = 0.0
    while not stopping:
        if player.poll() is not None:
            status = player.returncode
            break
        if remote.poll() is not None:
            status = remote.returncode
            if status:
                print('Remote capture failed; see the message above.', file=sys.stderr)
            break
        if time.monotonic() >= next_heartbeat:
            try:
                remote.stdin.write(b'ping\n')
                remote.stdin.flush()
            except (BrokenPipeError, OSError):
                status = remote.poll() or 1
                break
            next_heartbeat = time.monotonic() + 1
        time.sleep(.1)
finally:
    # EOF tells the remote supervisor to tear down capture before we close SSH.
    if remote.stdin:
        try:
            remote.stdin.close()
        except (BrokenPipeError, OSError):
            pass
    try:
        remote.wait(timeout=8)
    except S.TimeoutExpired:
        remote.terminate()
        try: remote.wait(timeout=2)
        except S.TimeoutExpired: remote.kill();remote.wait()
    if player and player.poll() is None:
        player.terminate()
        try: player.wait(timeout=3)
        except S.TimeoutExpired: player.kill();player.wait()
    if remote.stdout and not remote.stdout.closed:
        remote.stdout.close()
sys.exit(status if status >= 0 else 1)
