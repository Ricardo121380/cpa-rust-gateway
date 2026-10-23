# CPAR-66 原样交付 OCI 本地运行资格（2026-10-23）

运行结论：**PASS_LOCAL_OCI**。交付的原样 Linux ARM64 OCI 在本轮独占、本地、外部网络阻断条件下，通过真实 serve、reader、同容器再启动、新容器状态采用及正常退出；全部目标容器、控制器、私密 volume 和专属 VM 已释放。产品、Dockerfile、协议准入、固定 Build 服务资格及旧制品 revision 均未改变。#66 仍为 OPEN，未开始 #67。

## 1. 实际基线与候选分离

- 优先工作树：`/Volumes/Asgard/Agentprojects/CPA-Rust/worktrees/cpar62-client-metadata-20261020`，分支 `feature/62-client-metadata`。开工实际 HEAD `44bada2c70f910d758e56e7e4820af234fa0a8a9`；index 为空。
- 基线逐文件记录 7,438 项 bytes/type/mode/link/uid/gid；18 项继承 Git 状态与 15 项独立 0600 权限差异保留。历史 SHA 只定位证据，未 reset、clean、stash、覆盖旧产物或整体暂存。
- 被测制品源码继续为 **`737fd040e983bef329be27b5cf48b4f6986a5772` + 上轮 overlay**，schema38，完整六文件 unsigned set 位于 `/Volumes/Asgard/Agentprojects/CPA-Rust/output/cpar66-local-candidate-20261022/unsigned-artifact`。原制品未重建。
- 本轮验收代码为 **`f8d28c6b`**；仅改 3 个验收脚本。新入口 `scripts/acceptance/cpar66-oci.py` 只提供本任务 Docker 执行和观察，复用 `cpar66-reader-serve.py` 的业务 oracle；后者只提取 reader 执行与剩余进程检查的两个小入口。未建设多后端 runner。
- Full 在 Darwin 27.0.0 arm64 使用独立工作树与全新 target，输入为冻结的 **7,440 项实际文件**，包括继承 overlay 与权限；它和 Linux 产品制品源码分开绑定。后续报告 commit 不改变两者的受测身份。

亲读了适用 AGENTS、CONTEXT、交付与质量门禁、Dockerfile、P12 与 reader 契约、[上轮报告](cpar-66-local-candidate-20261022.md)及 candidate-binding/artifact-manifest/linux-run-command/OCI-binding/verification-reuse。Dockerfile、共享工作流及跨前后端共享路径均无本轮 diff，不触发 cross-boundary 变更记录。

GitHub 只读快照：[#66](https://github.com/Ricardo121380/cpa-rust-gateway/issues/66) OPEN，原生 `blocked_by=[62,64,65]`，均 OPEN，`blocking=[67]`，#61/#67 也为 OPEN。没有远程写入或解除依赖。

## 2. 完整镜像身份链

[image-binding.json](assets/cpar66-oci-runtime-20261023/image-binding.json) 重新核对六文件集合、五个 payload 的 bytes/SHA、OCI index/manifest/config、四层压缩 bytes/digest 与完整解压 tar 的 diff_id；再核对 Docker archive 全部共同成员及桥接索引。两个归档的共同成员逐字节相同，Docker 仅多 `manifest.json`，其 Config 和有序 Layers 指向同一完整镜像。不是仅比较 gateway 文件。

| 对象 | 固定身份 |
|---|---|
| artifact manifest SHA256 | `aff99e88cb24a7cf3f1489f0a712c3e37e83af76e29bb5768a9a552fcd255e0c` |
| binary SHA256 | `17c1fd546d1a8fbb7d310bc3f62a872f23b9d1b5b3f8195e383fc9f86162bdf2` |
| OCI archive SHA256 | `e4503dca17842ba3869758ac5e914a13be43c23428ff0cc7981dde2d07385f94` |
| Docker archive SHA256 | `3d630e8f146cf5bb8916365c262bc5be629a93e9c0f8878e063445393ad45e65` |
| OCI manifest digest / 本机 Docker 实际 image ID | `sha256:5a1b901db4ecdd31cccca59093a7fbcef53414fe9bc3c4205997ab6ce61214f9` |
| OCI config digest | `sha256:a8a678ffbc333d47a27f32f234578b6c04c75ad4e59f970f57f334f6de983f2a` |
| 实际执行文件 | `/usr/local/bin/gateway`；层内 bytes 与目标 `/proc/PID/exe` SHA 均等于交付 binary |
| 运行配置 | linux/arm64；User `65532:65532`；Entrypoint `["/usr/local/bin/gateway"]`；WorkingDir `/nonexistent` |
| revision / Rust / target | `737fd040e983bef329be27b5cf48b4f6986a5772` / `1.97.1` / `aarch64-unknown-linux-gnu` |
| 原 base label | `debian:bookworm-slim@sha256:9b67294679b30e5d6ab257b40594feeb4a4b81f7fcf4131f4decf0d6a212a9b0` |

Docker 29.2.1 / API1.53 的当前镜像存储实际返回 manifest digest 作为 image ID；config digest 独立记录，二者没有混称。启动只用完整固定 image ID，没有依赖 tag。载入后完整 Config 与 RootFS 有序层再次核对，见 [loaded-image.json](assets/cpar66-oci-runtime-20261023/loaded-image.json)。原 P12 严格 unsigned verifier 再次 PASS；原 candidate-binding 的 source/build/assembly/environment/artifact/runtime/verification 共 **56 项**也逐项重算一致。

## 3. 原样镜像的实际运行

最终运行 [run.json](assets/cpar66-oci-runtime-20261023/run.json) 为 **PASS_LOCAL_OCI**；[runtime-qualification.json](assets/cpar66-oci-runtime-20261023/runtime-qualification.json) 为 **290/290**。内部先记录 `PASS_RUNTIME_PENDING_CONTROLLER_CLEANUP`，外层只有控制器退出、容器/volume 删除并复核后才能提升为最终 PASS；环境释放另有回执，未改写内部阶段状态。

本轮独占 Colima `cpar66-oci-20261023` 为原生 VZ/aarch64，4 CPU、6 GiB RAM、24 GiB disk，Ubuntu 24.04.4 LTS / kernel `6.8.0-100-generic`，`activate=false`、`ssh-config=false`、`ssh-agent=false`。复用上轮已导出的固定 helper image `sha256:eca0fa3b1a319ee5ddaab9a3804af488536c51fe4125506cb1b8bc0039003aea`。helper 内 Python 只做控制、fixture 与观察，**没有在 helper 中执行 gateway**。

目标容器由默认 ENTRYPOINT 启动 gateway；没有 `--user`、`--workdir` 或 entrypoint 覆盖，也没有挂载替代 binary/Python/firewall 工具。目标 rootfs 只读、capabilities 全清、NoNewPrivs=1、PID namespace 独立；必要的本轮 state、credentials、keys、私密输出及错误 key 负例目录分别以 volume subpath 显式挂载。credential/key 为只读。目录0700，文件0600；fixture、key、DB 和 plaintext 全在本轮外部私密目录，未进入 Git 或公开报告。

辅助控制器持有 task Engine socket、host PID 观察及 NET_ADMIN/CHOWN/DAC_OVERRIDE/FOWNER/SYS_PTRACE 权限。这些只属于控制器；目标 gateway 的结论来自 Docker `State.Pid` 对应的实际 `/proc/PID/exe/status/cwd/mountinfo/ns/net/route`，不能用控制器权限替代。

| 检查 | 实际结果 |
|---|---|
| 全新状态 | schema38，readiness 200，HEAD health；初始控制面为空 |
| 两监听器与鉴权 | data 无 admin route；admin 未鉴权/错误 key 均404，正确 key 可读；data models 未鉴权401 |
| OS 与文件边界 | 实际 UID/GID 四列均65532，CapEff/CapPrm/CapBnd=0，NoNewPrivs=1；exe/hash、默认 cwd、rootfs ro 与 credential/key ro 均通过 |
| metadata reader | v1/v2 完整 raw/source/owner/request/实际 Attempt 准确；错误 owner/request/Attempt 均 exact UNKNOWN，错误 key 非零且无输出 |
| Usage / summary reader | Usage 原始数值表示与身份、summary 完整有序内容与执行身份准确；同样覆盖错误身份/key |
| reader 输出 | 22 个场景，另6次覆盖拒绝，共 **28 个实际 reader 容器**；0600及实际 UID/GID65532；原值保留，覆盖不改变旧输出 |
| stdout/stderr 与普通日志 | 成功只允许固定无值确认行；失败只允许既有通用无值错误。普通日志无私密 sentinel |
| 网络 | 目标共享本轮 `network=none` 的受控 namespace，实际无默认路由；IPv4/IPv6 firewall 拒绝其余通信。目标自身的 bash socket 探针证明外部 IPv4/IPv6和活跃无关loopback被拒，受控接收端可达 |
| 接收数 | 仅 `GET /cpar66-envelope-probe` 一次；无 inference、后台 Provider 或真实客户端请求 |

实际 server/reader 容器的 create 参数、默认配置、退出状态以及所有关键检查均在 runtime 回执中。reader 成功输出的实际属主与权限在控制器观察到的新文件上核对；没有把控制器 root 身份计作 reader 身份。

## 4. 容器重启、状态采用、退出和 cleanup

共 **34 个目标容器**：5 个 serve、28 个 reader、1 个空 state 只读负例。一个 serve 容器正常停止后再次启动，因此形成6次 serve进程生命周期。

| 场景 | container ID 前缀 | 实际 VM PID / start_time | 退出 |
|---|---|---|---|
| 全新 state | `c6af3b4ab388` | `8971 / 87430` | SIGTERM，0 |
| metadata fixture采用 | `656a74c33158` | `9165 / 87677` | SIGTERM，0 |
| 同一容器再次启动 | `656a74c33158` | `10036 / 88098` | SIGTERM，0 |
| 新容器采用同一 state/key | `9a285d002b52` | `10120 / 88168` | SIGTERM，0 |
| Usage fixture采用 | `d5988c89cbff` | `10457 / 88343` | SIGTERM，0 |
| summary fixture采用 | `69435307574e` | `10939 / 88727` | SIGTERM，0 |

同容器重启与新容器采用分别核对 container/PID/start_time。metadata marker、schema38、三个 encrypted evidence 表的行摘要和授权 v1/v2 读回保持；只读回既有合成证据，不发请求、不重新生成 Provider 数据。fixture切换只发生于前一 serve 正常退出后。

所有6次正常停止均由真实 Docker stop 发送 SIGTERM，保存的 Engine event 为 signal15，无signal9；退出0，无OOM，约0.359–0.438秒，无45秒超时或强制kill。逐次检查旧PID不存在、19180/19181释放、SQLite WAL checkpoint正常且DB可再次采用。

运行结束前 receiver shutdown/server_close/join、19443释放、所有目标容器退出及删除均通过。控制器退出0后，宿主入口取回私密运行材料，再删除控制器和私密volume并独立inspect确认不存在。随后确认 Engine 无容器/volume，停止并删除本轮VM/data/profile，socket与agent PID消失；宿主19180/19181/19443/19999无监听。三个既有profile继续Stopped，全局Docker context仍default。见 [environment-cleanup.json](assets/cpar66-oci-runtime-20261023/environment-cleanup.json)。

## 5. 失败、最小负例与验证

没有暴露需要修改产品或 Dockerfile 的真实失败。所有开发失败和旧 RED 保留在外部output，没有覆盖为最终绿灯：

| 记录 | 分类与处理 |
|---|---|
| `original-01` | 控制器fixture改属65532后chmod缺FOWNER；gateway尚未启动。补辅助容器权限，目标capability不变 |
| `original-02` | 所有已有业务与退出检查通过，但“现存schema最新DB以ro挂载必定启动失败”的负例假设错误，等待退出超时，整轮仍为FAIL。保留真实结果，不归为产品缺陷 |
| `original-03` | 将负例精确定为“空state只读，初始化不可写”；原样镜像289/289及完整容器cleanup通过 |
| `final-f8d28c6b` | 增加显式新容器/进程身份断言后的同一原样镜像，最终290/290及环境cleanup通过 |
| 环境准备 | VM尚未就绪的socket访问、未安装的Ruby路径均留原日志；实际环境就绪后使用既有工具，没有扩大产品范围 |

定向5项测试通过：完整归档正例；Docker桥接错误Config引用/反转layer顺序拒绝；错误image绑定在任何Engine访问前拒绝；target cleanup抛错及receiver cleanup抛错均整体FAIL并保存回执。运行负例另包含真实错误owner/request/Attempt/key、输出覆盖拒绝、只读空state启动非零和目标网络/写入边界。没有放宽oracle、增大超时、追加Provider请求或改坏产品来制造RED。

Standards/Spec两轴只读审查分别 **PASS，0 finding**，见 [Standards](assets/cpar66-oci-runtime-20261023/review-standards.md) / [Spec](assets/cpar66-oci-runtime-20261023/review-spec.md)。审查绑定 `44bada2c...f8d28c6b` 和实际公开回执，不替代Full。

本轮新 [Full](assets/cpar66-oci-runtime-20261023/full-check.md) **45/45 PASS**，Rust **1,659 passed / 0 failed / 21 ignored**；[full-summary.json](assets/cpar66-oci-runtime-20261023/full-summary.json) 记录 `f8d28c6b0f8f66e60cbc1359ea5aaf8fa896d920`、实际overlay/input、全新target和精确命令。Node 24.19.0、Rust1.97.1及固定质量工具使用既有安装，不更改全局Node26。Full结束后7,440项source再次verify通过。

本轮dependency policy和RustSec advisory audit均正常退出0，未观察到本轮yanked timeout提示；没有独立追加yanked资格实验，旧轮 **`UNKNOWN_TIMEOUT` 历史状态原样保留**，不改称旧未知已获证明。旧Full仍属于其原受测源码，与本轮新Full分列。

最终文档 `scripts/check.sh docs` **7/7 PASS**，另做本轮staged文件Secret scan及diff whitespace检查；见 [docs-check.md](assets/cpar66-oci-runtime-20261023/docs-check.md)、[preservation-final.json](assets/cpar66-oci-runtime-20261023/preservation-final.json)。本轮只提交所属脚本与新报告/回执，继承内容不入本轮提交。

## 6. 可执行本地入口与准确启动示例

外部最终output：`/Volumes/Asgard/Agentprojects/CPA-Rust/output/cpar66-oci-runtime-20261023`。归档、原失败、完整私密运行材料、source archive/input manifest与原始日志都在这里。仓库只包含允许公开的回执和报告。[candidate-binding.json](assets/cpar66-oci-runtime-20261023/candidate-binding.json) 分别绑定产品artifact source/manifest、本轮验收脚本、环境、运行与Full；[targeted-validation.json](assets/cpar66-oci-runtime-20261023/targeted-validation.json) 保存定向检查命令与结果。

下例在当前Mac创建全新独占ARM64环境，复用已有归档与依赖；要求Colima/Docker/Python可用、旧制品目录完整且有合成fixtures。`run`输出必须不存在；入口自动复制fixture到新0700输出，所建volume及容器仅使用随机独占名称，PASS/FAIL后均执行清理。VM由示例的trap释放，输出证据保留：

```bash
set -euo pipefail
CPAR66_OLD=/Volumes/Asgard/Agentprojects/CPA-Rust/output/cpar66-local-candidate-20261022
CPAR66_RUN=$(mktemp -d /Volumes/Asgard/Agentprojects/CPA-Rust/output/cpar66-oci-recheck.XXXXXX)
CPAR66_PROFILE="cpar66-oci-$(date +%Y%m%d%H%M%S)"
test ! -e "/Volumes/Asgard/Colima/$CPAR66_PROFILE"
trap 'colima stop "$CPAR66_PROFILE"; colima delete "$CPAR66_PROFILE" --force --data' EXIT
colima start "$CPAR66_PROFILE" --activate=false --template=false \
  --ssh-config=false --ssh-agent=false --arch aarch64 --vm-type vz \
  --cpu 4 --memory 6 --disk 24 --mount "$CPAR66_RUN:w" \
  --mount "$CPAR66_OLD:r" --mount-type virtiofs
CPAR66_ENGINE="unix:///Volumes/Asgard/Colima/$CPAR66_PROFILE/docker.sock"
docker --host "$CPAR66_ENGINE" load --input "$CPAR66_OLD/builder-image.docker.tar"
python3 /Volumes/Asgard/Agentprojects/CPA-Rust/worktrees/cpar62-client-metadata-20261020/scripts/acceptance/cpar66-oci.py \
  --docker-host "$CPAR66_ENGINE" \
  --artifact "$CPAR66_OLD/unsigned-artifact" \
  --docker-archive "$CPAR66_OLD/gateway-image.docker.tar" \
  --fixtures "$CPAR66_OLD/fixtures-final" \
  --image sha256:5a1b901db4ecdd31cccca59093a7fbcef53414fe9bc3c4205997ab6ce61214f9 \
  --helper-image sha256:eca0fa3b1a319ee5ddaab9a3804af488536c51fe4125506cb1b8bc0039003aea \
  --output "$CPAR66_RUN/run"
```

实际入口通过Engine API create/start/stop/wait执行目标容器；下方Docker CLI形状与已记录的serve create参数一致。它需要**本入口已创建的受控network namespace及volume**：`CPAR66_NETNS`是控制器名称，`CPAR66_WORK_VOLUME`中已有`run/state`、`credentials`、`keys`、`private`、`wrong-keys`，目录0700/文件0600并属65532。不能直接指向用户真实state或凭据，不能省略network anchor/firewall前置条件。

```bash
docker --host "$CPAR66_ENGINE" run --name "$CPAR66_GATEWAY_NAME" --pull=never \
  --network "container:$CPAR66_NETNS" --read-only --cap-drop ALL \
  --security-opt no-new-privileges --pids-limit 128 \
  --mount "type=volume,source=$CPAR66_WORK_VOLUME,target=/state,volume-subpath=run/state" \
  --mount "type=volume,source=$CPAR66_WORK_VOLUME,target=/credentials,volume-subpath=run/credentials,readonly" \
  --mount "type=volume,source=$CPAR66_WORK_VOLUME,target=/keys,volume-subpath=run/keys,readonly" \
  --mount "type=volume,source=$CPAR66_WORK_VOLUME,target=/private,volume-subpath=run/private" \
  --mount "type=volume,source=$CPAR66_WORK_VOLUME,target=/wrong-keys,volume-subpath=run/wrong-keys,readonly" \
  sha256:5a1b901db4ecdd31cccca59093a7fbcef53414fe9bc3c4205997ab6ce61214f9 \
  serve --state-dir /state --credential-dir /credentials --master-key-dir /keys \
  --active-master-key-version 1 --data-listen 127.0.0.1:19180 \
  --management-listen 127.0.0.1:19181 --background-provider-tasks disabled
```

state只可由一个serve写入。正常stop并确认退出后，同一volume/key才可交给下一容器；reader使用同一镜像默认入口，state只读，output仍是私密可写挂载。entrypoint、user、cwd保持镜像默认值。受控loopback未映射宿主端口。

## 7. 各层证据与保留资格

| 范围 | 状态与边界 |
|---|---|
| 原完整unsigned ARM64制品 | 上轮PASS，本轮strict verifier与完整manifest/原绑定56项再核对PASS；没有新build |
| builder内stock binary资格 | 历史 **144/144 PASS_BOUNDED**，仍只属于该userspace/进程环境；未冒充OCI系统资格 |
| 本轮原样OCI | **PASS_LOCAL_OCI，290/290**；真实serve/reader/同容器再启动/新容器采用/SIGTERM/cleanup |
| 三客户端历史受控证据 | 仅按上轮verification-reuse记为历史REUSED；本binary/本OCI的真实客户端执行 **NOT_RUN**；原样Claude仍有既有阻断，D2不是本轮选择 |
| 本轮验收源码Full | **45/45 PASS**，Rust **1659/0/21**；Darwin arm64独立源/新target；不是把旧Full改称当前SHA重跑 |
| D2首版采用 | **NOT_DECIDED**；原样Claude控制承诺保留 |
| #61 `cost_in_usd_ticks` | **NOT_ADMITTED**；继续等待一手依据 |
| 固定Build服务资格、#66整票 | **BLOCKED / OPEN**，#62/#64/#65未解除 |
| #67、真实客户端/Provider/凭据 | **NOT_RUN**，未开始、未读取 |
| 同输入两份独立可重复构建 | **NOT_RUN**；仅TRACEABLE，不因archive存在就宣称reproducible |
| 远端CI、Sigstore签名、密码学验签 | 分别 **NOT_RUN**；本地hash不是签名真实性 |
| push、GitHub写入、正式版本、发布/部署、旧远端环境 | 全部 **NOT_RUN** |
