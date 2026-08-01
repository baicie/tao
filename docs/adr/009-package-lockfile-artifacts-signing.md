# ADR-009：包、Lockfile、组件产物与签名

* **状态**：Accepted
* **日期**：2026-08-01
* **目标版本**：Nexa Language 2.0 工具链与生态
* **关联 ADR**：

  * ADR-000：Nexa 编译器自举路线
  * ADR-001：Nexa 2.0 语言架构、所有权与运行时模型
  * ADR-003：NIR、LLVM 后端边界与稳定 ABI
  * ADR-005：Host ABI、Capability 与资源 Handle
  * ADR-006：Result、Panic、defer 与跨 ABI 错误模型
  * ADR-007：Async 状态机、Structured Concurrency 与并发安全
  * ADR-008：WebAssembly、JavaScript FFI 与跨平台 UI Host
* **实现优先级**：P1
* **兼容性范围**：不修改 Nexa Language 1.0 已冻结语义或当前 Rust/Cargo workspace 发布方式

---

## 1. 背景

Nexa Language 1.0 只交付了 Rust workspace 中的自用 `nexac 0.0.1`，没有承诺 Nexa package registry、
稳定 compiler artifact 或组件分发格式。Nexa 2.0 引入 Native/Wasm、Host capability 和 stable component ABI 后，
必须明确源码包、编译缓存、可执行产物与可分发组件的边界。

如果为了复用编译缓存而公开 NIR/MIR，内部优化格式会被永久冻结；如果 lockfile 不固定 source、digest、
feature、target 与 signature，离线或 CI 构建可能解析出不同依赖；如果签名不覆盖 capability 和 ABI metadata，
攻击者可以替换 payload 或扩大权限声明。

本 ADR 冻结解析、内容寻址、产物分类、签名与可复现构建合同，而不要求 Nexa 2.0 第一日就运营公共 registry。

---

## 2. 决策摘要

1. `nexa.toml` 是 source package manifest；`nexa.lock` 是规范化、可提交的解析结果。
2. 包身份由明确 source namespace、规范 package name 和 semantic version 组成，不按本地目录猜测。
3. 依赖 source 仅为 workspace/path、固定 Git commit 或 registry；不存在 Node-style fallback resolution。
4. 同一解析图中每个 package identity 只选择一个版本，冲突直接报告。
5. Application、binary 与发布构建必须使用 lockfile；published library 仍以 manifest range 表达兼容性。
6. Lockfile 固定完整 graph、source、version/commit、content digest、features、target condition、capability、unsafe 与 signature identity。
7. Source package、private compiler cache、runnable artifact 与 stable component artifact 是四种不同格式。
8. HIR、MIR、NIR、LLVM IR 和增量缓存永远不是稳定分发格式。
9. Stable component 只暴露 versioned descriptors、imports/exports、capabilities、ABI hashes 和 target payload。
10. Registry version 与其 source archive 内容不可变；撤回只标记 yanked，不替换 bytes。
11. 所有 digest 和 signature 都携带 algorithm ID；首个必需 digest 为 SHA-256，首个 package signature 为 Ed25519。
12. 签名覆盖规范 manifest、payload tree root、package identity、ABI/capability metadata 与 provenance reference。
13. 签名有效不等于受信任；信任由 registry root、publisher delegation、revocation 与本地 policy 决定。
14. 发布构建必须可复现：相同锁定输入、工具链、target/profile 产生相同 logical artifact digest。
15. 构建脚本默认禁止；显式脚本在 capability sandbox 中运行，输入和输出进入 build hash。
16. Loader 在执行 component 前验证 digest、signature、target、Host ABI、capability、panic/OOM 与 resource limits。

---

## 3. 包与模块边界

一个 source package 是带有一个 root manifest 的源文件集合。包可以包含多个 module 与显式 target：

```text
package
  -> library target
  -> binary targets
  -> test/bench targets
  -> optional component targets
```

相对 module import 仍服从 Language 1.0/后续 Edition 的确定规则；package import 只从 manifest dependency
alias 解析。编译器不会：

* 搜索父目录的随机 `node_modules`
* 自动尝试扩展名或 `index` 文件
* 从当前工作目录猜测 registry package
* 在 registry 失败后回退到同名 Git/path package
* 访问 manifest 未声明的 URL

Manifest 与 lockfile 解析属于 package/tooling phase，不进入 parser、HIR 或 MIR 的动态查找路径。

---

## 4. Package identity 与版本

Registry package 的规范身份为：

```text
registry-origin :: namespace/name @ semver
```

规则：

* `registry-origin` 使用配置中绑定的 registry ID，不只依赖可混淆显示 URL。
* namespace/name 采用规范 lowercase ASCII、`-` 分隔和长度限制。
* Unicode display name 可以存在于 metadata，但不参与 import identity。
* Semantic Versioning 决定 package API compatibility；Edition 独立记录语言语义版本。
* 已发布 `(origin, name, version)` 永不覆盖。
* Yank 阻止新解析选择，但已有 lockfile 仍可按 pinned digest 获取。

Git dependency identity 包含规范 remote ID 与完整 commit object ID；branch/tag 只能作为更新提示，不能进入锁定结果。
Path/workspace dependency 使用 manifest 声明的 package name/version，并在 lockfile 中记录 workspace-relative path 与 source tree digest。

发布 artifact 不能依赖未快照的外部绝对 path。

---

## 5. Manifest

`nexa.toml` 至少描述：

```text
[package]
name, version, edition, minimum_toolchain, license

[targets]
kind, root, crate-type/component-type

[dependencies]
alias -> package, version requirement, source, features, target condition

[features]
additive feature graph

[capabilities]
maximum requested Host capabilities and scopes

[build]
profile constraints, panic/OOM policy, optional sandboxed script

[security]
unsafe declaration, publisher/signing policy, allowed registries
```

Manifest 规则：

* dependency alias 与真实 package identity 分离，重命名不能制造第二身份。
* feature 只能加能力/代码，不能通过互斥负特性静默改变同名 API。
* target condition 使用固定表达式语言，不执行任意脚本。
* capability 是最大请求，不是运行授权。
* unsafe 与 build script 使用必须显式、可查询，并传播到审计 metadata。
* unknown required field 或 future manifest major 被旧工具链拒绝。

Secret、access token 和私钥禁止写入 manifest；registry credential 由独立 credential store 管理。

---

## 6. 解析规则

Resolver 对一个 workspace、target set 和 feature request 生成单一版本 graph。

确定规则：

1. 读取 workspace manifests，并验证 package identity 唯一。
2. 规范化 dependency source，不做 fallback。
3. 合并同一 package identity 的 version constraints。
4. 选择满足约束的最高非 yanked version，除非 lockfile 已固定兼容 version。
5. 对选中 package 合并 additive features。
6. 展开 target-conditioned edges，并保留所有声明 condition 供跨 target lockfile 使用。
7. 验证 capability/unsafe/build-script policy。
8. 输出稳定排序 graph 或最小冲突说明。

同一 graph 不并存同一 package identity 的多个 major/minor。如果约束无交集，resolver 报告依赖链，
要求升级、降级或显式 fork/rename package identity，而不是静默复制类型身份。

Resolver version 写入 lockfile。算法改变需要新的 resolver version，不能让旧 lockfile 在相同工具链上静默漂移。

---

## 7. Lockfile

`nexa.lock` 是 machine-generated UTF-8 TOML。规范 writer 固定 section、package node、edge、feature 与 key 排序；
语义 digest 基于规范数据模型，不依赖用户空白或注释。

Header 包含：

```text
lock_version
resolver_version
workspace_manifest_digest
generated_by_toolchain
registry_snapshot_ids
```

每个 node 至少包含：

```text
package_instance_id
source_kind and canonical source identity
name, version or git commit
source_tree_digest
dependencies with target conditions
resolved_features
declared_capabilities
unsafe/build-script flags
publisher/signature identities
license/provenance references
```

规则：

* Application/binary lockfile 应提交版本控制。
* `--locked` 在 manifest 与 lockfile 不一致时失败，不更新文件。
* `--offline` 只使用已验证 cache 与锁定 registry snapshot；缺失对象直接失败。
* `--frozen` 等价于 locked + offline + 禁止网络/写 lockfile。
* 发布 library 不把自己的开发 lockfile 强加给下游，但发布验证保存其构建 attestation。
* target 选择不能删除 lockfile 中其他条件 edge，避免跨平台反复改写。
* 手工修改后仍必须通过 schema、graph、digest 与 signature 验证。

Lockfile 不包含 registry token、私钥、绝对 workspace path 或不可复现时间戳。

---

## 8. 内容寻址与 source archive

Source package archive 的逻辑格式为 `.nexpkg`，包含：

* canonical `nexa.toml`
* package source 与声明的 resource
* license/notice
* normalized file manifest
* algorithm-qualified file digests 与 tree root
* signature envelope/provenance reference

规范化要求：

* path 使用 UTF-8、`/`、相对路径和唯一规范形式。
* 拒绝 `..` 逃逸、absolute path、NUL、重复 path 和 case-fold collision。
* symlink 默认禁止；未来允许时必须在 archive 内且 target 参与 hash。
* file mode 只保留规范 executable bit；owner/group 不进入内容。
* archive timestamp 归零；条目按 path byte order 排序。
* 解压大小、文件数、单文件大小和压缩比受预算限制。

Digest 校验在解析源文件或运行 build script 前完成。Cache 以 digest 寻址，读取时重新验证，不信任文件名。

---

## 9. 四类产物

### 9.1 Source package

`.nexpkg` 是可发布、可签名的规范源码集合，不包含被信任的预编译代码。

### 9.2 Private compiler cache

可以包含 CST/HIR/MIR/NIR、object、incremental query 和 LLVM bitcode。Cache key 必须包含完整 build fingerprint；
格式随工具链变化，可以随时删除，不能作为稳定依赖或 registry 公共组件。

### 9.3 Runnable artifact

面向应用部署的 executable、library bundle 或 Wasm module。它绑定 target/profile，包含 runtime 与 Host requirements，
但不自动形成供第三方链接的稳定 Nexa ABI。

### 9.4 Stable component artifact

逻辑扩展名 `.nexc`，包含：

* component format/version
* package identity 与 source digest
* target/architecture/runtime profile
* exported/imported stable descriptors
* ABI、layout 与 Host module hashes
* required/delegated capabilities
* panic/OOM/async/threading policy
* typed Native/Wasm payloads
* resource limits
* provenance 与 signature envelope

Stable component 不包含供消费者解释的 MIR/NIR。多 target bundle 可以封装多个独立 payload，
每个 payload 都有 digest/metadata，外层 signature 覆盖完整选择表。

---

## 10. Component 加载

Loader 使用固定顺序：

```text
parse bounded container
-> verify tree digests and signature envelope
-> evaluate trust/revocation policy
-> select exact compatible target payload
-> validate component/Host ABI versions and hashes
-> compute effective capabilities
-> validate panic/OOM/async/threading/resource policy
-> instantiate without user code
-> invoke explicit entry point
```

不能先运行 component initializer 再发现 capability 或 ABI 不匹配。Target 选择必须比较完整 triple、CPU features、
Wasm model、runtime ABI 与 profile；禁止仅按文件扩展名或 host OS 猜测。

Component imports 只能绑定到声明的 stable component/Host IDs。普通 Native Nexa symbol、Rust ABI、NIR ID
或未版本化 C symbol 不满足 stable component dependency。

---

## 11. Build fingerprint 与可复现性

Build fingerprint 至少包含：

* source package tree digest
* complete resolved dependency graph digest
* compiler release identity 与 bootstrap stage provenance
* language edition、target、CPU features 和 profile
* resolved features
* panic、OOM、async/threading 与 runtime module selection
* Host/component ABI schemas/hashes
* declared build inputs、sandbox policy 与 script output digests
* reproducibility format version

不得包含：

* workspace absolute path
* wall-clock build time
* random iteration order
* locale/timezone-dependent output
* undeclared environment variable
* registry credential

相同 fingerprint 必须产生相同 logical sections 和 artifact digest。签名时间、transparency receipt 或镜像封装
可以作为不影响 logical payload digest 的外层 metadata。

Debug information 中的 path 使用 remap table；object/archive member 排序、symbol metadata 和 compression
必须确定。Stage 构建按 ADR-000 比较规范化 artifact，而不是含签名时间的传输 envelope。

---

## 12. Build script 与生成代码

任意安装脚本默认禁止。确需 build script 时：

* manifest 明确声明脚本 target、解释器/tool digest 和 capabilities。
* 在独立 sandbox 运行，默认无网络、无环境、只读 source，只写 build output。
* 文件、env、tool、Host input 和 output 都进入 build fingerprint。
* 输出只能写声明目录，并经过 path、size、type 和 digest 验证。
* script 不能修改 package source、lockfile、dependency cache 或 credential store。
* 用户/policy 可以拒绝所有脚本或要求从源码重建。

生成代码在进入 compiler 前作为普通 source input hash；生成器不能向 HIR/MIR 注入未审计对象。

---

## 13. Digest、签名与 trust

所有 digest 表示为：

```text
algorithm-id : lowercase-hex-digest
```

首个 required algorithm 是 SHA-256。格式支持未来增加算法，但未知 required algorithm 必须拒绝，
不能降级到更弱或不受 policy 接受的算法。

首个 package/component signature 是 Ed25519。实现必须使用审计过的 crypto library，禁止自写签名算法。

Signature envelope 覆盖规范 statement：

```text
subject package/component identity and version
source/payload tree root
manifest digest
dependency lock/build fingerprint digest
target payload table
ABI and capability metadata digest
provenance/attestation digest
signature format version
```

Envelope 不直接签任意 JSON/TOML 字节；先按固定 schema canonicalize，防止字段顺序和重复 key 歧义。

Trust evaluation 独立于 cryptographic validity：

* Registry root 授权 namespace/publisher key。
* Publisher delegation 可以限制 package、version range 和 expiry。
* Root/key rotation 与 revocation 使用版本化、签名 registry metadata。
* Offline build 使用 lockfile 固定的 registry snapshot 和本地 trust root。
* 被撤销 key 的历史 artifact 是否可用由 timestamp/policy 决定，不只看当前时钟。
* 用户可以配置额外组织 trust root，但不能被 package 自己静默加入。

Registry metadata 应采用具有 root、snapshot、targets 和 freshness 防护的成熟更新框架语义，
而不是只下载一个未版本化 public-key 文件。

---

## 14. Provenance 与审计

发布流程生成 machine-readable attestation，记录：

* source repository/commit 或 source archive digest
* builder/toolchain identity
* lockfile/build fingerprint
* target/profile
* reproducibility result
* unsafe、build script 和 requested capabilities summary
* produced subjects/digests

Provenance 不应包含 secret、credential、完整用户路径或不必要个人信息。它证明构建声明和产物绑定，
但不自动证明源码安全。

CLI 必须能在不执行 package 的情况下显示 dependency、license、unsafe、script、capability、signature、
revocation、ABI 和 provenance 摘要，供 CI/policy engine 审计。

---

## 15. Capability 与供应链策略

Dependency 声明 capability 不自动获得应用授权。构建分两个集合：

```text
requested capabilities = root + transitive declarations
granted capabilities = deployment policy subset
```

Component 实例只接收 loader 显式委托的 subset。Lockfile 与 stable component metadata 同时保存声明，
使新增 capability 在升级 diff 中可见。

默认 policy 可以拒绝：

* 新增或扩大 capability scope
* 新 unsafe package
* 新 build script 或 networked build
* registry/source 变化
* publisher/signing key 变化
* license policy 变化
* unverifiable provenance

`nexa update` 必须把这些变化作为结构化 review summary 输出，不能只显示版本号。

---

## 16. 发布、镜像与离线

Registry 发布为两阶段：上传不可变 blobs，再原子发布签名 metadata。失败上传不会形成可解析版本。

Mirror 可以重新托管相同 digest blobs，但不能改变 package identity/source origin；授权 mirror metadata
必须由 trust policy 明确接受。依赖 confusion 通过 source-qualified identity、无 fallback 与 pinned digest 防护。

离线构建要求：

* lockfile 完整且与 manifest 一致。
* 所有 source blob、registry metadata、trust root 和 toolchain 已在 verified cache。
* 所有 freshness/revocation policy 有明确 offline snapshot 规则。
* 不尝试 DNS、Git update、registry index 或时间服务。

缺少任何 locked input 时失败并列出 digest，不从相似名称或全局 cache 猜测替代品。

---

## 17. 诊断

本 ADR 使用 `E6100-E6199` 作为 package、artifact 与供应链诊断范围。

| 错误码 | 含义 |
|--------|------|
| `E6101` | manifest schema/identity/edition 无效 |
| `E6110` | 依赖约束冲突或同身份多版本请求 |
| `E6111` | dependency source 不允许或发生 fallback 尝试 |
| `E6120` | lockfile 缺失、过期或与 manifest 不一致 |
| `E6121` | offline/locked input 缺失 |
| `E6130` | source/archive digest mismatch |
| `E6131` | archive path、重复项或解压预算无效 |
| `E6140` | signature 无效、key 未授权或已撤销 |
| `E6141` | registry snapshot/freshness policy 失败 |
| `E6150` | component target/ABI/runtime policy 不兼容 |
| `E6160` | capability、unsafe 或 build-script policy 拒绝 |
| `E6170` | build 不可复现或 fingerprint 输入不完整 |

诊断必须给出 package/source identity、dependency path、expected/actual digest/ABI/policy 和修复方向；
不得输出 credential、private key、authorization header 或 secret environment value。

---

## 18. 被拒绝的方案

### 18.1 把 NIR/MIR 作为公共包格式

这会冻结内部优化和编译器结构，绕过 source/ABI 验证，并与 ADR-003 冲突。

### 18.2 复制 Node/npm 模块解析

目录搜索、隐式 index/extension 和 source fallback 会降低确定性并增加 dependency confusion 风险。

### 18.3 同一 package identity 并存多个版本

它产生类型/组件身份分裂和 one-version 问题；初始生态选择明确冲突而不是复杂隔离。

### 18.4 只在 lockfile 记录版本号

同版本内容、source、feature、capability 或签名可以漂移，无法离线验证。

### 18.5 只签 payload binary

攻击者仍可替换 manifest、ABI、capability 或 target selector。签名必须覆盖完整规范 statement。

### 18.6 认为有效签名天然可信

任何攻击者都能生成自己的 key；必须有 namespace delegation、rotation、revocation 和本地 policy。

### 18.7 默认执行安装脚本

它在解析依赖时获得过大权限，破坏可复现构建并扩大供应链攻击面。

---

## 19. 后果

正面结果：

* 内部 compiler cache 可以快速演进，不污染稳定组件合同。
* Lockfile、digest 和 source identity 支持确定、离线和可审计构建。
* 签名同时绑定 payload、ABI、capability 与 provenance。
* 一版本规则减少类型身份分裂和组件加载复杂度。
* 默认禁用脚本并显式 capability，使供应链权限变化可审查。

成本与限制：

* Resolver 对冲突更严格，生态可能需要更快升级或显式 fork package。
* Registry 需要不可变 blob、签名 metadata、delegation、revocation 和 snapshot 基础设施。
* 可复现构建要求工具链、linker、archive、debug path 与签名 envelope 全部规范化。
* 发布和加载比“下载后直接执行”多出显著验证步骤。

---

## 20. 分阶段实现

### Phase P0：Manifest 与 Resolver

* `nexa.toml` schema
* source-qualified package identity
* single-version deterministic resolver
* feature/target/capability graph

### Phase P1：Lockfile 与 Cache

* canonical `nexa.lock`
* locked/offline/frozen modes
* content-addressed verified cache
* dependency/policy review summary

### Phase P2：Source Package

* normalized `.nexpkg`
* path/archive budget verifier
* immutable local registry fixture
* publish/yank semantics

### Phase P3：Artifacts 与 Reproducibility

* private compiler cache keys
* runnable artifact metadata
* `.nexc` stable component container
* build fingerprint 与 reproducibility comparison

### Phase P4：Signing 与 Provenance

* canonical signature statement
* SHA-256/Ed25519 implementation through audited libraries
* registry trust/delegation/revocation metadata
* build attestation 与 audit CLI

### Phase P5：Loader 与 Bootstrap

* component target/ABI/capability validation
* Stage artifact provenance
* offline trust snapshot
* malicious package/component conformance corpus

---

## 21. 测试要求

接受测试至少覆盖：

* registry、Git commit 和 workspace/path dependency resolution
* constraint/feature/target graph 的稳定 lockfile
* locked/offline/frozen 构建和 verified cache reuse
* normalized archive 在不同 filesystem/timezone 上得到相同 tree digest
* source、runnable、private cache 和 component artifact 分类
* multi-target component 选择与 Host/component ABI hash 验证
* valid signature、publisher delegation、key rotation 和 offline trust snapshot
* 同一 build fingerprint 在独立 workspace 产生相同 logical artifact digest

拒绝或 conformance-fail 测试至少覆盖：

* dependency confusion、source fallback 和同 identity 多版本冲突
* stale/tampered lockfile、digest mismatch 和 cache poisoning
* archive path traversal、duplicate/case collision、zip bomb 与 symlink escape
* overwritten registry version、unauthorized publisher、revoked/expired key 和 rollback metadata
* tampered capability/ABI/target table 或 signature downgrade
* undeclared env/network/file build input
* component 暴露 NIR/MIR 或 target/runtime 不兼容

Security corpus 必须使用无害 fixture key 与 synthetic payload，不保存真实 credential。

---

## 22. 验收标准

ADR-009 进入实现完成状态前必须满足：

1. Resolver 对相同 manifests/snapshot 产生相同单版本 graph 与 lockfile。
2. locked/offline/frozen 模式不会访问或接受未声明 source。
3. Source archive 在解析前通过 path、size、digest 和 signature 验证。
4. Private cache 与 stable component 格式在类型、metadata 和 loader 中完全分离。
5. Component signature 覆盖 payload、identity、ABI、capability、target 与 provenance reference。
6. Loader 在任何用户初始化前完成 trust、target、ABI、capability 和 runtime policy 验证。
7. 独立环境的 reproducibility test 比较通过，并覆盖 ADR-000 Stage artifact。
8. accepted 与 malicious/rejected corpus 同时进入 CI。

---

## 23. 最终决定

Nexa 2.0 的分发链路为：

```text
nexa.toml
-> deterministic resolver
-> canonical nexa.lock
-> verified content-addressed source
-> reproducible target build
-> runnable artifact or stable .nexc component
-> digest + signature + provenance
-> pre-execution loader policy
```

源码包、编译缓存和稳定组件保持不同生命周期；lockfile 固定输入，签名绑定合同，loader 在执行前验证信任。
