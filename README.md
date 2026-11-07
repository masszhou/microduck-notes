# microduck-notes

本仓库保存 Microduck 项目的开发成果、应用、文档和硬件资料。所有本项目的开发和文档修改都在本目录内完成。

## 目录

| 路径 | 内容 |
|---|---|
| [apps/dsh-servo-dashboard](<apps/dsh-servo-dashboard/README.md>) | DSH 多舵机 dashboard、native worker、测试与验证记录 |
| [apps/mac-servo-bench](<apps/mac-servo-bench/README.md>) | OpenRB-150 / XL330 的 Rust 桌面测试工具 |
| [docs](<docs/>) | 物料与研究文档 |
| [hardware/imu_to_dxl](<hardware/imu_to_dxl/README.md>) | IMU 转 DYNAMIXEL 资料和 KiCad 迁移工程 |
| [hardware/Robot_HAT_C1](<hardware/Robot_HAT_C1/README.md>) | 官方 HAT C1 参考工程，保留其 Apache-2.0 许可证 |
| `data/` | 预留的数据目录；当前为空，不由 Git 保存空目录 |

父目录中的 `Fanduck`、`microduck`、`microduck_rl`、`MicroDuck-BoBoRedesign`、`microduck-replica` 是独立的第三方参考目录。本仓库不包含这些目录，不在其中开发，也不将它们加入本仓库的提交。

## 本地验证

以下命令从本仓库根目录运行，不发送真实舵机运动命令：

```sh
npm --prefix apps/dsh-servo-dashboard test
npm --prefix apps/dsh-servo-dashboard run check
cargo test --locked --manifest-path apps/dsh-servo-dashboard/native/Cargo.toml
cargo test --locked --manifest-path apps/mac-servo-bench/Cargo.toml
```

首次 clone 后，在安装 DSH 插件之前构建本机 native worker：

```sh
cargo fetch --locked --manifest-path apps/dsh-servo-dashboard/native/Cargo.toml
npm --prefix apps/dsh-servo-dashboard run build:native
```

再通过 DSH 插件管理器安装当前 clone 中 `apps/dsh-servo-dashboard` 的实际绝对路径。目录移动后需要重新安装本地链接。仓库不保存某一台主机的 DSH profile 配置。

## 版本控制与安全

- 提交源码、Cargo.lock、文档、硬件工程和已有制造/检查资料。
- 忽略 build cache、native 可执行文件、安装包、串口日志、本地 agent checkpoint、凭据和 KiCad 临时状态。已有本地文件不删除。
- `mac-servo-bench` 的 `run`、`home`、`off` 会写真实设备，不能用来验证目录迁移。
- 旋钮实时控制的开发仍暂停。目录迁移和 Git 操作不构成舵机运动授权。
- 本仓库原创内容使用 [MIT License](<LICENSE>)。第三方资料不适用统一的 MIT 重授权，来源与原许可证见 [NOTICE.md](<NOTICE.md>)。
