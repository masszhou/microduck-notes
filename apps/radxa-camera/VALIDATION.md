# 1.0.0 验证记录（2026-11-08）

## 本次恢复工具验证

- Mac Python 语法检查通过；两个 CLI 的帮助入口可运行。
- 配置转换 4 项测试通过：保留其他 boot 配置与重复执行、拒绝冲突/重复 overlay、仅调整枚举且拒绝非自动模式、原子替换 IQ 符号链接而不修改原厂目标文件。
- ARM64 Podman，`python:3.14.3-slim-trixie` 中 ShellCheck warning 级别检查通过。`gcc -shared -fPIC -O2 -Wall -Wextra -Werror ... -ldl -pthread` 编译通过。
- 同一容器的 pthread 测试通过：被拒绝的 RR(20) 转为普通调度，保留栈大小、可执行和 join；默认线程创建不受影响。
- 固定两个 `.deb` 的 SHA256 校验通过；trixie APT 模拟安装两包及 GStreamer/v4l-utils 等依赖通过。APT 依赖没有锁定到某个 Debian point release（验证时仓库已显示 13.7）。
- 另一个全新 ARM64 trixie 容器实际安装完整依赖及两个厂商包成功；引擎 `ldd` 无缺失库，GStreamer 的 v4l2src/jpegenc 插件可用。显式列入 libstdc++6 与 libdrm2，避免仅依赖旧包不完整的依赖声明。
- 从原厂包生成的 IQ 文件 SHA256：`072cfbf58f360fe88ab08b3b13ab9c091468a8c46bc9f85de512c6c64418f54e`，与此前已在板子上验证 AE/AWB 的文件完全一致。
- 当前实际板子：6.1.115-vendor-rk35xx，服务 active/enabled、IMX219 entity、自动模式和 ISP 统计线程检查通过；新 `check.sh --capture` 完成 60 帧 NV12 1280×960 → JPEG 编码。此次只上传临时检查脚本，未重新安装系统配置或重启。
- 恢复归档解压后，两个包散列校验、配置测试与 Python 语法检查通过。归档不含摄像头图片、私钥或原始采集日志。

## 保留的此前实机证据

最终服务的 AE 干预恢复、连续 AWB、再次采集均已验证；旧串流生命周期测试覆盖 EOF、SSH 被终止、15 秒心跳超时、播放器退出和 Ctrl+C。详见 README。本次保存了相同的远端串流程序，Mac 入口增加 `--host` 参数，并容忍清理 stdin 时的 BrokenPipe。

## 未验证部分

没有重刷 Armbian，也未端到端执行新 `restore.py --reboot` 的 sudo 安装与重启流程；没有把现有板子的工作状态当作重装成功证据。重复安装、备份回滚逻辑已静态检查，尚未在全新实机上做破坏性演练。

将来重新刷机后，应记录：完整安装命令返回成功、新 boot ID、开机服务/60 帧检查、真实画面的 AE/AWB 调节及退出后再次打开。不同内核、不同模组、其他 media 节点顺序和色卡级色准均不在本次验收范围。

## 开发检查命令

```bash
python3 -m unittest discover -s tests -v
python3 -m py_compile *.py
bash -n install.sh rollback.sh check.sh prepare-camera.sh watch-camera.command
shellcheck -S warning install.sh rollback.sh check.sh prepare-camera.sh watch-camera.command
# 以下在 Linux ARM64 上执行：
gcc -shared -fPIC -O2 -Wall -Wextra -Werror -o /tmp/rkaiq-shim.so rkaiq-modinfo-shim.c -ldl -pthread
gcc -Wall -Wextra -Werror -o /tmp/rkaiq-thread-test tests/test-scheduling.c -pthread
LD_PRELOAD=/tmp/rkaiq-shim.so /tmp/rkaiq-thread-test
```

调度回退测试需在禁止实时调度的环境运行；具有实时调度权限的环境不会触发它要验证的回退分支。
