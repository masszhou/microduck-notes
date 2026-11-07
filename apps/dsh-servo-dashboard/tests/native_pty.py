#!/usr/bin/env python3
"""Real JSONL/serial/protocol tests. Opens only a newly-created PTY, never hardware."""
import json
import os
import pathlib
import pty
import queue
import select
import signal
import struct
import subprocess
import threading
import time

ROOT = pathlib.Path(__file__).resolve().parents[1]
BINARY = ROOT / "native/target/debug/servo-worker"


def crc(data):
    value = 0
    for byte in data:
        value ^= byte << 8
        for _ in range(8):
            value = ((value << 1) ^ (0x8005 if value & 0x8000 else 0)) & 0xFFFF
    return value


def stuff(data):
    return data.replace(b"\xff\xff\xfd", b"\xff\xff\xfd\xfd")


def unstuff(data):
    return data.replace(b"\xff\xff\xfd\xfd", b"\xff\xff\xfd")


def memory(ident):
    b = bytearray(147)
    struct.pack_into("<H", b, 0, 1200)
    b[6], b[7], b[11], b[12], b[68] = 42, ident, 3, 255, 2
    struct.pack_into("<I", b, 44, 100)
    struct.pack_into("<I", b, 48, 4095)
    struct.pack_into("<H", b, 100, 400)
    struct.pack_into("<I", b, 108, 20)
    struct.pack_into("<I", b, 112, 30)
    struct.pack_into("<i", b, 116, 2048)
    struct.pack_into("<i", b, 132, 2048)
    struct.pack_into("<H", b, 144, 50)
    b[146] = 25
    if ident in (11, 12):
        b[64] = 1
    return b


class Fixture:
    def __init__(self):
        self.master, self.slave = pty.openpty()
        self.path = os.ttyname(self.slave)
        assert self.path.startswith(("/dev/ttys", "/dev/pts/")), self.path
        # All possible IDs respond: full alias scan stays fast without changing worker timeouts.
        self.mem = {i: memory(i) for i in range(253)}
        self.writes = []
        self.fault = None
        self.follows = True
        self.running = True
        self.errors = []
        self.events = []
        self.responses = queue.Queue()
        self.counter = 0
        self.proc = subprocess.Popen([str(BINARY)], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                     stderr=subprocess.PIPE, text=True, bufsize=1)
        self.serial = threading.Thread(target=self.emulate, daemon=True)
        self.reader = threading.Thread(target=self.read_json, daemon=True)
        self.serial.start()
        self.reader.start()

    def read_json(self):
        try:
            for line in self.proc.stdout:
                obj = json.loads(line)
                if "event" in obj:
                    self.events.append(obj)
                else:
                    self.responses.put(obj)
        except Exception as exc:
            self.errors.append(exc)

    def reply(self, ident, data, error=0):
        body = stuff(bytes([0x55, error]) + data)
        packet = b"\xff\xff\xfd\x00" + bytes([ident]) + struct.pack("<H", len(body) + 2) + body
        packet += struct.pack("<H", crc(packet))
        if self.fault == "crc":
            packet = packet[:-1] + bytes([packet[-1] ^ 1])
            self.fault = None
        elif self.fault == "id":
            packet = packet[:4] + bytes([(ident + 1) % 253]) + packet[5:-2]
            packet += struct.pack("<H", crc(packet))
            self.fault = None
        os.write(self.master, packet)

    def emulate(self):
        pending = bytearray()
        try:
            while self.running:
                if not select.select([self.master], [], [], .1)[0]:
                    continue
                pending.extend(os.read(self.master, 4096))
                while len(pending) >= 7:
                    assert pending[:4] == b"\xff\xff\xfd\x00", pending[:20]
                    n = struct.unpack_from("<H", pending, 5)[0] + 7
                    if len(pending) < n:
                        break
                    packet = bytes(pending[:n])
                    del pending[:n]
                    assert crc(packet[:-2]) == struct.unpack_from("<H", packet, n-2)[0]
                    ident, body = packet[4], unstuff(packet[7:-2])
                    op = body[0]
                    if op == 1:
                        if ident in self.mem:
                            self.reply(ident, self.mem[ident][:2] + self.mem[ident][6:7])
                    elif op == 2:
                        addr, width = struct.unpack_from("<HH", body, 1)
                        if ident in self.mem:
                            if self.fault == "missing" and ident == 1:
                                continue
                            error = 128 if self.fault == "alert" and ident == 1 else 0
                            self.reply(ident, self.mem[ident][addr:addr+width], error)
                    elif op == 0x83:
                        assert ident == 254
                        addr, width = struct.unpack_from("<HH", body, 1)
                        assert len(body) == 6 + width, "must contain exactly one explicit target"
                        target, data = body[5], body[6:]
                        assert target <= 252 and addr in (64, 98, 100, 108, 112, 116)
                        self.writes.append((target, addr, bytes(data)))
                        if self.fault == "restore" and addr == 100 and data == struct.pack("<H", 400):
                            continue
                        self.mem[target][addr:addr+width] = data
                        if addr == 116 and self.follows:
                            self.mem[target][132:136] = data
                    else:
                        raise AssertionError(f"unexpected instruction {op}; broadcast WRITE forbidden")
        except OSError:
            if self.running:
                self.errors.append("PTY closed unexpectedly")
        except Exception as exc:
            self.errors.append(exc)

    def send(self, action, **kwargs):
        self.counter += 1
        request = {"requestId": self.counter, "action": action, **kwargs}
        self.proc.stdin.write(json.dumps(request) + "\n")
        self.proc.stdin.flush()
        return self.counter

    def wait(self, ident, timeout=15):
        end = time.monotonic() + timeout
        skipped = []
        try:
            while time.monotonic() < end:
                obj = self.responses.get(timeout=max(.01, end-time.monotonic()))
                if obj.get("requestId") == ident:
                    return obj
                skipped.append(obj)
        finally:
            for obj in skipped:
                self.responses.put(obj)
        raise AssertionError(f"request {ident} timeout; {self.errors}")

    def call(self, action, **kwargs):
        return self.wait(self.send(action, **kwargs))

    def prepare(self):
        r = self.call("connect", port=self.path, baud=1000000)
        assert r["ok"] and len(r["data"]["servos"]) == 20, r
        assert not self.writes
        r = self.call("scan", maxId=252)
        assert r["ok"] and r["data"]["fullScanValid"], r
        assert len(r["data"]["servos"]) == 253
        assert not self.writes

    def assert_untouched(self, controlled=(1,)):
        assert all(w[0] in controlled for w in self.writes), self.writes
        assert self.mem[11][64] == self.mem[12][64] == 1
        assert not self.errors, self.errors

    def close(self, sig=None):
        if self.proc.poll() is None:
            if sig:
                self.proc.send_signal(sig)
            else:
                self.proc.stdin.close()
            self.proc.wait(timeout=8)
        self.running = False
        self.serial.join(timeout=1)
        self.reader.join(timeout=1)
        os.close(self.master)
        os.close(self.slave)
        assert not self.errors, self.errors


def normal_and_lease():
    f = Fixture()
    f.mem[1][68] = 1  # SRL1: writes have no status ACK; explicit reads still verify.
    try:
        f.prepare()
        for args in [dict(id=4, positionDeg=180, actor="agent"), dict(id=1, positionDeg="180", actor="agent"),
                     dict(id=1, positionDeg=191, actor="human"), dict(id=1.5, positionDeg=180, actor="human")]:
            assert not f.call("move", **args)["ok"]
        assert not f.writes
        result = f.call("move", id=1, positionDeg=181, actor="agent")
        assert result["ok"] and result["data"]["command"]["arrivalVerified"], result.get("data", {}).get("command", result)
        assert f.mem[1][64] == 1
        count = len(f.writes)
        duplicate = {"requestId": result["requestId"], "action": "move", "id": 1, "positionDeg": 181, "actor": "agent"}
        f.proc.stdin.write(json.dumps(duplicate) + "\n")
        f.proc.stdin.flush()
        assert not f.wait(result["requestId"])["ok"]
        assert len(f.writes) == count
        assert not f.call("release", id=1, actor="human")["ok"]
        # Human heartbeats and read-only polling cannot keep agent torque alive.
        until = time.monotonic() + 6
        while time.monotonic() < until:
            f.call("heartbeat", actor="human")
            time.sleep(.25)
        state = f.call("status")["data"]
        assert not state["controlledIds"], state
        assert f.mem[1][64] == 0
        assert f.mem[1][98] == 0 and f.mem[1][100:116] == memory(1)[100:116]
        f.assert_untouched()
    finally:
        f.close()
    print("PASS native PTY: strict inputs, full scan, arrival, actor lease, touched-only restore")


def interrupted(sig):
    f = Fixture()
    try:
        f.prepare()
        f.follows = False
        pending = f.send("move", id=1, positionDeg=184, actor="agent")
        # Wait for target packet, not for the long-running move response.
        end = time.monotonic() + 3
        while not any(a == 116 and struct.unpack("<i", d)[0] > 2050 for _, a, d in f.writes):
            assert time.monotonic() < end, (f.writes, f.errors)
            time.sleep(.01)
        heartbeat = f.call("heartbeat", actor="agent")
        assert heartbeat["ok"] and heartbeat["data"]["controlledIds"] == [1]
        if sig == "stop":
            result = f.call("stop", actor="human")
            assert result["ok"]
            motion = f.wait(pending)
            assert motion["data"]["command"]["unknown"]
        else:
            f.close(sig=sig)
        assert f.mem[1][64] == 0
        assert f.mem[1][100:116] == memory(1)[100:116]
        f.assert_untouched()
    finally:
        if f.running:
            f.close()
    print(f"PASS native PTY: long move heartbeat + {sig or 'EOF'} cleanup")


def failure_cases():
    for fault in ("alert", "crc", "id", "missing", "restore"):
        f = Fixture()
        try:
            f.prepare()
            result = f.call("move", id=1, positionDeg=181, actor="agent")
            assert result["data"]["command"]["arrivalVerified"], result["data"]["command"]
            f.fault = fault
            if fault == "restore":
                result = f.call("release", id=1, actor="agent")
                assert result["data"]["controlledIds"] == [1]
                assert f.mem[1][64] == 0
            else:
                deadline = time.monotonic() + 3
                while f.mem[1][64] != 0:
                    assert time.monotonic() < deadline, fault
                    time.sleep(.02)
            f.fault = None
            deadline = time.monotonic() + 4
            while f.call("status")["data"]["controlledIds"]:
                assert time.monotonic() < deadline, fault
                time.sleep(.1)
            assert f.mem[1][64] == 0
            f.assert_untouched()
        finally:
            f.close()
        print(f"PASS native PTY: {fault} safety cleanup and retry")


def arrival_timeout():
    f = Fixture()
    try:
        f.prepare()
        f.follows = False
        pending = f.send("move", id=1, positionDeg=184, actor="agent")
        until = time.monotonic() + 6.5
        while time.monotonic() < until:
            f.call("heartbeat", actor="agent")
            time.sleep(.5)
        result = f.wait(pending)["data"]["command"]
        assert result["unknown"] and result["readbackVerified"] and not result["arrivalVerified"], result
        assert "6 seconds" in result["error"], result
        assert sum(a == 116 and struct.unpack("<i", d)[0] > 2050 for _, a, d in f.writes) == 1
        assert not f.call("status")["data"]["controlledIds"]
        assert f.mem[1][64] == 0
        f.assert_untouched()
    finally:
        f.close()
    print("PASS native PTY: six-second arrival timeout, heartbeats, no automatic resend")


def continuous_near_stationary_moves():
    f = Fixture()
    struct.pack_into("<i", f.mem[1], 128, 1)
    struct.pack_into("<i", f.mem[1], 132, 2071)
    struct.pack_into("<i", f.mem[1], 116, 2071)
    try:
        f.prepare()
        first = f.call("move", id=1, positionDeg=184, actor="agent")
        assert first["ok"] and first["data"]["command"]["arrivalVerified"], first.get("data", {}).get("command", first)
        assert f.mem[1][64] == 1
        assert f.call("heartbeat", actor="agent")["ok"]
        first_write_count = len(f.writes)
        pending = f.send("move", id=1, positionDeg=2071 * 360 / 4096, actor="agent")
        assert f.call("heartbeat", actor="agent")["ok"]
        second = f.wait(pending)
        assert second["ok"] and second["data"]["command"]["arrivalVerified"], second.get("data", {}).get("command", second)
        assert second["data"]["state"]["controlledIds"] == [1]
        assert all(addr != 64 for _, addr, _ in f.writes[first_write_count:])
        assert f.mem[1][64] == 1
        assert f.call("release", id=1, actor="agent")["ok"]
        assert f.mem[1][64] == 0
        assert f.mem[1][98] == 0 and f.mem[1][100:116] == memory(1)[100:116]
        f.assert_untouched()
    finally:
        f.close()
    print("PASS native PTY: two continuous raw1-feedback moves, actor heartbeat, original RAM restore")


def near_stationary_rejection_boundaries():
    f = Fixture()
    f.mem[1][64] = 1
    try:
        f.prepare()
        for raw, goal in ((2, 2048), (-2, 2048), (1, 2500), (-1, 2500)):
            struct.pack_into("<i", f.mem[1], 128, raw)
            struct.pack_into("<i", f.mem[1], 116, goal)
            result = f.call("move", id=1, positionDeg=181, actor="agent")
            assert not result["ok"] and "near-stationary" in result["error"], result
            assert not f.writes
            assert f.mem[1][64] == 1
        f.assert_untouched()
    finally:
        f.close()
    print("PASS native PTY: raw +/-2 and distant goals refuse takeover with zero writes")


def explicit_human_release_unowned():
    f = Fixture()
    struct.pack_into("<i", f.mem[11], 116, 1900)
    struct.pack_into("<i", f.mem[11], 128, 1)
    try:
        connected = f.call("connect", port=f.path, baud=1000000)
        assert connected["ok"] and not connected["data"]["fullScanValid"]
        assert not f.call("release", id=11, actor="human")["ok"]
        for ident in (1, 11):
            assert not f.call("release", id=ident, actor="agent")["ok"]
        assert not f.writes
        assert f.mem[11][64] == f.mem[12][64] == 1
        scanned = f.call("scan", maxId=252)
        assert scanned["ok"] and scanned["data"]["fullScanValid"]
        f.mem[11][11] = 4
        assert not f.call("release", id=11, actor="human")["ok"]
        assert not f.writes
        f.mem[11][11] = 3
        released = f.call("release", id=11, actor="human")
        assert released["ok"] and not released["data"]["controlledIds"]
        assert f.writes and all(ident == 11 and addr != 116 for ident, addr, _ in f.writes)
        assert f.mem[11][64] == 0 and f.mem[12][64] == 1
        assert f.mem[11][98] == 0 and f.mem[11][100:116] == memory(11)[100:116]
        assert struct.unpack_from("<i", f.mem[11], 116)[0] == 1900
        assert not f.errors, f.errors
    finally:
        f.close()
    print("PASS native PTY: explicit human release unowned11 only; agent/no-scan/bad-mode zero writes")


if __name__ == "__main__":
    explicit_human_release_unowned()
    continuous_near_stationary_moves()
    near_stationary_rejection_boundaries()
    normal_and_lease()
    arrival_timeout()
    interrupted("stop")
    interrupted(signal.SIGTERM)
    interrupted(None)
    failure_cases()
