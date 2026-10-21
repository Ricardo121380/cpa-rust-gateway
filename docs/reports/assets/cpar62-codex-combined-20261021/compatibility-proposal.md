# #62 原样 Codex 实际续轮与外部隔离：待授权的有界契约补充

状态：PROPOSED；尚未修改任何生产准入、core 解析或 builder 默认拒绝。记录时间：2026-10-21 Asia/Shanghai。

## 当前原始证据

- `baseline-fresh`：实际 Codex 0.161.0 六项严格成功判据均 false；Pi 正常/失败/取消回归通过。
- `driver-2`：四项私有资格和 controlled compiler 装配后首个公开请求 HTTP 200、独立 receiver=1、实际工具开始。TTY 创建受外层文件隔离阻止。
- `driver-3`：只准许 `/dev/ptmx` 与 `/dev/ttys[0-9]+` 内核终端设备后，真实 exec 工具报告内部 sandbox-exec `sandbox_apply: EPERM`。真实续轮 reasoning 缺 id、含 null content/encrypted_content，function_call_output 带新 id，第二公开请求 400。
- `external-red`：同版本 Codex 原生 turn/start externalSandbox / restricted；外层 OS fence 保持。实际 C4 声明 `sandbox="external"`、`sandbox_mode="external-sandbox"`，首个公开请求 400，receiver=0。没有删除或改写客户端字段。

## 拟议补充（只在批准后实施）

1. **无 id reasoning 输入历史**：限定已有 summary-only、id absent、content absent/null、encrypted_content absent/null 的固定客户端形状；summary 原文/顺序完整保留，不生成 id，不将 null 当密文，不绕过现有 owned reasoning 身份与权限规则。存在 id 的历史继续原校验。空白/坏 id、未知键、非空密文、不可验证内容仍拒绝。此客户端形状不是当前公开 create schema 的标准符合声明，须独立私有 local-controlled Build history 资格，默认生产和其他 adapter 拒绝。
2. **工具结果自身 id**：仅 absent 或既有 1–512 ASCII graphic 非空字符串规则；原值送独立 receiver、参与同份历史原生 item ID 唯一性；与 call_id 分开。`call_id` 仍必须关联先前准确工具调用，name/namespace presence/value 保持。id 不提供 owner、路由、幂等或查找权限。采用同一独立有界 history 资格，默认 builder/生产未资格拒绝。未知成员不放行。
3. **C4 外部沙箱诊断值**：仅增加实际固定客户端的配对 `sandbox="external"`、`sandbox_mode="external-sandbox"`。保留原 seatbelt/workspace-write；所有值仍是不可信客户端声明，仅完整加密本地留存并授权精确读回，不传播、不改变网关权限、不证明隔离。实际隔离由独立 OS 探针和运行配置实证，不能由元数据取得测试资格。

## 不变的验收含义

三个 completed 用户回合、五次增量 200、五次独立接收与准确逐请求 accounting、两个完整准确的工具关联和实际 42\n 文件全部保留。使用真实 exec_command + write_stdin；后者的整数 session_id 来自前一次真实工具结果。历史恢复使用同一版本新 native PID 的 thread/resume；取消用 turn/interrupt 且同 PID 一直活到 release 观察结束。

外层 Seatbelt 实际阻止 IPv4/IPv6 非 loopback、真实用户目录读取、临时区外普通文件写入；TTY 例外只含内核终端设备。临时 HOME/CODEX_HOME/工作区与合成 key 不变。不得以 stop_process、进程退出或 server.stop 记作取消成功。

固定 Build profile 1.0.50 继续 BUILD_ACTIVATION_BLOCKED。#61 success companion 与费用仍独立，未知费用不填零。不调用 Provider/付费账号，不进行远程写入、发布或工单关闭。

## 来源

- Codex 固定 commit 979011409de0a60b52f179721948e65531d26144 `protocol/src/models.rs`，ResponseItem Reasoning 与 FunctionCallOutput 的可选 id/null 表示。
- 同 commit app-server-protocol 的 v2 SandboxPolicy / NetworkAccess / TurnStartParams，真实 externalSandbox、restricted、turnTrigger 与 turn/interrupt 入口。
- 本轮完整 Responses create 文档 SHA-256 9c4d505259ac72b879a3697f4f606d4131989ae686d204078b7b22e5638794d0；result id 是独立项身份。reasoning id 在该公开 schema 中仍 required，所以无 id history 明确是本地兼容范围，不能声称标准或固定服务资格。
