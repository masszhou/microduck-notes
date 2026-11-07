# 来源与许可证

本仓库整理个人开发成果和第三方参考资料，与 Pollen Robotics 无隶属关系，也不表示获得其背书。根目录的 [MIT License](<LICENSE>) 不为第三方内容重新授权。

## RPI Robot HAT C1

[hardware/Robot_HAT_C1](<hardware/Robot_HAT_C1/README.md>) 来源于 [Pollen Robotics 的 elec_RPI_Robot_HAT](https://github.com/pollen-robotics/elec_RPI_Robot_HAT)。本仓库保留目录内的 [Apache-2.0 LICENSE](<hardware/Robot_HAT_C1/LICENSE>)。器件手册和厂商资料保留各自作者的权利，不视为本仓库原创。

## imu_to_dxl 参考资料与 KiCad 转换

[hardware/imu_to_dxl](<hardware/imu_to_dxl/README.md>) 的原始 EDA 工程、图纸、说明和接线资料来自 [fanhao375/microduck-replica](https://github.com/fanhao375/microduck-replica)。KiCad 转换记录使用的上游基线为 `c2d2ffefc133437add912961917fe5049a287a7b`。

- 上游署名说明：[NOTICE.md](https://github.com/fanhao375/microduck-replica/blob/c2d2ffefc133437add912961917fe5049a287a7b/NOTICE.md)。
- 上游许可证：[LICENSE](https://github.com/fanhao375/microduck-replica/blob/c2d2ffefc133437add912961917fe5049a287a7b/LICENSE)。
- 上游声明其原创代码采用 Apache-2.0，文档、图片及模型衍生内容采用 [CC BY-NC-SA 4.0](https://creativecommons.org/licenses/by-nc-sa/4.0/)。这些材料继续保留原作者署名、非商业和相同方式共享条件，不适用本仓库根目录的 MIT 重授权。
- 上游引用的 Microduck 源码和模型来自 [Pollen Robotics](https://github.com/pollen-robotics)，应同时遵守上游对这些内容的许可说明。
- 本仓库的 KiCad 转换与整理不等于重新取得原硬件设计的著作权。对于上游没有明确覆盖的硬件文件，不能据根目录 MIT License 推断其商用授权。

本次目录迁移仅调整文档路径、失效预览链接和网表中的本机路径元数据；没有重新设计或重新验证电路。KiCad 转换的具体改动与未验证范围见 [转换说明](<hardware/imu_to_dxl/kicad/README.md>)。

## 应用代码中的参考

[mac-servo-bench](<apps/mac-servo-bench/README.md>) 和 [dsh-servo-dashboard](<apps/dsh-servo-dashboard/README.md>) 使用 Rust crates 提供的协议与串口能力；依赖版本保留在各自 Cargo.lock。它们不将父目录中的第三方实现作为本仓库源码提交。引用外部源码、规格或文档时，保留对应来源和许可证。
