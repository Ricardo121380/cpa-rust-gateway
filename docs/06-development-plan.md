# Public development roadmap

This public roadmap intentionally contains no production hostnames, network topology,
account inventory, deployment receipts, migration records or rollback material.

## Current capabilities

- OpenAI Chat Completions, OpenAI Responses and Anthropic Messages protocol boundaries.
- Provider-isolated credentials, routing and upstream transport.
- JSON and bounded SSE projection for supported protocol/provider combinations.
- Versioned control-plane configuration, encrypted secret storage and audit records.
- Request, attempt and usage telemetry with protected management read models.
- Generated management client and administration interface.

## Open-source priorities

1. Stabilize public configuration examples and local-only quick-start workflows.
2. Expand protocol conformance and adversarial stream fixtures.
3. Improve provider adapter documentation and contribution boundaries.
4. Add versioned pricing and durable billing projections without exposing credentials.
5. Keep release, SBOM and supply-chain verification reproducible on public CI.

Production deployment history and operator-specific runbooks are maintained outside the
public repository.

# 公开开发路线图

本路线图不记录生产主机名、网络拓扑、账号清单、部署回执、迁移记录或回滚材料。

当前开源重点包括：完善本地配置示例、扩展协议一致性与对抗流测试、补强 Provider Adapter
文档、建设版本化价格与持久计费投影，以及保持公开 CI 中可复现的发布和供应链验证。
