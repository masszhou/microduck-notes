# v1.0.5：界面请求拒绝修复

## 结果

已安装到当前 DSH web profile，`application=applied`，warnings 为空。用户刷新一次页面后，实际浏览器 POST 检测成功。15 个舵机在线，`error=null`、`busy=null`、`controlledIds=[]`。本次没有发送运动或卸力命令。ID 11、12 保持 Torque ON。

## 根因与修复

DSH Connection 在进入插件前检查 Host、Origin 和登录身份。HTTP bridge 随后将内部 `Request.url` 改为 `http://dsh.internal/api/servo-dashboard`，但保留原始 Origin。

v1.0.4 错误地将浏览器 Origin 与内部 URL 比较，导致全部合法 POST 返回 403。此前 Host 单元测试使用了未经 bridge 转换的 URL，所以没有覆盖真实页面路径。这是插件实现和测试 fixture 的缺陷。

v1.0.5 删除这项错误比较。DSH 原有认证与 Host/Origin 防护保持不变；插件继续要求 JSON、自定义 header，并拒绝 `Sec-Fetch-Site: cross-site`。没有放开 CORS 或增加免认证入口。

Client 使用同一个 request helper 执行按钮请求和一次只读启动检测。检测仅 POST `status`，不启动串口、不发送 heartbeat、不发送运动。Host 记录不含凭据的请求计数与 Origin，供实际链路核对。

同时修复：

- 已连接时，串口下拉框显示当前端口，不再误示“没有检测到串口”。
- 已连接时仍可刷新串口列表。
- 点击舵机任意一行即可选择；ID 按钮继续支持键盘操作。
- 如果所有舵机重新提供有效数据，界面将旧 poll 故障标为“读取已恢复，运动前仍需完整搜索”。原故障文本和安全限制保留。没有清除 native 的故障检查或自动恢复运动权限。

ID 6 的原始串口超时原因不确定。观察到后续读取恢复；本次没有修改 native timeout 或降低安全阈值。

## 验证

1. 先把单元测试 Request.url 改为真实 bridge 使用的地址。旧实现出现两个失败，复现浏览器 403。
2. 最终 **26 项 JavaScript tests 全通过**，syntax checks 通过。
3. `tests/transport-integration.mjs` 直接导入当前 DSH 的真实 trust fence 与 HTTP bridge。合法请求通过；恶意 Origin、opaque Origin、恶意 Host、cross-site marker 均拒绝；只读 status 没有启动 native worker。
4. 在实际 `http://127.0.0.1:3080` 上验证：无登录 GET=401；无登录 POST=401；跨站 POST=403。
5. 用户刷新后，实际浏览器的自动检测读回：

```json
{
  "version": "1.0.5",
  "uiTransport": {
    "successfulPosts": 1,
    "lastAction": "status",
    "lastSuccessAt": 1794088745650,
    "browserOrigin": "http://127.0.0.1:3080",
    "carrierOrigin": "http://dsh.internal"
  },
  "onlineCount": 15,
  "error": null,
  "controlledIds": []
}
```

6. Production native JSONL smoke 通过。Native 仍为 1.0.2，源码和 binary 未改变；沿用此前 25 项 Rust、13 项 PTY 场景结果，本次没有重复运动测试。

## 验证边界

已验证真实浏览器 POST 通道，但未自动操作鼠标、键盘或复测 light/dark 外观。用户只执行了一次页面刷新，没有代操作舵机。首次交付的其他硬件记录保留在 VALIDATION.md。

运行 bridge 回归：

```sh
node tests/transport-integration.mjs /path/to/deepseek-harness/packages/client/connection/src
```

原 v1.0.4 安装包保留作历史记录。新安装请使用 `local-dsh-servo-dashboard-1.0.5.tgz`。
