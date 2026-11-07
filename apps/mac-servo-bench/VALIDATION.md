# 本次验证记录

日期：2026-09-26。Mac 本机；串口 `/dev/cu.usbmodem1101`，设备描述 ROBOTIS / OpenRB-150。

## 新增15路归中程序

按用户确认的定义，新增 `home.sh`（等价于 `run.sh home`）：ID 1–15全部回到2048（180°）。沿用默认Profile Velocity=30、Profile Acceleration=20及现有设备检查、监测阈值、看门狗、退出扭矩关闭与RAM恢复。连续3次反馈满足全部15路位置误差≤12刻度、速度绝对值≤0.03 rad/s才算到位；默认20秒超时。成功到位后关闭扭矩，不持续保持中位。

本次验证：

- `cargo test --locked --offline`：10项单元测试通过，含新增归中判定和参数检查。
- `cargo clippy --locked --offline --all-targets -- -D warnings`：通过。
- `home.sh --help`：从上级目录用绝对路径启动成功，完成默认构建并显示归位参数；脚本通过 `zsh -n` 检查。
- `python3 tests/pty_smoke.py`：13种虚拟串口场景全部通过。保留原有6种场景，新增正常归中、ID 15不动导致超时、位置到中位但速度仍高、状态Alert、缺失回包、配置读回不一致、归位中SIGINT。模拟15路从不同初始位置逐步回中，确认中位目标只发送一次；各异常均未误报归位成功。全部场景确认扭矩关闭、EEPROM不变、RAM设置恢复。

**归中功能尚未进行实机运动验证。** 此次仅运行单元测试、虚拟串口和帮助命令；下方实机记录属于先前的桌面测试。

## 当前默认值：按用户要求提高幅度、速度和加速度

用户明确速度30和加速度20指 Profile 寄存器原始值。当前默认 `mirror`、每个舵机±45°、8秒周期、Profile Velocity=30、Profile Acceleration=20；两项原来写死为20和3，现在可由命令行调整。移除旧的25°/s固定目标速度限制，改为核对所设Profile Velocity。幅度参数范围30–90°，wave模式各组也至少±30°。

验证：8项单元测试、Clippy、默认CLI离线demo通过；6种虚拟串口场景通过。虚拟舵机在扭矩开启时检查15路Profile确实为30/20，额外验证自定义42/27，以及结束后恢复原寄存器值。此次更新未实机运行；下方8秒小幅实机记录为历史参数。

## 后续视觉观察调整

用户反馈螺丝标记看不出运动。复查其 `1790381578747-22549.jsonl`：实际运行的是硬件模式、最大幅度10°的 wave；运行7.35秒后中断，15路编码器变化范围约5.7–18.2°。这支持“小幅运动不明显”的可能性，不能替代现场视觉确认。

新增 `--pattern mirror --amplitude-deg 45 --period 12`：每个舵机均使用±45°目标幅度，奇偶ID反向。7项单元测试及Clippy通过，包含两种pattern的单圈边界和mirror全90°目标跨度检查；CLI离线demo通过。此新增模式尚待现场观察，下面的实机记录仍仅对应原先8秒/最大±10°的wave。

## 软件验证

- `cargo test --offline --locked`：6 项通过，覆盖有符号反馈与单位换算、61/14 数组顺序与嘴部排除、动作与目标映射、单圈边界、阈值与参数检查、协议帧 CRC/分片/去填充。
- `cargo clippy --offline --all-targets -- -D warnings`：通过。
- `python3 tests/pty_smoke.py`：5 种场景通过，正常结束、状态 Alert、缺失回包、配置读回不一致、SIGINT。均确认模拟舵机扭矩关闭、EEPROM 未变、修改的 RAM 设置恢复。使用真实 CLI、serialport、rustypot 路径，虚拟串口替代 USB 硬件。
- `demo --duration 1`：完成，不打开硬件端口，日志明确标注模拟数据。

## 硬件只读探测

- ID 1–15 全部在线；型号数值1200（XL330-M288），固件53。
- 57600 bps，位置模式3，原始 Return Delay Time=250（500 μs）。
- 开始时15个扭矩均为0；电压4.8–4.9 V，温度23–25°C。
- 记录：[hardware-probe-actual.jsonl](logs/hardware-probe-actual.jsonl)。

## 实际运动验证

执行命令：

```sh
./target/debug/mac-servo-bench run --duration 8 --amplitude-deg 10 \
  --log logs/hardware-motion-small.jsonl
```

启用完整 TX/RX 输出，保存终端文本为 [hardware-motion-small.stdout.txt](logs/hardware-motion-small.stdout.txt)。

| 项目 | 结果 |
|---|---|
| 运动时间 | 约8.19秒，包含循环末尾的调度等待；另有探测、配置、退出时间 |
| 控制轮数 | 40 |
| 每轮工作耗时 | 最小101.93 / 平均102.11 / 最大102.34 ms |
| 超过200 ms工作预算 | 0轮 |
| 完整收发包数 | 1184，含探测、配置、运行及退出 |
| CRC错误 / 状态错误 | 0 / 0 |
| 运动期间电压 | 4.8–4.9 V |
| 最高温度 | 25°C |
| 单舵机最大电流绝对值 | 19 mA（轮询反馈值） |
| 15路电流绝对值之和的最大值 | 200 mA（轮询反馈值，不是外部电源峰值测量） |
| 每轮数组 | observation=61、action=14、target=15，全部长度检查通过 |

每个 ID 在采样期间的实际编码器角度变化范围（最大值减最小值，度）：

```text
ID       1      2     3      4      5     6      7      8     9     10     11     12     13     14     15
range 15.82  10.02  7.30  17.40  13.45  8.09  18.19  10.63  7.47  13.45  11.69   8.44  18.54  12.48   8.35
```

这证明全部15路有实际位置响应；不代表已对机械转向、螺丝视觉标记、机器人关节或电源瞬态做过验证。

退出后重新执行只读 `probe`，与运行前记录逐项比较：

- 15个舵机 Torque Enable 都为0。
- EEPROM 0..63 的原始字节全部与运行前一致。
- RAM 98..115 的原始字节全部与运行前一致，包括 watchdog、PWM和profile设置。
- 本次所有写入地址仅为64、98、100、108、116，均为RAM区域。

原始总线/数组记录：[hardware-motion-small.jsonl](logs/hardware-motion-small.jsonl)。
结束后状态：[hardware-probe-after.jsonl](logs/hardware-probe-after.jsonl)。
统计：[validation-summary.json](logs/validation-summary.json)。

原始运动日志的 `run_summary.elapsed_s` 是从会话启动计时（约9.77秒），不是运动阶段时长；运动阶段8.19秒见同次终端记录。当前代码已把运动时长单列为 `motion_elapsed_s`，保留 `elapsed_s` 作为统一会话时间戳。原始硬件记录保持不变。

## 验证范围

实机验证为 **8秒、最大±10°** 的空载波浪。默认30秒/±30°、其他幅度、长期运行、真实拔线/断电/堵转、接入IMU、ONNX策略控制未进行实机验证。错误与中断路径通过虚拟串口验证。程序不改ID、波特率或EEPROM配置。
