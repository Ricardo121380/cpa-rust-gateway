complete

PASS（限定为本次公开映射、公开声明值及公开文件字节比对）。

固定且当前 HEAD 均为 `14f694ee9483dce95b6a5305a9ece863c027578d`；下列路径均相对仓库 `/Volumes/Asgard/Agentprojects/CPA-Rust/repo/cpa-rust-gateway`，`P=docs/reports/assets/cpar-build-linux-final-20261013`。

| 公开文件 | 实际比较字段 | 条目数 | 证据 |
|---|---|---:|---|
| `P/prepared-freeze.json` | `freeze.files` | 17 | `P/prepared-freeze.json:9-27` |
| `P/upload.json` | `result.uploaded_hashes` | 17 | `P/upload.json:11-29` |
| `P/presend-binding.json` | `result.files` | 17 | `P/presend-binding.json:11-29` |

三份映射逐项键、SHA256 值完全一致，JSON 中的条目顺序也一致；所有 SHA 字符串长度均为 64。关键核验抓手：

- `gateway-aarch64-unknown-linux-gnu`：`cdadcb2cd679fe02e291f7a02fd6714aa92df816a91a5624858112ad19818b1e`，分别见 `prepared-freeze.json:24`、`upload.json:26`、`presend-binding.json:26`；另与 `presend-binding.json:30` 的 `result.binary_sha256` 相同。
- `artifact-manifest.json`：`655c037df68c5657b250105bbb07bed665210a7f6b20ea8d8b88aab488157a56`，分别见 `prepared-freeze.json:18`、`upload.json:20`、`presend-binding.json:20`；另与 `presend-binding.json:31` 的 `result.manifest_sha256` 相同。

公开 freeze 声明比对：

- `P/prepared-freeze.json:39`：`payload_freeze_sha256 = d00c82797519a31c5973717c0eb5c628d70a9078baa089248c75ac63fe3d5f61`。
- `P/presend-binding.json:10`：`result.payload_freeze_sha256 = d00c82797519a31c5973717c0eb5c628d70a9078baa089248c75ac63fe3d5f61`。
- `upload.json` **没有单独 freeze 字段**：已检查固定 HEAD 中整个 JSON，递归检索全部键名中包含 `freeze` 的字段，结果为空；范围为 `P/upload.json:1-33`。实际核到的是 `result.uploaded_hashes` 的全部 17 项，不能声称第三份存在 `payload_freeze_sha256`。

三个公开文件均以原始字节比较固定 HEAD blob 与当前磁盘文件，全部逐字节相同：

| 文件 | 字节数 | 固定 HEAD 与当前磁盘共同的文件 SHA256 |
|---|---:|---|
| `prepared-freeze.json` | 5578 | `bed5cd285c3985f60bd8320fe56cc2fac62bfd0783b26e56c0b251b806e020f7` |
| `upload.json` | 2325 | `1b300952736daf987c7f55d6621baae9c102255f6737d8a119de1ae906b1a53c` |
| `presend-binding.json` | 3826 | `f1f56926ccaf5780a267d1ee505dc36fc5a26ae536e27af12ca63b07678512e8` |

限制与存疑：以上 PASS 证明的是公开映射一致、两个实际存在的 freeze 声明值一致，以及这三个公开文件与固定 HEAD 字节一致。未读取或重新哈希 17 个真实 payload 文件，未产生新的远端、运行时或 Provider 证据。另曾对公开 `prepared-freeze.json` 的 `freeze` 对象执行无契约依据的 `json.dumps(freeze, sort_keys=True, separators=(',', ':')).encode('utf-8')`，所得 SHA256 为 `ab8983481ed453157734450372aaea47933565665d174d4cc51323a33e37a5f3`，不同于声明值；未读取驱动及实际 freeze 序列化契约，不能将该派生哈希视为原 payload 重算结果，也不能据此判定原 freeze 失败。
