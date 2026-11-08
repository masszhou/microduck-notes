# 来源与许可

- `rkaiq-modinfo-shim.c` 的 ioctl 适配部分来自 [pollen-robotics/microduck](https://github.com/pollen-robotics/microduck/blob/6507d2e960417aaa4ecd38eccf59b2dcf586ecd2/scripts/rkaiq-modinfo-shim.c)，保留原注释与 [Apache-2.0 许可证](licenses/Apache-2.0.txt)。2026-11-08 增加限定条件的 pthread 调度回退并补充构建说明；该文件继续按 Apache-2.0 使用。
- Radxa 的 rkaiq 与 IQ `.deb` 是独立第三方输入，仅按固定 URL/SHA256 获取并保存在本机缓存/个人恢复归档中，不作为本仓库 MIT 原创代码。安装包内 `/usr/share/doc/<包名>/copyright` 及上游许可声明适用于其内容。对外再分发归档前需遵守对应软件包的再分发及源码提供要求。
- IQ 调整由 `configure.py` 从包内文件生成，仅更改两个枚举字符串，不提交厂商 JSON 的副本，不将其重新授权。
- 其余新脚本与文档遵循仓库根目录 MIT 许可证；源码包附 [MIT 许可证](licenses/MIT.txt)。
