#!/usr/bin/env python3
"""Exercise the actual CLI/serial/rustypot path against 15 emulated motors.

No hardware, no Python dependencies. Builds a PTY-only feature on macOS/Linux.
Checks motion/homing, status alert, missing reply, config read-back failure and SIGINT.
"""
import json
import os
from pathlib import Path
import pty
import select
import signal
import struct
import subprocess
import tempfile
import threading
import time

ROOT = Path(__file__).resolve().parents[1]
BIN = ROOT / "target/pty-tests/debug/mac-servo-bench"


def crc(data):
    c = 0
    for b in data:
        c ^= b << 8
        for _ in range(8):
            c = ((c << 1) ^ (0x8005 if c & 0x8000 else 0)) & 0xFFFF
    return c


def packet(servo_id, data, error=0):
    body = bytearray()
    original = bytes([0x55, error]) + data
    for i, b in enumerate(original):
        body.append(b)
        if original[max(0, i - 2):i + 1] == b"\xff\xff\xfd":
            body.append(0xFD)
    p = b"\xff\xff\xfd\0" + bytes([servo_id]) + struct.pack("<H", len(body) + 2) + body
    return p + struct.pack("<H", crc(p))


def motor():
    b = bytearray(147)
    struct.pack_into("<H", b, 0, 1200)
    b[6:14] = bytes([53, 0, 1, 250, 0, 3, 255, 2])
    struct.pack_into("<H", b, 36, 885)
    struct.pack_into("<I", b, 44, 445)
    struct.pack_into("<I", b, 48, 4095)
    b[68] = 2
    struct.pack_into("<H", b, 100, 885)
    struct.pack_into("<I", b, 116, 2048)
    # Includes FF FF FD in the current/velocity block: exercises byte stuffing.
    struct.pack_into("<hi", b, 126, -1, -3)
    struct.pack_into("<I", b, 132, 2048)
    struct.pack_into("<H", b, 144, 50)
    b[146] = 25
    return b


def run_case(case):
    homing = case.startswith("home")
    fault = case.removeprefix("home_") if homing else case
    master, slave = pty.openpty()
    path = os.ttyname(slave)
    motors = {i: motor() for i in range(1, 16)}
    for i, b in motors.items():
        b[7] = i
        if homing:
            struct.pack_into("<i", b, 132, 768 + i * 137)
            struct.pack_into("<i", b, 128, 0)
    eeprom = {i: bytes(b[:64]) for i, b in motors.items()}
    done = threading.Event()
    enabled = threading.Event()
    center_sent = threading.Event()
    failures = []
    injected = False
    velocity, acceleration = (42, 27) if case == "custom_profile" else (30, 20)
    center_packets = []

    def emulate():
        nonlocal injected
        buf = bytearray()
        try:
            while not done.is_set():
                if not select.select([master], [], [], 0.05)[0]:
                    continue
                buf.extend(os.read(master, 4096))
                while len(buf) >= 7:
                    assert buf[:4] == b"\xff\xff\xfd\0", buf.hex()
                    size = 7 + int.from_bytes(buf[5:7], "little")
                    if len(buf) < size:
                        break
                    p = bytes(buf[:size]); del buf[:size]
                    assert crc(p[:-2]) == int.from_bytes(p[-2:], "little")
                    body = p[7:-2].replace(b"\xff\xff\xfd\xfd", b"\xff\xff\xfd")
                    op = body[0]
                    addr, width = struct.unpack_from("<HH", body, 1)
                    if op == 0x83:
                        assert addr in (64, 98, 100, 108, 116), addr
                        for start in range(5, len(body), width + 1):
                            i = body[start]
                            data = body[start + 1:start + 1 + width]
                            if fault == "readback" and addr == 100 and i == 7 and not injected:
                                injected = True
                                continue
                            motors[i][addr:addr + width] = data
                            if addr == 116 and not homing:
                                motors[i][132:136] = data
                        if homing and addr == 116 and all(b[64] == 1 for b in motors.values()):
                            assert all(struct.unpack_from("<i", b, 116)[0] == 2048 for b in motors.values())
                            center_packets.append(p)
                            center_sent.set()
                        if all(b[64] == 1 for b in motors.values()):
                            assert all(b[108:116] == struct.pack("<II", acceleration, velocity) for b in motors.values()), "wrong profile registers at torque enable"
                            enabled.set()
                    elif op in (2, 0x82):
                        ids = [p[4]] if op == 2 else list(body[5:])
                        if homing and addr == 124 and center_sent.is_set():
                            for i, b in motors.items():
                                if fault in ("stuck", "interrupt") and i == 15:
                                    continue
                                present = struct.unpack_from("<i", b, 132)[0]
                                goal = struct.unpack_from("<i", b, 116)[0]
                                remaining = goal - present
                                present += max(-192, min(192, remaining))
                                if fault == "flyby":
                                    present = goal
                                struct.pack_into("<i", b, 132, present)
                                # A fast pass through the target must not count as settled.
                                struct.pack_into("<i", b, 128, 100 if fault == "flyby" else 0)
                        for i in ids:
                            error = 0
                            if addr == 124 and enabled.is_set() and not injected and i == 7:
                                if fault == "alert":
                                    error = 0x80; injected = True
                                elif fault == "missing":
                                    injected = True
                                    continue
                            response = packet(i, bytes(motors[i][addr:addr + width]), error)
                            # Deliberately fragment replies just like USB reads.
                            os.write(master, response[:7])
                            os.write(master, response[7:])
                    else:
                        raise AssertionError(f"unexpected opcode {op}")
        except BaseException as exc:
            failures.append(exc)

    worker = threading.Thread(target=emulate, daemon=True)
    worker.start()
    try:
        with tempfile.TemporaryDirectory(prefix="servo-bench-test-") as tmp:
            logfile = Path(tmp) / "trace.jsonl"
            with (Path(tmp) / "stdout.txt").open("w+") as output:
                command = [str(BIN), "run", "--port", path, "--quiet-bus", "--duration", "10" if case == "interrupt" else "1", "--log", str(logfile)]
                if homing:
                    command = [str(BIN), "home", "--port", path, "--quiet-bus", "--timeout", "3" if case == "home" else "1", "--log", str(logfile)]
                if case == "custom_profile":
                    command += ["--profile-velocity", str(velocity), "--profile-acceleration", str(acceleration)]
                proc = subprocess.Popen(command, stdout=output, stderr=subprocess.STDOUT)
                if fault == "interrupt":
                    assert (center_sent if homing else enabled).wait(8), "did not start motion"
                    time.sleep(0.3)
                    proc.send_signal(signal.SIGINT)
                try:
                    code = proc.wait(timeout=15)
                except subprocess.TimeoutExpired:
                    proc.kill(); proc.wait(); raise
                output.seek(0)
                text = output.read()
                assert code == (0 if case in ("normal", "interrupt", "custom_profile", "home") else 1), text
                events = [json.loads(line) for line in logfile.read_text().splitlines()]
            assert not failures, failures
            assert all(b[64] == 0 for b in motors.values()), "torque still on"
            assert all(bytes(b[:64]) == eeprom[i] for i, b in motors.items()), "EEPROM changed"
            assert all(b[98] == 0 and b[100:102] == struct.pack("<H", 885) and b[108:116] == bytes(8) for b in motors.values()), "RAM settings not restored"
            shutdown = [e for e in events if e["event"] == "shutdown_complete"]
            assert shutdown and shutdown[-1]["success"], text
            cycles = [e for e in events if e["event"] == "cycle_plan"]
            if case in ("normal", "custom_profile"):
                assert cycles
                assert all(len(e["observation"]) == 61 and len(e["action"]) == 14 for e in cycles)
                assert all(e["feedback"]["current_ma"] == [-1.0] * 15 for e in cycles)
                configured = next(e for e in events if e["event"] == "motion_enabled")
                assert configured["profile_velocity_raw"] == velocity and configured["profile_acceleration_raw"] == acceleration
            if fault in ("alert", "missing", "readback"):
                assert injected
            completed = [e for e in events if e["event"] == "home_complete"]
            if homing:
                assert not cycles, "homing must not run the wave loop"
                assert len(center_packets) == (0 if fault == "readback" else 1), "center goal must be sent exactly once"
                assert bool(completed) == (case == "home"), text
                if case == "home":
                    assert all(struct.unpack_from("<i", b, 132)[0] == 2048 for b in motors.values())
                    feedback = [e for e in events if e["event"] == "home_feedback"]
                    assert [e["settled_samples"] for e in feedback[-3:]] == [1, 2, 3]
                if fault in ("stuck", "flyby"):
                    assert "homing timed out" in text
                if fault == "flyby":
                    assert all(struct.unpack_from("<i", b, 132)[0] == 2048 for b in motors.values())
            print(f"PASS {case}: torque off, EEPROM unchanged, RAM restored; cycles={len(cycles)}; homed={bool(completed)}")
    finally:
        done.set(); worker.join(timeout=1)
        os.close(master); os.close(slave)


if __name__ == "__main__":
    subprocess.run(["cargo", "build", "--locked", "--offline", "--features", "pty-tests", "--target-dir", "target/pty-tests"], cwd=ROOT, check=True)
    for case in ["normal", "custom_profile", "alert", "missing", "readback", "interrupt",
                 "home", "home_stuck", "home_flyby", "home_alert", "home_missing", "home_readback", "home_interrupt"]:
        run_case(case)
