# servo-worker 1.0.2 JSONL contract

## Process and framing

Run `bin/servo-worker`. stdin/stdout use UTF-8 JSON Lines. stdout contains JSON only. stderr contains process-level diagnostics. No port opens until `connect`. The worker never selects a port automatically.

Request envelope: `{ "requestId": string | integer, "action": string, ... }`.
Response envelope: `{ "requestId": sameValue, "ok": true, "data": ... }` or `{ "requestId": sameValue, "ok": false, "error": string }`.
Unsolicited event: `{ "event": "state", "data": STATE }`.
Malformed JSON has `requestId:null`. Unknown fields, wrong JSON types, unsupported actions, nonfinite/out-of-range values are rejected. Maximum input line: 16384 bytes. String requestIds require1..128 bytes. A process refuses repeated valid requestIds; it never resends an uncertain motion. The process shuts down after 100000 accepted request IDs. Host command IDs must map to stable requestIds; `commandId` is not a native field.

`ok:true` means the action was processed. **Only `data.command.arrivalVerified:true` proves an observed arrival.** Accepted moves finish with a command result even on an unknown outcome. Preflight rejection uses `ok:false`. Host must not automatically retry an unknown motion, including after process restart.

## Actions

| action | Additional fields | data |
|---|---|---|
| ports | none | `{ports: PORT[]}` |
| connect | `port:string`, `baud:integer` | STATE after initial read-only IDs1..20 scan |
| scan | optional `maxId:20|252`, default20 | STATE after scan |
| status | none | current cached STATE |
| heartbeat | `actor:"human"|"agent"` | STATE |
| move | `id:integer`, `positionDeg:number`, `actor:"human"|"agent"` | `{command:COMMAND,state:STATE}` after verified arrival or unknown result |
| release | `id:integer`, `actor:"human"|"agent"` | STATE after matching-owner cleanup; explicit human may acquire an unowned discovered motor solely to release it |
| stop | `actor:"human"|"agent"` | STATE after initiating cleanup |
| disconnect | none | disconnected STATE only after verified cleanup; error retains connection and dirty IDs for retry |

PORT = `{path:string,manufacturer:string|null,product:string|null,serialNumber:string|null,vid:integer|null,pid:integer|null}`.
Baud allowlist: 9600, 57600, 115200, 1000000, 2000000, 3000000, 4000000. Baud configures the host serial port only; it never edits servo EEPROM.
Native human IDs are0..252. Native agent IDs are1..3. There is exactly one in-flight move. Completed moves may keep more than one touched motor holding. Full scans refuse while any touched/dirty ID remains. ID252 full scan includes ID0 for alias safety. Heartbeat/status/stop/disconnect remain responsive between bounded scan or motion steps.

For an already owned ID, `release` requires matching ownership. Agent release of any unowned ID always refuses. Explicit human release of an unowned discovered motor requires a current valid full scan, no observed alias/offline/unsupported-device risk, and a fresh snapshot passing the compatible configuration/telemetry safety gates below. It records the original RAM settings and human dirty ownership before writing, then executes the existing torque-off/restore cleanup. It does not require a near-stationary goal and never writes Goal Position. **Warning: this explicit human action unloads the selected motor. Host must obtain the intended user's explicit action and display the unloading warning.** Agent `stop` releases agent-owned IDs only. **By the parent's explicit updated contract, human `stop` releases every plugin-touched ID, including agent-owned IDs.** Neither action writes any unrelated ID. Explicit human release touches only its selected ID before cleanup. A release/stop response may still contain controlled IDs if cleanup remains pending. Wait for `controlledIds` to exclude the IDs and for fresh torque=false telemetry. Cleanup retries are allowed; motion retries are not.

## STATE (all fields always present)

```typescript
interface State {
  connected: boolean;
  port: string | null;
  baud: number | null;
  updatedAt: number;              // Unix epoch milliseconds, state serialization time
  servos: Servo[];               // sorted by ID; offline rows retain last known values
  error: string | null;          // latest global diagnostic; not an authorization signal
  commands: Command[];           // last 64 terminal commands plus in-flight command
  controlledIds: number[];       // ALL dirty/touched IDs, including failed cleanup
  scanning: boolean;
  version: "1.0.2";
  aliasCheckComplete: boolean;   // same validity as fullScanValid
  fullScanAt: number | null;     // epochMs of latest completed full scan, even if invalid
  fullScanValid: boolean;        // complete scan, no unknown alias risk, age <60 seconds
  aliasSafetyError: string | null;
}
```

## SERVO (all fields always present)

```typescript
interface Servo {
  id: number; model: number; firmware: number;
  positionTicks: number; positionDeg: number;
  goalTicks: number; goalDeg: number;
  velocityRpm: number; currentMa: number; pwmPercent: number;
  voltage: number; temperature: number;
  torque: boolean; led: boolean; hardwareError: number;
  operatingMode: number; driveMode: number; homingOffset: number;
  minPositionTicks: number; maxPositionTicks: number;
  profileVelocity: number; profileAcceleration: number; watchdog: number;
  online: boolean; lastSeen: number; error: string | null;
  fresh: boolean;                // online and feedback age <750ms (monotonic clock)
  secondaryId: number; statusReturnLevel: number; goalPwm: number;
  owner: "human" | "agent" | null;
  cleanupPending: boolean;
}
```

Ticks use absolute encoder coordinates, not joint calibration. `degrees=ticks*360/4096`. Signed current uses XL330 units1mA. Velocity uses0.229rpm/raw. Present PWM uses100/885percent/raw. Voltage uses0.1V/raw. Temperature uses Celsius. Profile acceleration/velocity, Goal PWM and watchdog retain raw register units. `lastSeen` is epochMs of the last complete successful feedback read. `online:false` does not zero or discard values. A fresh read is not an assertion that the physical installation is safe.

## COMMAND (all fields always present)

```typescript
interface Command {
  requestId: string | number;
  id: number; actor: "human" | "agent";
  positionDeg: number;           // requested degrees
  targetTicks: number; targetDeg: number; // actual inward-quantized target
  accepted: boolean;
  readbackVerified: boolean;     // target register exact readback succeeded
  arrivalVerified: boolean;     // later fresh feedback within1degree of target
  unknown: boolean;
  status: "accepted" | "readbackVerified" | "arrivalVerified" | "unknown";
  outcome: "pending" | "arrivalVerified" | "unknown";
  error: string | null;
  acceptedAt: number;
  completedAt: number | null;
}
```

Facts only advance. `readbackVerified:true` remains true if subsequent arrival fails. An unknown command never becomes successful merely because later telemetry is near its target. Arrival verification uses a fresh read taken after target readback, within6seconds from acceptance. This is position arrival, not a claim of zero velocity or mechanical settling.

## Native safety authority

- Move validates all fields before any register write. It freshly reads EEPROM and RAM; it never edits EEPROM.
- Agent requires ID1..3 and both fresh present and requested target in170..190degrees. Per the parent's updated bound, its single move also stays within10degrees of fresh present. It cannot acquire a human-owned ID.
- Human may control a supported discovered ID; requested and quantized targets must remain within10degrees of fresh present. Endpoint quantization uses ceil/floor inward bounds.
- Required configuration: model1200, Operating Mode3, Drive Mode bits2/3 clear, Homing Offset0, SecondaryID255, Status Return Level1 or2, EEPROM minimum<maximum<=4095, present inside limits, nonzero EEPROM velocity limit, nonzero valid Goal PWM, no hardware error or status error/Alert, no latched watchdog.
- Required telemetry:4.2..5.8V, temperature<55C, absolute current<350mA, feedback<750ms old.
- No motion until a full0..252 scan validates every detected device as model1200 with SecondaryID255. Any corrupt/partial/Alert/unknown reply, unreadable detected device, previously discovered offline device, scan restart or bus fault invalidates the certificate. Certificate expires60seconds after completion for new moves and human release of unowned IDs. Active holds require heartbeat but do not require rescanning. No scan is permitted while touched.
- The worker saves original Goal PWM/Profile Acceleration/Profile Velocity/Watchdog before its first write and records dirty ownership first.
- A motor initially torqueON must meet the near-stationary hold gate: fresh goal within1degree of present, and absolute raw present velocity<=1 (<=0.229RPM, about0.02398rad/s). This tolerates one velocity quantization unit; it does not prove exact zero velocity or mechanical settling. Raw velocity±2 and distant goals refuse takeover before any write. Existing holding torque stays on continuously. A motor initially torqueOFF stays off until its current hold target is preloaded.
- The worker applies Goal PWM<=200, Profile Acceleration<=10 and Profile Velocity<=5, taking the smaller existing finite limit. Velocity also respects the EEPROM velocity limit. Existing profile0 denotes unlimited, so0 becomes the conservative limit. Watchdog50 means1000ms. It freshly validates/preloads the current hold target, enables torque only if initially off, freshly revalidates and sends the target.
- Every write uses Protocol2 SYNC_WRITE with exactly one explicit target plus exact register readback. SYNC_WRITE necessarily has wire packet ID254; **there is no broadcast WRITE instruction and no multi-ID write payload**. This permits SRL1 and SRL2 without interpreting a missing write ACK as success.
- Allowed write registers only: Torque Enable64, Watchdog98, Goal PWM100, Profile Acceleration108, Profile Velocity112, Goal Position116. CRC, expected response ID, status opcode, exact data length, status error/Alert and byte stuffing are checked before accepting a reply. The stream wrapper rejects hostile lengths before rustypot allocation/parsing.
- Polling is read-only, single-owner and nonoverlapping, targeting3Hz. Controlled feedback gets scheduling priority. Oversubscribed buses can run slower; stale controlled feedback causes touched-only cleanup instead of silently weakening the freshness bound.

## Lease, cleanup, failure

Ownership starts when a valid explicit move is accepted, or immediately before cleanup for a valid explicit human release of an unowned motor. Each owned motor has a5second monotonic lease. Only an on-time heartbeat for that actor renews existing non-cleaning motors. A new move does not renew an existing lease. Polling/status/other-actor heartbeat never renew it. A late heartbeat cannot revive an expired lease.

Success retains torque. Release/stop/disconnect, lease expiry, EOF, SIGINT/SIGTERM, controlled telemetry hazards or polling failure initiate touched-only cleanup. Cleanup first writes and verifies torque0, then clears watchdog, restores original PWM/acceleration/velocity/watchdog with exact readback. It never restores the old goal or old torque state. **A motor initially at torque1 will end at torque0 if explicitly controlled and released. Untouched IDs11/12 remain as found.**

Failed cleanup retains original settings, owner and dirty ID. The connected worker retries cleanup at1second intervals. Disconnect refuses to close until all dirty IDs clear. EOF/signal shutdown makes3 cleanup passes and reports exit2/stderr if dirty IDs remain. Drop makes one last touched-only attempt during normal unwinding. Output backpressure triggers cleanup instead of blocking the bus thread. SIGKILL, kernel/USB failure and loss of external power are outside software torque-off guarantees. A watchdog stops motion but does not guarantee torque0. If cleanup cannot confirm torque0, remove external servo power.

## Limitations and verification boundary

Full scanning establishes observable Protocol2 topology at the selected baud, not mathematical proof of physical uniqueness. Identical duplicate primary IDs, silent/nonresponding devices, devices powered up after scan, external masters and different-baud devices cannot all be excluded by serial software. Use an exclusive fixed powered bus with unique configured primary IDs. No external controller may change EEPROM, goals or topology during the session. Unsupported models block motion globally because their SecondaryID layout cannot be assumed.

No real `/dev/cu.usb*` port was opened during implementation. Unit tests use an in-memory Bus. PTY tests use only paths returned from `pty.openpty()`. The `pty-tests` feature changes baud to0 only for recognized PTY paths because macOS PTYs lack IOSSIOSPEED; production builds must not enable it. Hardware commissioning and deployment belong to the parent.

Build/test commands from project root:

```sh
cargo test --offline --locked --manifest-path native/Cargo.toml
cargo build --offline --locked --manifest-path native/Cargo.toml --features pty-tests
python3 tests/native_pty.py
cargo build --offline --locked --release --manifest-path native/Cargo.toml
```
