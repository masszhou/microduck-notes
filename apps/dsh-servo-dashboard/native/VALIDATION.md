# Native validation — 2026-11-07

Version: **1.0.2**. Platform: macOS arm64. This native implementation/test session opened no real serial hardware and moved no real hardware. Parent hardware observations below are reported findings, not independently reproduced here.

## Passed

- `cargo test --offline --locked --manifest-path native/Cargo.toml`: **25 tests**,0failed.
- `cargo clippy --offline --locked --manifest-path native/Cargo.toml --all-targets -- -D warnings`: passed.
- `cargo fmt --manifest-path native/Cargo.toml --check`: passed.
- PTY feature build and `python3 tests/native_pty.py`: **13 scenarios**, passed.
- `sh native/build.sh`: optimized production build without `pty-tests`, copied to `bin/servo-worker`.
- `python3 tests/native_jsonl.py`: production JSONL smoke passed, no serial access.

Production binary: Mach-O64-bit arm64,997KiB.

SHA-256:

```text
239b9b9cad0f56f4d0dc54ad4a79a87be4a01f3dd339885240771b189d45d575
```

## Tested safety properties

Unit tests cover strict types/fields/nonfinite values; fresh configuration and telemetry gates; exhaustive human endpoint quantization across4094 present ticks; agent182degree→170degree counterexample; initial torqueON near-stationary takeover with raw velocity±1 accepted vs raw±2/int32 extremes/distant-goal rejection; two consecutive raw1-feedback moves preserving lease/original RAM; explicit human release of an unowned discovered motor; no-scan/expired-scan/alias/bad-mode/undiscovered release refusal with zero writes; failed unowned-release restoration dirty retry; EEPROM velocity cap; zero/unlimited profile handling; actor ownership; actor-specific lease and late heartbeat; touched-only dirty restore and retry; human stop all touched vs agent-only stop; unknown target no automatic resend; full-scan certificate expiry and SecondaryID refusal; missing IDs; unrelated hardware hazard; agent present escaping170..190; signal shutdown equivalent core cleanup.

The PTY emulator runs the actual JSONL/serial/rustypot path with all253possible IDs. It asserts each SYNC_WRITE contains exactly one explicit target, only allowlisted RAM registers, valid CRC, and no broadcast WRITE. It starts IDs11/12 at torque1 and checks they receive zero writes and retain torque1, except the explicit human release-unowned11 scenario where only11 unloads and12 remains unchanged. SRL1 andSRL2 both work.

PTY scenarios:

1. Initial/full scan zero writes; strict invalid actions; duplicate request refusal; arrival; mismatched-owner release refusal; unrelated heartbeat cannot extend lease; touched-only original RAM restore.
2. Six-second arrival timeout while heartbeat remains active; exact target never auto-resends.
3. Long move accepts heartbeat and human emergency stop.
4. Long move SIGTERM cleanup.
5. Long move EOF cleanup.
6. Status Alert triggers touched-only safe-off and retry.
7. Corrupt CRC triggers touched-only safe-off and retry.
8. Wrong expected ID triggers touched-only safe-off and retry.
9. Missing reply triggers touched-only safe-off and retained dirty state.
10. Failed RAM restoration retains dirty ownership and retries after the injected fault clears.
11. Two consecutive moves with raw velocity1 while keeping the actor heartbeat lease; torque never drops between moves; release restores the original RAM.
12. Raw velocities±2 and distant goals with raw±1 refuse torqueON takeover with zero writes.
13. Explicit human release of unowned ID11 writes only ID11, retains its old goal, restores its RAM and leaves ID12 torqueON. Agent unowned release, missing full scan and incompatible fresh mode all refuse with zero writes.

## Findings fixed during implementation

- Offline dependency resolution initially pulled unavailable libudev; matched the proven bench serialport `default-features=false` convention.
- The first253-device PTY scenario exceeded the750ms controlled freshness bound. The worker safely failed rather than report success. Scheduling now prioritizes controlled feedback at3Hz while serializing background polls.
- Parent identified unwanted torque unloading and agent step-bound mismatch. Initial near-stationary torqueON now stays continuously ON until explicit cleanup. Agent targets now also intersect present±10degrees with170..190degrees. Regression tests pass.

## 1.0.2 repair findings

- Parent reported stable present position with raw velocity1 (0.229RPM), including static torqueOFF devices. The exact-zero takeover gate was too strict. The native gate now permits raw velocity-1..1 while retaining goal-present<=1degree. The term is near-stationary hold, not proof of exact zero motion.
- Parent requested explicit human unload of unowned discovered motors whose old goals differ from present. This action now takes dirty ownership only after valid full-scan/no-alias and fresh compatible safety checks, then runs existing torque-off/RAM restoration. Agent unowned release remains forbidden. No new goal write exists in this path.
- macOS killed the in-place-overwritten executable with SIGKILL even though its SHA-256 matched the runnable release artifact and codesign verified both files. The build script now verifies destination paths and atomically renames a new temporary inode into the production binary path. Production JSONL smoke passes after this fix.

## Not validated here

No physical arrival, load holding, current transient, USB loss, duplicate physical primary IDs, servo firmware behavior or actual OpenRB bridge was tested. PTY verifies software protocol, not mechanical safety. Native full scans at120ms timeout can take about31seconds on a mostly empty bus; Host should allow at least45seconds. Parent owns commissioning and installation. See `CONTRACT.md` for exact schema and shutdown limits.
