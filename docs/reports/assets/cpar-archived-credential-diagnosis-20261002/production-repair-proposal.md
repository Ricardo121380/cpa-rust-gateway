# AC-01 生产单字段重新封装申请（未执行）

授权后进展：[生产实施及 fresh-copy 复验](../../cpar-archived-credential-production-repair-20261002.md) PASS。用户随后明确回复“授权”，本申请所限定的单字段方案已执行；下文是授权前提交的原方案，保留当时状态与审批问题，不代表当前仍待此项授权。部署授权仍独立。

状态：COPY_VALIDATED / PRODUCTION_NOT_EXECUTED，发布仍 BLOCKED。依据[主报告](../../cpar-archived-credential-aead-diagnosis-20261002.md)、[修复](repair-receipt.json)、[四组原生读取](native-cross-validation.json)与[迁移／CAS回退](migration-and-rollback-receipt.json)。本申请只涉及生产 AC-01 的 AAD 绑定修复，不包含部署、服务 stop/start/current 切换、账号重新授权、真实推理或关闭工单。

## 精确对象

主机 new-vps；数据库 `/var/lib/cpa-rust-gateway/control.sqlite3`；表 `upstream_credentials`；唯一目标由下面的 root-only locator 确定：

`/opt/cpa-rust-gateway/staging/cpar-execution-arm64-20261002-run36956561884/archived-credential-diagnosis-20261002/target.private.json`

它包含精确 config_version_id / credential_id / upstream_id 和原密文校验。公开只使用 AC-01。目标为 archived config revision0 内的 oauth_json / active credential revision2 / key_version1。可信来源是同库直接 parent 配置中的同 Credential／Upstream，kind/status/revision/key_version及原密文与目标相同，父 AAD 认证通过。生产和 capture 在2026-10-02 17:15:51 CST仍逐列相同；此观察不能替代操作时新核对。

## 前置条件

1. 用户明确授权这一生产数据写入；确认仍是 old b0cf36e3 / hash0b516c31 / schema31 / cpa-rust-gateway.service，unit/current/监听没有漂移。没有隐含部署授权。
2. 重新读取精确目标、直接 parent、当前 active 与 rollback predecessor；匹配全部身份、kind/status/revision/key_version、parent/lifecycle/config revision和原密文。目标仍不能被 active／current predecessor 引用；任何变化先停止重审。
3. 持久 Master Key 与服务实际 LoadCredential 匹配，其他五把配套 key 也与恢复集合对应；目标材料只在受保护本机进程处理。
4. 创建新的受保护完整、一致 pre-repair 集合：两库、必要WAL状态通过 Online Backup捕获、backups、6 key、old ELF、unit/current及ownership/mode。跨库/key稳定窗口检查通过，否则保留失败集合并停止；不使用10月2日02:47的旧capture覆盖生产。
5. 以该 fresh 集合重新验证父上下文成功、子上下文失败，以及恢复方案。保存精确目标原密文与新封装到独立 root0700／file0600目录，并保留 fresh完整集合。复核其他全部数据和 active 关系，禁止使用早期 bearer/revision0材料或另一个账号。

## 最小事务与验证形态

在受保护进程内，用 fresh 直接父记录的精确 AAD 解密同一材料，以 fresh nonce和**目标自己的 AAD**重新封装；不更改明文、key_version、credential revision/status/kind、configuration lifecycle/revision、关系、权限或历史事件。

SQLite `BEGIN IMMEDIATE` 后，**在同一事务内**再次核对目标及父、配置关系、版本状态和原封装预期值，再执行下面形态的绑定参数更新。所有原始标识、密文和key只在本机变量内，SQL错误不输出参数。

```sql
BEGIN IMMEDIATE;
-- 先核对直接 parent 和匹配的源材料仍与 fresh 操作基线相同。
UPDATE upstream_credentials
SET ciphertext = :fresh_resealed_envelope
WHERE config_version_id = :exact_config_id
  AND id = :exact_credential_id
  AND upstream_id = :exact_upstream_id
  AND ciphertext = :expected_original_envelope
  AND key_version = 1 AND revision = 2
  AND kind = 'oauth_json' AND status = 'active'
  AND EXISTS (
    SELECT 1 FROM config_versions v
    WHERE v.id = upstream_credentials.config_version_id
      AND v.status = 'archived' AND v.revision = 0
      AND v.parent_id = :exact_parent_id
  );
-- 变化必须恰好1行；在COMMIT前完成结构和精确加密认证断言。
COMMIT;
```

验证目标材料的 plaintext 与可信父材料恒等，目标 AAD 可认证，其他字段／记录未变；记录一个新的私有操作回执。回执包含校验和计数及原／新材料私有路径，公开仅显示脱敏结果。保留旧密文和既有审计，不重写历史日志。密文封装修正保持 Secret的逻辑身份和原revision，避免伪造历史凭据轮换。

提交后重新读取精确目标，获得 fresh post-repair 一致副本。在独立网络和状态目录用**当前旧 ELF**复验目标 metadata/export和归档 validate，以父输出相等及普通41/41/native5/5认证证明修复；候选80/hash542迁移后保留原列并读取同目标。调用均限副本，不在生产做export、validate或 Provider 请求。检查生产 active、账号／绑定／路由／权限和 current/服务保持；整个生产持久变更必须只有 AC-01 的 ciphertext 一列。运行中其他事务的真实漂移单独记录，不能声称整库字节冻结。

在线 SQL事务以5秒为获得 writer lock的上限，获得后设置10秒事务截止点，超时在提交前rollback；不计划服务停启。锁无法有界获得、其他writer导致一致捕获窗口无法成立或任何前置不满足，停止；不扩大成维护窗口或服务操作。若需要停服，应另行形成具体申请。

## 停止条件与回退

目标／父关联或原密文变化、status/revision/key_version/config lifecycle漂移、active/current predecessor引用变化、已加载key不符、备份不一致、父无法认证、目标不是确定上下文异常、update命中非1行、原生读取／迁移／完整性／数据保持失败、来源明文不同：停止，不重试凑通过。

事务提交前失败直接 ROLLBACK。提交后需要撤销时，只在同一目标身份和配置保护条件下，以**当前密文等于本次修复密文**为 CAS预期，把保留原ciphertext写回，恰好1行才提交；再次检查所有其他列和关系。该回退已在副本验证可恢复54张原表的逻辑基线；它会恢复已知历史异常，因此发布回到 BLOCKED。若目标已被别人改变，不覆盖，保留现场并停止请求协调。

不直接整库覆盖运行生产，避免丢失期间的续期、管理或请求／账本写入。完整 pre-repair集合留作灾难恢复依据；若单字段回退不可用而需要整套恢复或服务操作，另行申请具体数据窗口及停启授权。

## 需要用户决定的一项

是否授权在上述前置与停止条件成立时，执行 **new-vps 上 AC-01 这一行 ciphertext 的生产重新封装、fresh-copy原生复验和有界单字段回退**？本轮尚未执行。权限门槛来自用户本轮明确排除“生产数据写入”的边界，而非工具或Skill额外增加的确认。完成生产处置后仍需重新判断发布门槛，部署另行授权。
