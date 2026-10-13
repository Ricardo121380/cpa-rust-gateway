# GitHub 追加与独立读回

三次PATCH只修改既有进度评论；随后另行GET逐票核对正文/AC、状态、元数据、完整分页依赖及评论精确字节。

| 票 | 状态 | 既有评论 | 历史前缀 | 依赖 |
|---|---|---|---|---:|
| #47 | OPEN | [追加已核实](https://github.com/Ricardo121380/cpa-rust-gateway/issues/47#issuecomment-6081215242) | 精确保留 24298字符 | 8 |
| #57 | OPEN | [追加已核实](https://github.com/Ricardo121380/cpa-rust-gateway/issues/57#issuecomment-6081234278) | 精确保留 27999字符 | 23 |
| #9 | OPEN | [追加已核实](https://github.com/Ricardo121380/cpa-rust-gateway/issues/9#issuecomment-6081235490) | 精确保留 26738字符 | 0 |

新标记各只出现一次，评论各仍一条，正文/AC和全部受保护元数据及分页依赖保持。机器结果见[独立读回](github-readback.json)，[写入回执](github-write.json)，[发送前只读比较](github-preflight.json)。没有关闭三票、push、发布或部署。
