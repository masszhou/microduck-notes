# 交付与验收记录

当前版本为 **1.0.5**。浏览器 POST 拒绝问题已修复，真实浏览器检测已通过。最新证据见 [v1.0.5 修复记录](<HOTFIX-1.0.5.md>)。以下保留首次 v1.0.4 交付时的历史结果，不作为当前 UI 验证结论。

## 首次交付结果

已在当前 DSH `web` profile 安装 `@local/dsh-servo-dashboard` **1.0.4**。入口是侧栏「舵机操作台 / Servo dashboard」。目标 GUI 仍为 `http://127.0.0.1:3080`，没有启动替代 server。

最终 native worker 为 **1.0.2**，macOS arm64 production build。插件保留与 OpenRB-150 的只读连接，15 个舵机均在线，`controlledIds=[]`。打开操作台后可读取 Position；任何新运动都需要先执行完整搜索并明确推送。

源码目录没有 Git repository。本次未创建 commit，未修改既有 mac-servo-bench 源码。既有项目仅增加最初只读 probe 的运行日志。独立 Claude Review 为 **0 轮**；用户没有指定审阅预算。本记录不将实现子代理的测试称为独立审阅。

## 实际安装证据

- 最终 `plugin_manager install_bundle`：`application=applied`，version `1.0.4`，warnings `[]`。
- 实际工具返回 Host version `1.0.4`、`hostRevision=r4-commissioned-controls`、worker version `1.0.2`。
- Client Inspect 确认 `sidebar.panellist` 的 `servo-dashboard` 和 `main` 的 `servo-dashboard` 都为 active。
- 同一最终版本经过卸载、重新安装后，Client 注册仍然存在。
- zh/en 的标题和说明来自导出的 locale 文件。原创 SVG 已通过 XML 检查，保留在实际安装目录。
- 无凭据请求 `GET /api/servo-dashboard` 返回 HTTP 401。JSON header、Origin 和输入校验另有 Host 单元测试。

## 硬件验证

设备：OpenRB-150，`/dev/cu.usbmodem1101`，**1,000,000 bps**。完整搜索覆盖 ID 0–252，发现 ID **1–15**，均为 model **1200**、Firmware **53**、Operating Mode **3**。

用户明确调试原点为 **180°**。Agent 的实测位置和目标均限制在 **170°–190°**，仅允许 ID 1、2、3。本次只向 **ID 1** 发送运动与恢复写入；ID 2–15 在开发期间均只读。

最终运动链路通过安装后的 `servo_dashboard` 工具执行，工具和 UI 使用同一个 Host controller：

| 操作 | 寄存器目标 | 实际读回 | 结果 |
|---|---:|---:|---|
| 推送 184° | 2094 ticks / 184.042969° | 首次到位判定 2086 ticks / 183.339844° | `readbackVerified=true`、`arrivalVerified=true` |
| 保持并发送 heartbeat | 2094 ticks | 2090 ticks / 183.691406°，0.229 RPM | Torque ON，未请求中间卸力 |
| 连续推送原始目标 | 2071 ticks / 182.021484° | 首次到位判定 2081 ticks / 182.900391° | `readbackVerified=true`、`arrivalVerified=true` |
| heartbeat 租约结束 | 不再发送运动目标 | Torque OFF，控制集合清空 | PWM/Profile/Watchdog 恢复 |

最终 ID 1 实测 **2077 ticks / 182.548828°**。它与首次测试前的 **2071 ticks / 182.021484°** 相差 **0.527344°**。程序确认回到原始目标的 **1° 到位容差**内，但没有宣称机械位置精确恢复。关闭 Torque 后，固件和机械状态可能使 Goal/Present Position 继续变化。

恢复读回：Goal PWM **885**、Profile Velocity **0**、Profile Acceleration **0**、Bus Watchdog **0**、Torque **OFF**。舵机 11、12 保持原先的 Torque **ON**，Goal 分别为 **2048** 和 **1866** ticks；其 RAM Profile/PWM/Watchdog 未改变。

详细事实：`evidence/hardware-final.json`。首次调试及故障复现：`evidence/hardware-commissioning-1.json`。

## 最终回归

| 验证 | 结果 |
|---|---|
| JavaScript Host/controller/client source-contract tests | **23 passed** |
| Rust unit tests | **25 passed** |
| 实际 JSONL / serial / rustypot PTY 场景 | **13 passed** |
| Production JSONL smoke，不打开真实串口 | passed |
| JavaScript syntax checks | passed |
| `cargo clippy --all-targets -- -D warnings` | passed |
| `cargo fmt --check` | passed |
| `npm run build:native`，生产构建及原子替换 | passed |
| 真实 15 路发现、状态读取、ID 1 连续运动及清理 | passed |
| 最终 Client slot 注册及重装 | passed |

PTY 测试覆盖：CRC、错误 ID、Alert、丢包、6 秒到位超时、UNKNOWN 不重发、输入类型与范围、向内量化、actor 隔离、心跳超时、EOF/SIGTERM、失败恢复保留 dirty 状态、连续两次运动、显式 human 单独关闭 ID 11 Torque 且保留 ID 12。

Native 生产文件 SHA-256：

```text
239b9b9cad0f56f4d0dc54ad4a79a87be4a01f3dd339885240771b189d45d575
```

归档名称、完整文件列表与归档 SHA-256 另存于 `artifacts/DELIVERY.json`，避免把归档自身 hash 写进归档造成循环。

## 已修复的真实问题

1. 静止 XL330 有时报告原始速度 1，即 0.229 RPM。严格要求零速度会拒绝后续命令。最终实现沿用既有 bench 的低速阈值，允许原始速度 −1..1，并保留目标误差不超过 1° 的要求。真实连续运动与离线反例均通过。
2. 初始 Torque 已开启但旧 Goal 与 Present 相差超过 1° 的舵机无法直接接管。操作台增加显式人工单舵机卸力入口，要求总线检查与安全确认。Agent 仍不能关闭未接管舵机的 Torque。
3. Client locale 注册没有归入 `ctx.effect`，重装后可能残留字典并使页面入口注册失败。最终修复 disposer 归属；相同最终版本的卸载和重装后，两个 Client slot 仍正常注册。
4. macOS 原地覆盖已执行的 Mach-O inode 会触发 code-signing 缓存问题。构建脚本改为经过路径检查的同目录新文件及 atomic rename，生产 smoke 已通过。
5. 多个 Host 并发操作的 busy 状态、接管前的 owner 保留、人工卸力失败后的 owner 保留、原生命令字段及 commandId 去重均有回归测试。

## 未验证范围

- 当前工具没有浏览器控制能力。本次未执行真实页面的鼠标点击、键盘交互、截图、light/dark 视觉检查或浏览器 console 检查。Client slot active 不等于这些视觉项目已经通过。
- 实际 UI HTTP 调用未通过浏览器点击验证；Host route 契约测试和真实 Agent tool 调用了同一个业务 controller。
- 未对 ID 2–15 做真实运动或卸力测试。人工关闭原本已使能舵机的路径仅有 unit/PTY 验证。
- 没有通过物理拔 USB、断电或短路测试故障。软件与 Bus Watchdog 不构成安全级硬件急停。
- 到位判定不是零速度、无机械负载或精确静止的证明。
- 同 Primary ID 的物理重复设备、静默设备和扫描后热插拔不能仅通过串口绝对排除。需要独占、固定、唯一 Primary ID 的总线。

## 人工触点与持续目标

- 需求澄清：1 次问答，包含原点选择和 Wizard Disconnect 确认。
- 串口权限：首次只读 probe 遇到 sandbox 限制，精确重试获得批准。随后用户将环境改为 Full access / never approval。
- 登录：0 次。要求用户代操作运动命令：0 次。要求用户排查普通代码问题：0 次。
- Goal：`goal-2df3a3c5-8d1f-4e28-ae70-1e0311b7ac35`。实际 `roundsStarted=0`；子代理回调不计为自动续轮。
- 模型 token 数与有效工时不确定。本次不以代码字节或墙钟时间替代。
