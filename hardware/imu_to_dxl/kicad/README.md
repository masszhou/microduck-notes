# imu_to_dxl · KiCad K1

2026-09-13。基于本仓库第三方复刻设计，重建为 **KiCad 10 原生、可编辑工程**。已在本机 KiCad 10.0.6 PCB Editor 中实际打开。包含原理图、已布线 PCB、本地符号和封装库。

**这是待打样的复刻原型。** 检查通过说明 KiCad 中的连接与设计规则一致，不能替代器件选型评审、固件开发或实物验证，也不代表恢复了 Pollen Robotics 的官方设计。

## 打开工程

在 KiCad 中打开 [`imu_to_dxl.kicad_pro`](imu_to_dxl.kicad_pro)，随后进入原理图或 PCB 编辑器。移动工程时请复制整个 `kicad` 文件夹；本地库通过 `${KIPRJMOD}` 相对引用，无需安装额外符号库。

| 文件 | 内容 |
|---|---|
| [`imu_to_dxl.kicad_sch`](imu_to_dxl.kicad_sch) | 完整电路、连接器引脚、供电、时钟及总线缓冲 |
| [`imu_to_dxl.kicad_pcb`](imu_to_dxl.kicad_pcb) | 双层布局、走线、过孔、已填充的底层地铜 |
| `imu_to_dxl.kicad_sym`、`sym-lib-table` | 项目符号库与注册表 |
| `imu_to_dxl.pretty/`、`fp-lib-table` | 项目封装库与注册表 |
| [`BOM.md`](BOM.md) | 从源工程元件字段整理的物料表，未查询库存或替代料 |
| [`previews/imu_to_dxl.svg`](previews/imu_to_dxl.svg) | 可放大的原理图预览；同目录另有 PNG |
| [`previews/pcb-top.svg`](previews/pcb-top.svg)、[`previews/pcb-bottom.svg`](previews/pcb-bottom.svg) | 顶层布线与装配轮廓、镜像底层铜；另有 PNG |
| `checks/` | ERC、DRC、引脚核对、网表及移除过孔记录 |
| `source/` | 原始 EDA 文件副本、恢复的设计快照、预期引脚网络表 |

## 板子参数

| 项目 | K1 数值 |
|---|---|
| 外形 | 名义 45 × 22 mm，四角 R2 |
| 层数 / 板厚 | 2 层 / 1.6 mm |
| 安装孔 | 2 个直径 2.2 mm 非金属化孔 |
| 孔中心 | 从板框左下角起，MH1=(2.667, 2.413) mm；MH2=(42.672, 19.685) mm |
| 电子元件 | 29 个；另有 2 个安装孔封装 |
| 电气引脚 | 105 个；23 个有效网络，另有 4 个明确未连接引脚 |
| 走线 / 过孔 | 227 段 / 20 个 |
| 地铜 | B.Cu；已重新填充 |
| 主要设计规则 | 间距 ≥0.15 mm；线宽 ≥0.15 mm；铜到板边 ≥0.30 mm；过孔直径/孔径 ≥0.60/0.30 mm |

板框按资料中的名义尺寸重建，并保留源工程孔位和元件位置。没有将源资料中的“1 oz 铜厚”转为经板厂确认的叠层；生产参数需在打样前确定。

## 接口与源资料差异

以下针脚按**封装焊盘编号**列出；不能仅凭插头朝向或线色推断编号。

| 接口 | 1 | 2 | 3 | 4 | 5 | 6 |
|---|---|---|---|---|---|---|
| J1、J2，EH 2.5 mm | GND | VDD_BUS | DXL_BUS | — | — | — |
| J4、J5，PH 2.0 mm | DXL_BUS | VDD_BUS | GND | — | — | — |
| J3，SWD + UART | GND | SWDCLK | SWDIO | **UART_RX** | **UART_TX** | +3V3 |

J3 的 RX/TX 以板上 MCU 为参照：外部串口 TX 接 J3.4，外部 RX 接 J3.5。源工程实际连接及原理图图片都是 **4=RX、5=TX**，部分文字接线说明则相反；本次按实际工程连接保留。J3 的串口网络属于 3.3 V MCU 引脚。

J4/J5 保留了源工程的 PH 型封装；**尚未确认它与实际飞特线束或 AMP 2.0 插头机械兼容**。两个接口组的第 1、3 脚顺序相反。

源资料的新旧段落另有两处差异，本次按最新 EDA 连接处理：

- `RX_EN` 接 U1.1、U3.7，由 R7 的 10 kΩ 上拉保持默认使能；并非永久直连 3.3 V。`DE` 接 U1.8、U3.1，发送使能为低有效。
- X1 为 16 MHz **有源振荡器**，X1、C9、R3 均设置为装配。输出经 R3=33 Ω 接 U1.2 `PC14 / OSC_IN`，不是早期文档所述默认 DNP 的无源晶体。固件仍需正确配置时钟。

## 重建方法与改动

源文件是嘉立创 EDA 专业版 `.eprj2`。从其保存历史恢复了最新原理图、PCB、器件和库对象，再写成原生 KiCad 对象；没有把原理图或 PCB 图片当作不可编辑底图。

保留了源工程的器件、引脚网络、焊盘几何、布局与 227 段走线；U4 的不规则铜焊盘作为 KiCad 自定义焊盘保留。处理删除过的原理图线段时，同时核对了其所属 WIRE 对象，避免把历史孤立线段恢复为错误连接。

K1 的明确改动如下：

1. 按名义 45 × 22 mm、R2 重建原生板框；安装孔沿用源坐标。
2. 将 `3.3V` 命名为 `+3V3`，振荡器输出自动网名改为 `Net-(X1-OUTPUT)`；KiCad 局部标签生成的 `/` 前缀只影响名称显示。
3. 添加电气引脚类型、未连接标记和供电标记；关联原理图与 PCB，保留器件型号、立创编号和数据手册字段。
4. 整理原理图标注及 PCB 位号、首脚／阴极标记。原封装的外形图移到 F.Fab，减少丝印与焊盘冲突。
5. 移除 **11 个仅在底层连接地铜、顶层没有连接的多余 GND 过孔**，对应 KiCad 的 `via_dangling` 报告。未移除信号或电源走线；坐标见 [`checks/removed_unused_vias.json`](checks/removed_unused_vias.json)。
6. 原文件的独立泪滴铜形状未导入；K1 使用走线、焊盘和地铜本身连接。封装未添加完整 courtyard 装配间隙轮廓或 3D 模型。

## 已完成的数字检查

| 检查 | 结果 | 证据 |
|---|---|---|
| 源工程逐引脚与 KiCad 原理图网表比对 | 29 个元件、105 个电气引脚，0 不一致 | [`pin_net_comparison.json`](checks/pin_net_comparison.json) |
| KiCad ERC | 0 项错误或警告 | [`erc.json`](checks/erc.json) |
| KiCad PCB DRC | 0 项错误或警告 | [`drc.json`](checks/drc.json) |
| 未连接项 | 0 | 同上 |
| 原理图与 PCB 一致性 | 0 项问题 | 同上 |
| 文件与画面 | 原生工程已在 KiCad 打开；原理图及正反面输出已查看 | `previews/` |

没有添加逐项 DRC 排除项。报告保留了 KiCad 默认忽略的检查项，其中包括 `missing_courtyard`；由于没有完整装配间隙轮廓，**上述结果不覆盖器件本体、插头外壳与机械结构碰撞**。也没有独立完成全部器件数据手册及保护电路的工程复审。

迁移检查补充（2026-11-07）：历史 `checks/artifact_sha256.json` 的 38 项中有 37 项与当前文件一致。`imu_to_dxl.kicad_pro` 当前 hash 为 `4b3cdba5b32d82fef5dfe11ee7cdbdc6113b2f1c7b8b1176bce5723d7bc6e814`，与历史记录不同；文件修改时间为 2026-09-20，早于本次迁移检查。本次保留当前配置，不重写历史清单，也不据历史 ERC/DRC 报告宣称当前配置已经重新验证。

## 尚需验证

- **固件**：本次没有编写或烧录固件。让它在总线上表现为舵机、响应目标 ID 与寄存器读取，需要 MCU 的协议、半双工方向控制、IMU 读取与时钟配置。1 Mbps 通信、姿态轴向和数据格式尚未实测。
- **连接与装配**：核实购买的连接器系列、首脚朝向、线束顺序、实际封装尺寸、安装孔和整机空间。无 3D 模型的 PCB 视图不能证明装配适配。
- **通电与负载**：尚未打样、焊接、通电或测量波形。源布局的总线电源路径及接插件没有做下游舵机电流／温升验证；F1 保护本板供电分支，并不限制总线直通支路的电流。

## 复现与维护

正常修改请直接使用 KiCad；下面的脚本仅用于复现迁移。**重新运行生成脚本会覆盖原生工程，不适合在手工修改之后直接运行。**

迁入 `microduck-notes` 后，`checks/netlist.xml` 中的 `design/source` 已规范化为相对报告文件的路径，不再保存开发者的 home 路径。其余网表与日期保留历史结果；这不是一次新的 ERC/DRC 验证。重新导出报告时，提交前需要再次检查本机路径。

需要带 `pcbnew` 的 KiCad Python。当前 Mac 的解释器位于 `/Applications/KiCad/KiCad.app/Contents/Frameworks/Python.framework/Versions/3.9/bin/python3.9`。

1. `build_kicad.py`：从 `source/design.json` 生成初始工程，此时包含源文件的 31 个过孔。
2. `finish_silkscreen.py`：整理 F.Fab、丝印位号和首脚标记。
3. `sync_and_verify.py`：导出原理图网表，核对全部引脚，同步 PCB 网络／元件字段和本地封装库。
4. 运行 `kicad-cli pcb drc --refill-zones --save-board --schematic-parity --format json -o checks/initial-drc.json imu_to_dxl.kicad_pcb`。
5. 仅针对这份新报告运行 `remove_unused_vias.py checks/initial-drc.json`；不能复用旧板的报告。
6. 再次填铜并运行 DRC／原理图一致性检查，同时运行 `kicad-cli sch erc --format json -o checks/erc.json imu_to_dxl.kicad_sch`。交付的 `checks/drc.json` 是最终报告。

源快照只包含绘图和库对象，不包含运行本次转换所需的外部服务依赖。原 `.eprj2` 副本同时保留，方便回到原软件核查。

## 来源与署名

第三方设计来源：[fanhao375/microduck-replica](https://github.com/fanhao375/microduck-replica)，本地基线提交 `c2d2ffefc133437add912961917fe5049a287a7b`，路径 `hardware/imu_to_dxl/`。本次使用同目录的 EDA 工程、原理图／PCB 图纸及逆向说明。

源 `.eprj2` 的 SHA-256：`4fef83c09d2db93c257a41728f3a30559384a1a201ccc825cb206e3c4bb9a243`。

EDA 文件读取方法参考 [Dao-PCB-Design-Agent 的格式解码实现](https://github.com/dao-genesis/Dao-PCB-Design-Agent/blob/main/lceda_bridge/cdp_studio/eprj2_codec.py)；对象几何解释参考 [KiCad 官方 EasyEDA Pro 导入器](https://gitlab.com/kicad/code/kicad/-/tree/master/pcbnew/pcb_io/easyedapro)。本目录的转换脚本为本次编写。

原资料的署名和许可证约定见仓库 [`NOTICE.md`](../../../NOTICE.md) 与 [`LICENSE`](../../../LICENSE)。本次迁移保留来源，不为第三方设计另行指定许可。K1 与 Pollen Robotics 无隶属或背书关系。
