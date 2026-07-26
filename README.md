# Prism · CPA-Rust Gateway 管理与观测面板

对标 CPAMP(seakee/CPA-Manager-Plus)的 Web 管理面板,视觉采用 Apple Liquid Glass。
纯浏览器 SPA(React 19 + TypeScript),最终以静态产物嵌入 cpa-rust-gateway 二进制,
经管理监听器同源服务。

- 设计契约:`cpa-rust-gateway/docs/07-management-frontend-design.md`
- 工程计划:`cpa-rust-gateway/docs/08-management-frontend-development-plan.md`
- 契约缺口与批准记录:`cpa-rust-gateway/docs/change-requests/CR-FE-001-management-frontend.md`
- 产品事实:[PRODUCT.md](PRODUCT.md)

## 与网关仓库的关系(唯一耦合点)

```text
cpa-rust-gateway/docs/openapi/management-v1.json   ← 契约唯一真源(后端会话维护)
        │  npm run sync-contract(复制 + 重新生成客户端)
        ▼
prism/contracts/management-v1.json → src/generated/management-client.ts
        │  npm run build
        ▼
prism/dist/  ──(FE-1 出口集成时)──▶  网关 include_bytes! 嵌入
```

后端契约变更后,本仓库执行一次 `npm run sync-contract` 即完成同步;
`npm run check` 会在客户端与契约失步时失败(机械保证,不靠自觉)。

## 命令

```bash
npm run dev          # 开发服务器(真实后端,需网关管理监听器可达)
VITE_PRISM_FIXTURES=1 npm run dev   # fixture 演示模式,无需后端
npm run test         # vitest(纯模型 + fixture 集成)
npm run build        # tsc strict + vite 构建(固定产物名,无哈希)
npm run check        # 安全不变量 + 产物清单 + CSP + 客户端新鲜度(--double-build 加双构建一致)
npm run sync-contract  # 从网关仓库拉取契约并重新生成客户端
npm run generate     # 仅重新生成客户端(契约已在本仓库)
```

## 安全不变量(scripts/check.mjs 机械强制)

秘密零浏览器存储 · 生成客户端是唯一 fetch 通道 · CSP 'self' 无内联 ·
产物清单与嵌入清单一致 · 双构建字节一致 · reveal-once 生命周期。
