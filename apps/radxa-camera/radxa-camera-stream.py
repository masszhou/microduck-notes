#!/usr/bin/env python3
"""Stream JPEG on stdout; stop the owned capture process when SSH stdin closes."""
import fcntl
import os
from pathlib import Path
import selectors
import signal
import subprocess
import sys
import time

stopping = False
def stop(*_):
    global stopping
    stopping = True
for sig in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
    signal.signal(sig, stop)

lock_dir = Path.home() / '.cache/radxa-camera'
lock_dir.mkdir(parents=True, exist_ok=True)
lock = (lock_dir / 'video0.lock').open('a')
try:
    fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
except BlockingIOError:
    sys.exit('Another managed camera stream is running. Close its player first.')
if subprocess.run(['systemctl', 'is-active', '--quiet', 'rkaiq_3A.service']).returncode:
    sys.exit('rkaiq_3A.service is not active; check the service before streaming.')

capture = subprocess.Popen([
    'gst-launch-1.0', '-q', '-e', 'v4l2src', 'device=/dev/video0', 'io-mode=4', '!',
    'video/x-raw,format=NV12,width=1280,height=960', '!',
    'jpegenc', 'quality=75', '!', 'fdsink', 'fd=1',
], stdin=subprocess.DEVNULL, start_new_session=True)
sel = selectors.DefaultSelector()
sel.register(sys.stdin.fileno(), selectors.EVENT_READ)
exit_code = 0
last_input = time.monotonic()
try:
    while not stopping:
        if time.monotonic() - last_input > 15:
            print('Client heartbeat expired; releasing camera.', file=sys.stderr)
            break
        result = capture.poll()
        if result is not None:
            exit_code = result if result >= 0 else 128 - result
            break
        if sel.select(timeout=.2):
            if not os.read(sys.stdin.fileno(),4096):
                break
            last_input = time.monotonic()
finally:
    sel.close()
    # Only signal the process started by this invocation; never kill all captures.
    for sig, wait in ((signal.SIGINT, 3), (signal.SIGTERM, 2), (signal.SIGKILL, 2)):
        if capture.poll() is not None:
            break
        try:
            capture.send_signal(sig)
            capture.wait(timeout=wait)
        except subprocess.TimeoutExpired:
            continue
    if capture.poll() is None:
        print('Camera process did not exit after SIGKILL.', file=sys.stderr)
        exit_code = 1
sys.exit(exit_code)
