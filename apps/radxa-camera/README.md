# Radxa Zero 3W / IMX219 恢复工具 1.0.0

在 Mac 上用一个入口恢复 Armbian 摄像头配置：启用 CSI overlay，安装固定版本的 rkaiq 和 IQ 文件、编译兼容库、保留自动曝光（AE）/自动白平衡（AWB），安装开机服务和 SSH 串流工具。无需安装或编译 `rsetup`，无需在 Mac 上交叉编译。

## 已验证的基线

| 项目 | 本次实测配置 |
| --- | --- |
| 板子 | Radxa Zero 3W，RK3566 / ISP 2.1，aarch64 |
| 系统 | Armbian 26.8.3，Debian 13 trixie；当时 `/etc/os-release` 显示 13.6 |
| 内核 | **6.1.115-vendor-rk35xx** |
| 摄像头 | 用户的“适用于 Pi5”IMX219，模块标识 `rpi-camera-v2` / `default` |
| Boot DTB | `rockchip/rk3566-radxa-zero3.dtb` |
| Overlay | `radxa-zero3-rpi-camera-v2` |
| Sensor / media | `m00_b_imx219 2-0010`，`/dev/media0` |
| 采集 | ISP `/dev/video0`，NV12 1280×960；传感器源为 SRGGB10 3280×2464 |
| 传输 | GStreamer JPEG quality=75 → SSH → Mac ffplay |

安装器会拒绝其他架构、Debian 主版本、内核或 DTB；同一内核下也会检查 overlay 文件并试合并 DTB。**这不是通用的任意 Armbian 镜像安装器。** 请另行保存这次使用的原始 Armbian 镜像及校验值；本工具不包含 OS 镜像、内核、DTB 或 Wi-Fi 配置。本次未记录原始刷机镜像的下载地址/哈希，不能据版本字符串复原该镜像。

## 重装后：在 Mac 上一键恢复

1. 刷入上述基线的 Armbian，创建 `devuser`（或其他有 sudo 权限的普通用户），联网并启用 SSH。断电状态下接好已确认可用的 CSI 排线。先确认板子上的 `uname -r` 与上表一致。
2. Mac 需要 Python 3、OpenSSH、FFmpeg。若缺少：`brew install python ffmpeg`。先手动登录一次确认 SSH 主机密钥，并配置公钥登录，例如：

   ```bash
   ssh devuser@radxa-zero3
   # 退出远端后；若未配置公钥，使用自己已有的公钥：
   ssh-copy-id -i ~/.ssh/id_ed25519.pub devuser@radxa-zero3
   ssh -o BatchMode=yes devuser@radxa-zero3 true
   ```

   如果没有 `ssh-copy-id`，可使用已有的公钥部署方式，或 `brew install ssh-copy-id`。重装后主机密钥通常改变：先在板子本地控制台核对新指纹，再移除该主机的旧 known_hosts 条目并重新信任。工具不会自动忽略或删除 SSH 主机密钥。账户密码和 sudo 密码都不保存到工具中。
3. 在 Mac 进入本目录（或解压归档后的 `radxa-camera` 目录），关闭已有摄像头窗口，然后运行：

   ```bash
   python3 restore.py --host devuser@radxa-zero3 --reboot
   ```

   按提示输入板子的 **sudo 密码**。脚本校验/下载固定厂商包、上传恢复工具、安装配置、重启，并等待新的 boot ID，随后检查服务、统计线程及 60 帧 JPEG 采集。最长等待重启 180 秒；网络/主机名变化时需用实际地址重试。使用 IP 也可以，例如 `--host devuser@192.168.1.50`。SSH 自定义端口通过 `~/.ssh/config` 设置。

   去掉 `--reboot` 可以只安装；首次启用 overlay 后需自行重启。恢复过程中发现不兼容内核、其他摄像头 overlay、不同厂商包版本或正在采集时会退出，不会强杀采集进程。失败会保留远端暂存目录和已创建的配置备份。
4. 开始观看：

   ```bash
   ./watch-camera.command --host devuser@radxa-zero3
   ```

   默认主机是 `devuser@radxa-zero3`，也可直接双击 `watch-camera.command`。按 **q**、关窗口或 **Ctrl+C** 停止。该入口管理独立 SSH 连接并定时发送心跳；退出或断线后只清理本次启动的 GStreamer，避免 `/dev/video0` 一直被占用。静默网络断开时，远端心跳超时为 15 秒，之后最多约 7 秒清理。

## 保存恢复归档

```bash
python3 bundle.py
```

输出 `dist/radxa-camera-1.0.0.tar.gz` 和同名 `.sha256`，归档包含源码、文档和两个固定 `.deb`。**请把这两个文件另存到备份盘。** 包缓存与归档不提交到 Git；源码中的 `assets.py` 保留下载地址和 SHA256。

使用归档时：

```bash
shasum -a 256 -c radxa-camera-1.0.0.tar.gz.sha256
mkdir camera-restore
tar -xzf radxa-camera-1.0.0.tar.gz -C camera-restore
cd camera-restore/radxa-camera
python3 restore.py --host devuser@radxa-zero3 --reboot
```

这份归档避免依赖两个厂商包未来仍可下载，**不是完全离线的 OS 恢复盘**：板子仍需访问 Debian/Armbian APT 源安装编译工具、GStreamer、v4l-utils 等依赖。没有 Mac 也可把解压目录复制到板子，执行 `sudo bash install.sh --user devuser --reboot`；若只从 Git 获取源码，先运行 `python3 assets.py` 下载包。

## 安装过程和保留的修复

- 保留 `/boot/armbianEnv.txt` 中其他设置和 overlay，仅追加 `radxa-zero3-rpi-camera-v2`。多个 overlays 条目或另一摄像头 overlay 会被拒绝。
- 固定 `camera-engine-rkaiq-rk3568` **5.0x1.2-rc2** 与 `rockchip-iqfiles-rk356x` **0.1.16**。来自 Radxa bullseye 包池，但不添加 bullseye APT 源、不替换系统发行版；在当前 trixie 上验证过依赖。下载地址和散列见 [assets.py](assets.py)。
- 从固定 IQ 包生成 `imx219_rpi-camera-v2_default.json`，仅替换两个解析枚举名：`AECV2_STRATEGY_MODE_LOWLIGHT_PRIOR` → `AECV2_STRATEGY_MODE_LOWLIGHT`，`CALIB_AWB_HDR_FRAME_CHOOSE_MODE_AUTO` → `CALIB_AWB_HDR_FR_CH_AUTO`。检查每个 ISP21 场景仍为 AE Enable=1、AUTO，AWB byPass=0、AUTO。原厂 `/usr/share` 文件不修改，以原子替换方式写入 `/etc/iqfiles`。
- 在板子编译 `rkaiq-modinfo-shim.c`。其 ioctl 适配修复内核和旧引擎模块信息结构长度不同导致的 IQ 路径为空问题。
- 同一兼容库仅在 **显式 SCHED_RR priority=20 的线程创建被 EPERM 拒绝时**重试普通调度。这是本机 cgroup 实时调度限制的修复；否则服务可能显示 running，却没有统计线程、AE/AWB 不再持续工作。`LD_PRELOAD` 只用于摄像头服务，没有修改全局调度策略。
- 安装 `/etc/systemd/system/rkaiq_3A.service` 和 `/usr/local/lib/rkaiq-imx219/`，启动前等待 IMX219 并设置传感器源模式。开机启用服务；overlay 尚未生效时等重启。
- 安装用户的 `~/.local/bin/radxa-camera-stream.py`，加入 `video` 组。新 SSH 会话生效；Mac 播放器必须使用本目录的入口。

可重复运行恢复命令；每次保留独立备份、从固定原始 IQ 重新生成配置，不会重复追加 overlay。未用全新镜像实测整个一键流程，见下方验证边界。升级内核、驱动或厂商包后需要重新核验；脚本不会自动降级不同版本的厂商包，也不会锁定系统更新。

## 检查、常见故障、回滚

在上传后打印的板子暂存目录 `~/.cache/radxa-camera-recovery.*/radxa-camera/` 内运行：

```bash
bash check.sh              # 服务、版本、sensor、自动模式、统计线程
bash check.sh --capture    # 额外采集 60 帧；先关闭播放器
systemctl status rkaiq_3A.service --no-pager
journalctl -u rkaiq_3A.service -b -n 100 --no-pager
```

- **Device '/dev/video0' is busy**：板子执行 `fuser -v /dev/video0`，再用 `ps -fp PID` 确认是谁占用。优先关闭对应播放器；只有确认是自己的旧采集任务时，才 `kill -INT PID`。不要批量杀摄像头/ISP 进程。旧的裸 `ssh ... | ffplay ...` 命令不会完整管理退出，请使用新入口。
- **画面偏暗/偏绿**：先看 `check.sh` 是否通过、服务日志是否有统计线程失败或持续的 `no free item for sofInfo`。确认启动后曝光能随遮挡/恢复光照而变化，白/灰物体颜色是否合理。服务 active 或 JSON 为 AUTO 都不能单独证明反馈循环工作。不要用 Mac 色彩滤镜掩盖失效的 AE/AWB。
- **没有 IMX219 entity**：检查 `grep '^overlays=' /boot/armbianEnv.txt`、`media-ctl -d /dev/media0 -p`、`sudo dmesg | grep -Ei 'imx219|rkisp|csi|i2c'`。检查排线前关机断电；这套软件不能判断排线触点朝向，也不能补救错误排线/引脚映射。
- **启动日志出现 focus/首次参数错误**：这颗固定焦距模块本次验证中出现过这类厂商信息；以实际采集和自动调节验证为准，不能据单个 E 行判定好坏。`VIDIOC_SUBDEV_S_CLIENT_CAP` 在本次内核不支持，v4l2-ctl 可能输出错误但后续控制读写仍执行，应核对返回的控制值。
- **SSH 主机密钥提示/密码提示与播放器输出混在一起**：先单独完成 SSH 公钥连接。新播放器使用 BatchMode 与严格主机密钥检查，验证失败会退出。

每次安装会打印配置备份，例如 `/var/backups/radxa-camera/日期-进程号/`，并将路径写入 `/usr/local/lib/rkaiq-imx219/installed-backup`。关闭串流后回滚：

```bash
sudo bash /var/backups/radxa-camera/实际备份目录/rollback.sh
sudo reboot
```

回滚恢复该次安装前的 boot 配置、服务、IQ 文件、兼容库目录和用户串流脚本；服务原先启用/运行的状态也恢复。**依赖包、两个厂商包及 video 组成员资格保留**，`packages-before.txt` 记录安装前包状态。回滚后若原先没有摄像头服务，不会启动新厂商服务。多个备份应按时间倒序回滚，避免混用不同状态。安装失败不会自动移除依赖或自动覆盖回原配置，请按错误输出和备份处理。

本次最初手工安装的旧备份在 `/var/backups/rkaiq-imx219/20261108-204059-2125/rollback.sh`，属于旧工具，与本目录的新恢复备份分开保留。

## 验证记录与边界

2026-11-08 的实际板子验证：安装最终调度兼容库后，统计线程存在；采集中强制 exposure=100 后，下一个约 1 秒采样恢复到 2100，模拟增益变化并收敛。AWB 有连续逐帧运行记录；真实画面亮度和颜色改善，未做标准色卡校准。旧测试包括重复采集、SSH EOF、强制断开、心跳超时、播放器退出、Ctrl+C，摄像头均释放并能再次打开。

恢复工具的验证记录见 [VALIDATION.md](VALIDATION.md)。当前板子保留工作配置；**没有为了测试而重刷系统，也没有把本次整理工具的容器测试等同于全新 Armbian 实机验收**。重装后入口的采集检查通过，再用实时画面检查 AE/AWB 和重连。

第三方来源和许可证见 [NOTICE.md](NOTICE.md)。不包含用户照片、私钥或机器日志。
