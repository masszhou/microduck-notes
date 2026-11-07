#!/usr/bin/env python3
"""Smoke-test the production binary without opening any serial port."""
import json
import pathlib
import subprocess

root = pathlib.Path(__file__).resolve().parents[1]
requests = [
    '{"requestId":"status","action":"status"}',
    '{"requestId":"status","action":"status"}',
    '{"requestId":"invalid-move","action":"move","actor":"agent","id":4,"positionDeg":180}',
    '{"requestId":"wrong-fields","action":"status","actor":"agent"}',
    '{"requestId":"nonfinite","action":"move","actor":"agent","id":1,"positionDeg":1e400}',
    '{"requestId":"disconnect","action":"disconnect"}',
]
result = subprocess.run([str(root / "bin/servo-worker")], input="\n".join(requests) + "\n",
                        text=True, capture_output=True, timeout=5)
assert result.returncode == 0, (result.returncode, result.stderr)
assert not result.stderr, result.stderr
lines = [json.loads(line) for line in result.stdout.splitlines()]
responses = [obj for obj in lines if "requestId" in obj]
assert len(responses) == len(requests), responses
assert responses[0]["ok"] and responses[0]["data"]["version"] == "1.0.2"
assert not responses[0]["data"]["connected"]
assert all(not obj["ok"] for obj in responses[1:5]), responses
assert responses[5]["ok"] and not responses[5]["data"]["connected"]
assert all(not obj["data"]["controlledIds"] for obj in lines if obj.get("event") == "state")
print("PASS production JSONL: status, strict fields, invalid agent ID, duplicate ID, nonfinite, EOF")
