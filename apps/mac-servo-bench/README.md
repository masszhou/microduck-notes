# Mac / OpenRB-150 / 15 × XL330 桌面测试

针对本次硬件：XL330-M288-T ×15、ID **1–15**、出厂 **57600 bps**、OpenRB-150 出厂 USB 桥接固件、VIN 外部 **5 V / 3 A** 供电、无 IMU、空载。

独立 Rust 项目，使用与 `microduck/duck-control` 相同的 `rustypot 1.6.0` 协议库。没有 GUI、没有 ONNX 推理，也不需要启动 Armbian 服务。

## 直接开始

先让 DYNAMIXEL Wizard **Disconnect**，释放串口。保持外部电源和 USB 连接。

```sh
# From the microduck-notes repository root
cd apps/mac-servo-bench

# 已连接的串口；不会打开端口
./run.sh ports

# 只读检查 15 个舵机，不使能扭矩
./run.sh probe

# 真实运动：30秒、每个舵机±45°、奇偶ID反向；速度30、加速度20（原始值）
./run.sh run

# 等价的显式参数，8秒一个周期
./run.sh run --pattern mirror --amplitude-deg 45 --period 8 --duration 30 \
  --profile-velocity 30 --profile-acceleration 20
```

默认自动选择唯一的 ROBOTIS/OpenRB 串口。当前验证到的端口为 `/dev/cu.usbmodem1101`；重插后可能变化。也可以指定：

```sh
./run.sh run --port /dev/cu.usbmodem1101
```

**`run` 会实际运动。** 按 `Ctrl+C` 提前结束。正常结束、SIGINT/SIGTERM、通信错误或监测阈值触发时，程序尝试关闭全部 15 个舵机的扭矩，并逐个读回确认。

其他常用命令：

```sh
# 不连接硬件：看模拟反馈与相同布局的数组
./run.sh demo --duration 5

# 更大幅度：每个舵机±60°，10秒一个周期
./run.sh run --amplitude-deg 60 --period 10

# 放慢输出，逐笔看数据
./run.sh run --hz 2 --duration 30

# 屏幕只显示数组/状态；全部原始总线数据仍写入日志
./run.sh run --quiet-bus

# 显式关闭 ID 1–15 的扭矩；即便某个舵机无响应也尝试其他舵机
./run.sh off

./run.sh run --help
```

首次运行会编译，后续使用构建缓存。已安装的 Rust/Cargo 即可；`Cargo.lock` 固定依赖版本。代码仅位于此文件夹。

## 全部回到中位（2048 / 180°）

先退出正在运行的测试，并让 DYNAMIXEL Wizard **Disconnect**。在本文件夹运行：

```sh
./home.sh

# 等价命令
./run.sh home

# 指定串口，隐藏屏幕上的原始数据包
./home.sh --port /dev/cu.usbmodem1101 --quiet-bus

# 可调整速度、加速度和到位超时，均沿用现有测试的单位
./home.sh --profile-velocity 30 --profile-acceleration 20 --timeout 20

./home.sh --help
```

**会实际运动：ID 1–15（包括 ID 10）全部回到编码器中位 2048，即180°。** 该命令用于当前桌面测试，不定义装配后的机器人站姿，也不会重设 Homing Offset 或其他 EEPROM 配置。

归位复用现有设备检查及保护：先预装当前位置并使能扭矩，再一次性下发15路中位目标；默认 Profile Velocity=30、Profile Acceleration=20。以5 Hz读取位置、速度、电流、电压和温度。全部15路连续3次满足位置误差≤12刻度（约1.05°）、速度绝对值≤0.03 rad/s，才报告 `HOME reached`。超时默认20秒，从中位目标下发后计时；超时、通信错误、监测阈值触发或中断均报失败。

**到位后关闭全部扭矩并恢复原 RAM 设置，不持续锁住中位。** 到位信息对应关闭扭矩之前的反馈；卸力后轴可能因外力移动。按 `Ctrl+C` 可中止并尝试关闭扭矩；日志仍写入 `logs/`，增加 `home_plan`、`home_feedback`、`home_complete` 事件。`home_complete` 表示已读回到位，随后 `shutdown_complete` 单独记录扭矩关闭与设置恢复的结果。

## 舵机怎样一起动

所有舵机使用位置模式。启动先读取当前位置，预装保持目标，再依次使能扭矩；以 4 秒的平滑过渡开始波浪。

默认使用 `mirror`：每个舵机幅度±45°、周期8秒、下发频率5 Hz，奇偶ID反向。默认 Profile Velocity=30、Profile Acceleration=20，均为 Wizard 控制表中的原始值；启动时逐个读回确认。

可选 `--pattern wave`：相邻ID相位差24°，幅度分组衰减但最低不小于±30°。在幅度参数45°时，各组为±45°、±33.75°、±30°。幅度参数现在允许30–90°，低于30°会报错。

默认镜像模式的两端相差90°，前4秒渐入，建议至少观察20秒。此前10°小幅命令已不再接受。目标正弦的峰值速度由幅度/周期决定；新的校验依据所设 Profile Velocity，不再固定限制为25°/s。默认45°/8秒的目标峰值约35.3°/s，Profile Velocity=30对应约41.22°/s的轮廓上限。往返换向时速度自然经过0；写入轮廓上限不代表全程保持该速度。

每个舵机以自己的起始位置附近为中心；若太接近单圈边界，中心会向内移动，保证整个过程在 `0..4095` 内，不跨越 0/360°。螺丝标记的初始朝向不必一致。

一次 `SYNC_WRITE` 包含 15 个不同的目标，而非逐个舵机阻塞运动。`SYNC_READ` 后 15 个舵机依次回包，测量并非严格同一时刻采样。

在 57600 bps 下，一轮 23 字节反馈 ×15 加上目标包约需 0.11 秒的理论线速时间，还需留出应答/USB/终端时间；因此默认 5 Hz。程序会打印每轮耗时及超期次数，不通过追赶突发掩盖超期。

## 终端和日志里能看到什么

每个真实收发包打印：

- `TX` / `RX`、相对时间、ID、指令、地址/长度、CRC 检查、状态错误码，以及完整十六进制。
- 每轮 15 路实际位置、速度、电流；日志另存电压、温度、PWM。
- 完整的 `observation[61]`、分块解释、`action[14]`、15 路最终目标角度。
- 每轮工作时间、是否超过周期预算、最终扭矩关闭是否逐个确认。

这是 **Mac 串口边界实际写入/读出的字节**，不是 TTL 引脚上的逻辑分析仪采样；无法直接测到电气波形或总线冲突。TX 表示已提交给串口。`SYNC_WRITE` 按协议没有 ACK，因此日志不会把发送成功当作舵机执行成功；后续的实测位置用于观察响应。

每次会新建 `logs/<毫秒时间戳>-<进程号>.jsonl`。可以用 `--log 自己的路径.jsonl`，已存在的文件会被拒绝覆盖。

主要事件：

| event | 内容 |
|---|---|
| `session` | 数组顺序、数据来源、schema 版本 |
| `wire_chunk` | 每次串口 read/write 实际处理的原始字节；包括不完整或无效帧 |
| `packet` | 从字节流重组的完整包、CRC、指令/状态信息 |
| `device` | 型号、固件、原始 EEPROM 配置和待恢复的 RAM 设置 |
| `motion_parameters` | 所选pattern、幅度、周期、持续时间与渐入时间 |
| `cycle_plan` | 写目标之前的完整反馈、61 维 observation、14 维 action、15 路目标 |
| `cycle_complete` | 本轮目标是否提交、耗时、是否超期；不代表收到写入 ACK |
| `shutdown_complete` | 扭矩关闭和 RAM 恢复是否成功，失败 ID/原因 |

## 与后续 RL 数组的关系

布局参照 `../../../microduck/duck-control/src/obs.rs`：

| Python 风格切片 | 长度 | 本测试中的数据 |
|---|---:|---|
| `obs[0:3]` | 3 | `[0,0,0]`，**模拟陀螺仪** |
| `obs[3:6]` | 3 | `[0,0,-1]`，**模拟直立重力方向** |
| `obs[6:20]` | 14 | 实测角度减桌面测试中心，rad，跳过 ID 10 |
| `obs[20:34]` | 14 | 实测角速度，rad/s，跳过 ID 10 |
| `obs[34:48]` | 14 | 上一轮成功提交的 dummy action；第一轮初始化为 0 |
| `obs[48:61]` | 13 | 全 0 的 command，即本测试不请求机器人行走 |

14 个动作的对应 ID 为：

```text
[1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 12, 13, 14, 15]
```

Microduck 的第 10 个逻辑槽位是 mouth。这里暂用 **ID 10** 演示该槽位，照样参加波浪，单独出现在 15 维 `target_ticks` / `target_deg` 与 `mouth_target_ticks` 中。其余槽位按顺序对应 Microduck 的左腿5、颈头4、右腿5。

```text
实测反馈 -> observation[61]
时间/相位 -> dummy wave -> action[14] + mouth target
目标 = bench_home + amplitude_rad * action
15 个目标 -> 同一个 SYNC_WRITE -> 下一轮实测反馈
```

目标量化为编码器刻度后再计算 action，因此数组和真实下发值相互对应。电流与温度是诊断数据，不属于 Microduck 的 61 维 observation。

**仅格式对齐，不是可直接喂给已训练策略的真实机器人状态。** 无 IMU、无机器人连杆和关节标定，bench home 也不是 Microduck 的训练 home pose。日志固定标注 `policy_ready=false`、`imu_source=synthetic_upright`，`action_source` 按模式为 `dummy_wave` 或 `dummy_mirror`。`demo` 中连舵机反馈也是模拟值。

## 运行设置与退出

`probe` 不写寄存器。`run` 和 `home` 要求型号 1200、位置模式3、单圈限位0..4095、Homing Offset=0、禁用 Secondary ID、使用速度型 Profile，且禁用“目标触发扭矩使能”。不满足时先报出具体 ID/配置，不自动改 EEPROM。

运行只调整下列 RAM 设置，并在退出且确认 torque=0 后恢复原值：

| 寄存器 | 运行值 |
|---|---|
| Goal PWM (100) | 原值与 200 取较小值，约 22.6% PWM 上限 |
| Profile Acceleration (108) | 默认20，约429.15°/s²；可用 `--profile-acceleration` 调整 |
| Profile Velocity (112) | 默认30，约41.22°/s；可用 `--profile-velocity` 调整 |
| Bus Watchdog (98) | 50，即 1 秒没有收到指令时停止运动 |

Torque Enable(64) 和 Goal Position(116) 是本测试主动控制的状态；退出后 torque 保持 0，目标保留最后值，不恢复测试前可能残留的运动目标。

每轮监测：电压 `4.2..5.8 V`、温度 `<55°C`、单舵机电流绝对值 `<350 mA`、15 路电流绝对值之和 `<2000 mA`，任何读包失败、状态错误/Alert 或越界都停止测试并尝试关扭矩。电流是轮询遥测，**不是硬件限流器**，也不能保证捕捉瞬时峰值；5 V / 3 A 电源的限流仍由电源负责。

看门狗停止运动不等于扭矩关闭。USB 拔出、强制杀进程或系统故障时无法保证软件关扭矩；若报告关闭未确认，可直接切断外部舵机电源。

## 验证与源码

```sh
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo build --locked
python3 tests/pty_smoke.py
```

`pty_smoke.py` 通过虚拟串口模拟15个舵机，运行真实 CLI/串口/协议路径，检查正常结束、硬件 Alert、缺失回包、配置写回不一致及 Ctrl+C 时的关闭和恢复；另外检查全部归中、一个舵机未到位超时、经过中位但速度未稳定，以及归位过程中上述异常的退出和恢复。

- `src/bus.rs`：批量读写、设备检查、启动和退出。
- `src/model.rs`：单位换算、波浪、61/14 数组与阈值。
- `src/trace.rs`：串口日志、拆包、CRC/状态检查。
- `src/main.rs`：CLI、周期调度、输出与信号处理。
- `src/home.rs`：15路归中和实际位置/速度到位判定；`home.sh` 为直接启动入口。
- [VALIDATION.md](VALIDATION.md)：本次已完成验证与硬件记录。

依据：本地 `../../../microduck/duck-control/src/bus.rs`、`model.rs`、`obs.rs`；[XL330-M288 官方控制表](https://emanual.robotis.com/docs/en/dxl/x/xl330-m288/)；[Protocol 2.0](https://emanual.robotis.com/docs/en/dxl/protocol2/)；[OpenRB-150](https://emanual.robotis.com/docs/en/parts/controller/openrb-150/)。
