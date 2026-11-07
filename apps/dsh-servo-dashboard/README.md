# DYNAMIXEL 舵机操作台

这是安装在 DSH Web GUI 内的 Cordis dashboard 插件，不是独立网页。侧栏入口为「舵机操作台 / Servo dashboard」。

## 使用

1. 在 DYNAMIXEL Wizard 点击 Disconnect。
2. 打开 DSH 侧栏的「舵机操作台」。
3. 选择 OpenRB-150 串口。
4. 选择当前总线的 Baud rate。本次硬件为 **1 Mbps**。
5. 点击「连接并搜索」。默认搜索 ID 1–20。
6. 点击「完整搜索 ID 0–252」，完成运动前的 Secondary ID 检查。
7. 在总览表点击一个 ID。
8. 编辑目标 Position，或用 −1° / +1° 按钮编辑目标。
9. 确认此舵机可以安全运动。
10. 点击「推送 Position」。

目标编辑不会发送命令。每次推送最多移动 **10°**，后端使用新读取的 Present Position 检查这一限制。Goal Position 是目标寄存器，不是轴的实测位置。只有 `arrivalVerified: true` 才表示插件通过后续遥测核对到位。

总览显示 Present Position、Goal Position、相对 180° 的偏差、Torque、电流、电压和温度。详情显示速度、PWM、LED、Operating Mode、Homing Offset、Position limits、Profile 和 Hardware Error。位置采用绝对 encoder 角度：`degrees = ticks × 360 / 4096`。

## 安全边界

- 连接、搜索和状态轮询均不写寄存器，不自动使能 Torque。
- 插件不写 EEPROM，不修改 ID、Baud rate、Operating Mode、Homing Offset 或 Position limits。
- 手动运动支持发现的 XL330-M288，要求 Position mode、Homing Offset=0、速度型 Profile，且禁用 Torque On by Goal Update。
- 插件在首次运动前检查完整总线的 Secondary ID，避免一个 ID 的指令影响其他舵机。检查失效时，先释放本插件控制的 Torque，再重新完整搜索。
- 插件检查新读取的电压、温度、电流、硬件错误和位置边界。软件轮询不是硬件限流器。
- 插件临时使用受限 PWM、速度、加速度和 Bus Watchdog。详细值及恢复规则见 native/CONTRACT.md。
- 成功推送后，插件保持该舵机的 Torque。点击「释放此舵机 Torque」可结束保持。
- 「释放本插件控制的 Torque」仅影响本插件接管的舵机，不关闭其他程序已开启的 Torque。
- 如果需要手动关闭一个其他程序已使能的舵机，请选择该 ID，完成完整搜索和安全确认，再点击「关闭此舵机 Torque（卸力）」。这是单个舵机的显式人工操作；Agent 不具备关闭未接管舵机 Torque 的权限。
- 对于 Torque 已开启的舵机，直接推送要求当前目标与实测位置相差不超过 1°，且速度绝对值不超过 0.229 RPM。否则先按上述步骤明确关闭 Torque，再推送新目标。关闭 Torque 可能使承重机构下落。
- 离开操作台、切换到后台页面或失去心跳约 5 秒后，插件尝试释放接管的舵机。需要持续保持时，请保持操作台在前台。
- 本插件接管一个原先 Torque=ON 的舵机后，释放仍将该舵机 Torque 设为 OFF；它不会重新使能旧目标。
- 通信中断、USB 拔出、进程强制终止或电源异常时，软件不能保证关闭 Torque。Bus Watchdog 不等于 Torque OFF。必要时切断外部舵机电源。
- `UNKNOWN` 表示写入或到位结果不确定。不要自动重复发送。先读取状态并检查硬件。

## Agent 操作

`servo_dashboard` 与 UI 调用同一个 Host controller。Agent 的运动权限独立于 UI：仅允许 ID **1、2、3**，并要求实测位置和目标均在 **170°–190°**。Agent 无法通过参数修改这一限制，也不能接管人工控制的舵机。

工具动作：`ports`、`connect`、`scan`、`status`、`move`、`heartbeat`、`release`、`stop`、`disconnect`。`move` 需要唯一 `commandId`，同一个 ID 不会重复发送。状态查询不会续租 Torque 保持；只有当前控制方的 `heartbeat` 会续租。

## 架构与开发

- `client.js`：原生 React slot 页面，使用 DSH locale 与 theme tokens。
- `index.js`：经过 DSH Connection 身份校验的 API route 与 Agent tool。
- `controller.js`：单个 native worker、调用方隔离、去重、错误与状态反馈。
- `native/`：Rust 串口与 Protocol 2.0 实现，参考已有 mac-servo-bench 的 rustypot / serialport 交互。
- `bin/servo-worker`：供本机 DSH Host 启动的已编译程序。
- `tests/`：Host/controller 单元测试与 native 离线测试。

```sh
npm test
npm run check
cargo test --locked --manifest-path native/Cargo.toml
npm run build:native
```

`npm run build:native` 会构建生产版本并更新 `bin/servo-worker`，同时保留执行权限。插件不会自动运行安装脚本，也不依赖独立 HTTP server。当前二进制面向本次 macOS 主机；迁移到另一平台时必须重新编译。

## 仓库迁移与本地安装

源码现在位于 `microduck-notes/apps/dsh-servo-dashboard`。上述开发命令从本目录运行；也可从仓库根目录使用 `npm --prefix apps/dsh-servo-dashboard ...`。

Git 只保存源码、测试、文档和精选验证记录。`native/target/`、`bin/servo-worker`、`artifacts/` 和本地 checkpoint 不提交。首次 clone 后，先获取 Rust 依赖，再构建本机程序：

```sh
cargo fetch --locked --manifest-path native/Cargo.toml
npm run build:native
```

在 DSH 插件管理器中，从本目录的实际绝对路径安装 `@local/dsh-servo-dashboard`。如果移动目录，先移除旧的插件链接，再从新位置安装；不要直接修改 DSH profile 文件，也不要保留旧路径兼容链接。

Host 通过 `import.meta.url` 定位 `bin/servo-worker`，测试通过各自文件的位置定位工程。运行时不依赖开发者的 home 路径。串口是运行时枚举结果；文档中的 `/dev/cu.usbmodem1101` 仅代表历史测试设备。

首次交付的硬件记录见 [VALIDATION.md](<VALIDATION.md>)，浏览器请求修复见 [HOTFIX-1.0.5.md](<HOTFIX-1.0.5.md>)。首次记录中的“没有 Git repository”是迁移前的历史情况。
