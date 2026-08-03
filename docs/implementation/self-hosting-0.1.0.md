# Futao 0.1.0 自举实施计划

## 目标

从固定的 `nexac 0.0.1` Rust Stage 0 出发，逐步交付由 Futao
编写的编译器核心，建立自动化、可复现、可回滚的 `C0 -> C1 -> C2 -> C3` 构建链。
工具链在自举期间保持 `0.0.x`；只有 ADR-011 的完整门槛通过后才发布 `0.1.0`。

本计划是交付顺序，不改变 Language 1.0 已冻结的兼容性基线，也不把 ADR 的
Accepted 状态解释为对应实现已经存在。

## 当前基线

* 工具链版本：`0.0.10`。
* Stage 0：固定为 `nexac 0.0.1` Rust 实现，覆盖 Lexer、Parser、Resolver、Type Checker、HIR、CFG MIR、
  reference interpreter、诊断、conformance 与 release gate。
* 语言名称：新设计和源码使用 Futao / `.ft`；现有 Nexa / `.nexa` 输入在迁移策略
  明确前保留兼容。
* 默认分支：`mvp`。
* 已接受架构：ADR-000 至 ADR-011。
* `0.0.2` 已收敛产物边界：自举输出是内部 NIR，公共 stable component 仍由 ADR-009 定义为 `.nexc`。
* `0.0.3` 已建立 Futao 源码入口、纯 compiler core 与 differential 基础设施。
* `0.0.4` 已建立自举 ownership/storage reference kernel 与独立验收门槛。
* `0.0.5` 已建立 target-neutral typed NIR、独立 verifier、显式 target layout 与私有自举产物。
* `0.0.6` 已冻结 `futao-bootstrap-v1` 并交付 content-addressed Bootstrap Stdlib `0.0.1`。
* `0.0.7` 已交付真实 Futao Lexer、UTF-8 snapshot schema 与无差异 corpus/fuzz gate。
* `0.0.8` 已交付真实 Futao Parser、lossless CST/recovery snapshot schema 与无差异 corpus/fuzz gate。
* `0.0.9` 已交付真实 Futao Resolver、resolver snapshot schema、模块图/作用域/名称可观察合同与无差异 corpus/fuzz gate。
* 当前大目标：按
  [`0.0.11` 完整 compiler core 实施合同](futao-compiler-core-0.0.11.md)
  先闭合 `0.0.10` 剩余 Type Checker differential，再完成 HIR/MIR/NIR、driver
  与完整 corpus 门槛。

## 关键依赖

```text
ADR-000 边界收敛
  -> deterministic core + differential harness
  -> 自举所需 ownership/storage kernel
  -> NIR builder/verifier + internal bootstrap artifact
  -> Bootstrap Profile + Bootstrap Stdlib
  -> Lexer -> Parser -> Resolver -> Type Checker
  -> HIR/MIR/NIR lowering + compiler driver core
  -> C1 -> C2 -> C3
  -> cross-platform reproducibility + two release candidates
  -> 0.1.0
```

每条箭头是验收依赖。后续阶段可以提前做不影响接口的研究，但不能在前置门槛失败时
发布为已完成里程碑。

## 版本路线

| 版本 | 交付里程碑 | 发布门槛 |
|---|---|---|
| `0.0.1` | 固定 Rust reference/bootstrap baseline | Language 1.0 release gate 保持通过 |
| `0.0.2` | 修订并接受 ADR-000 | 冻结 Stage 0、provenance、内部自举产物及 ADR-003/009 边界 |
| `0.0.3` | Futao 编译器入口与 differential 基础设施 | `.ft` 输入、纯 compiler core 接口、canonical diagnostics/corpus 可双实现比较 |
| `0.0.4` | 自举 ownership/storage kernel | Move/Drop 与受限 `Vec`、字符串构建、Arena 满足编译器 workload |
| `0.0.5` | NIR 与自举产物 | target-neutral NIR builder、独立 verifier、确定性序列化与 schema/version gate |
| `0.0.6` | Bootstrap Profile 与 Stdlib | 受限语法/语义集合冻结，stdlib 无隐式 Host I/O 且构建确定 |
| `0.0.7` | Futao Lexer | token/span/diagnostic differential 与 fuzz corpus 无未解释差异 |
| `0.0.8` | Futao Parser | lossless CST、recovery、accepted/rejected differential 与 fuzz 通过 |
| `0.0.9` | Futao Resolver | module graph、scope、visibility、cycle diagnostics 与 Rust 一致 |
| `0.0.10` | Futao Type Checker | inference、generics、match/ownership 检查与 Rust 一致 |
| `0.0.11` | 完整 Futao compiler core | HIR/MIR/NIR lowering、diagnostics、driver core 能编译 corpus |
| `0.0.12` | Stage 1 compiler | C0 产出 C1；C1 能编译自身、Bootstrap Stdlib 与真实示例 |
| `0.0.13` | Release candidate 1 | C1/C2/C3 自动化；normalized C2/C3 与跨平台产物一致 |
| `0.0.14` | Release candidate 2 | 第二个连续稳定 RC；性能、资源上限、fallback 与 provenance 通过 |
| `0.1.0` | 默认自托管工具链 | ADR-011 全部门槛通过，release PR 执行唯一一次版本提升 |

版本号是默认切片。若某阶段超出一个可审查 PR，可增加 `0.0.x` 版本，但必须保持依赖
顺序和验收语义；不得合并多个未验证阶段后直接跳到 `0.1.0`。

## 阶段实施

### `0.0.2`：冻结自举合同

* 修订 ADR-000，使 Futao compiler core 输出 ADR-003 定义的内部 NIR，而不是提前承诺
  公共 VM bytecode 或稳定 `.nca`。
* 定义 C0 的可获取来源、校验和、Rust/MSRV、schema 与 Bootstrap Stdlib pin。
* 定义 canonical serialization、允许规范化的字段和禁止进入产物的非确定输入。
* 增加 ADR 一致性审查和 bootstrap manifest schema 测试。

验收：ADR-000 Accepted；artifact、runtime、backend、package component 的边界无冲突；
从干净环境可重建固定 C0。

交付状态：已实现。`bootstrap/stage0/bootstrap-manifest.json` 固定 Stage 0 source commit、
source archive/Cargo.lock SHA-256、Rust 1.80 与 locked release recipe；
`cargo xtask bootstrap-contract --rebuild-stage0` 会重算 provenance、从 detached worktree
重建 `nexac 0.0.1` 并执行版本 smoke。Bootstrap Stdlib 被显式固定为 `not-defined`，
直到 `0.0.6` 才允许替换为真实版本与 digest。内部 NIR/public component 混用进入 rejected fixture。

### `0.0.3`：建立双实现比较面

* 为新源码建立 `.ft` 入口，同时保留历史 `.nexa` compatibility fixtures。
* 将文件系统、环境变量、时钟、随机数、路径规范化放在 Host shell，compiler core 只接收
  显式输入并返回结构化输出。
* 定义 token、CST、diagnostic、HIR、MIR、NIR 的 canonical dump。
* differential harness 对同一 corpus 运行 Rust/Futao 实现，并分类所有已知差异。

验收：同输入重复执行产出相同 dump；路径、locale、hash iteration 不改变结果；失败
fixture 具有稳定 diagnostic code 与 span。

交付状态：已实现。`CompilerInput` 只接受显式 UTF-8 逻辑 source identity 和源码集合，
拒绝 Host 路径、重复 source 与缺失 entry；`CompilerOutput` 返回稳定 source ordinal、结构化
diagnostic、typed HIR、完整 CFG MIR 以及 token/CST/diagnostic/HIR/MIR/NIR 六阶段状态。
`.ft` 和历史 `.nexa` 均可作为 entry/import，混合图作为迁移期兼容合同保留。
`nexac dump` 输出 schema version 1 canonical JSON；输入顺序、逻辑根目录、进程工作目录、
locale、时区与 Rust `HashMap` 随机种子不进入比较结果。当前 Rust reference adapter 可执行，
Futao adapter 明确报告 unavailable；NIR 明确报告计划于 `0.0.5` 提供，不存在占位 crate
或伪产物。详细 schema 与分类规则见
[`differential-0.0.3.md`](differential-0.0.3.md)。

### `0.0.4`：只交付自举需要的运行时子集

* 实现并验证编译器 workload 所需的所有权、Move、Drop 和 cleanup。
* 提供受限、确定性的 `Vec`、map/set、string builder、arena 与 intern table。
* 明确 OOM、容量溢出、迭代顺序和 drop glue 行为。

验收：accepted 与 compile-fail 测试同时存在；Miri/sanitizer 可覆盖的 Rust 底座通过；
大型 corpus 不出现数量级内存或时间退化。

交付状态：已实现。`nexa_storage` 以 `#![forbid(unsafe_code)]` 提供 Host-reference layout、
不可伪造 allocator provenance、失败保持原值的受限 owned storage、UTF-8 构建、逆序 Drop、
generation-checked Arena、稳定有界集合与纯 Place ownership state machine。accepted、rejected、
compile-fail、ZST、溢出、资源清理、Rust 1.80、Miri 和 20,000-entry release workload 均进入
自动化门槛。该状态只表示自举所需子集交付，不宣称 ADR-004 的 target layout/NIR verifier、
Box/Shared、跨后端或 ABI 验收完成。详细边界见
[`storage-kernel-0.0.4.md`](storage-kernel-0.0.4.md)。

### `0.0.5`：交付可验证的内部 NIR

* 实现 typed NIR builder、target layout 描述、verifier 与 deterministic serializer。
* compiler core 不依赖 LLVM 类型；backend 只消费已经完成语义决策的 NIR。
* schema 版本不兼容时 fail closed；独立 Rust verifier 不信任 Futao 生成器。

验收：mutation/rejected fixtures 被 verifier 拒绝；round trip 保持 canonical form；固定
输入跨平台产生相同目标无关 NIR。

交付状态：已实现。`nexa_nir` 交付 N0/N1 typed module、builder、独立 verifier、32/64-bit
显式 target layout 和 schema 1 private canonical artifact；compiler core 将 `Bool`、`Int`、
`Unit`、single-block return、single-assignment local、static call 与 typed print intrinsic
降低为真实 NIR。字符串、聚合、闭包、可变 local、多 block 和后续 operation 明确输出
`Deferred`，不伪造部分产物。bootstrap manifest 固定 magic、schema、verifier/version、
target profile、feature flags、canonical encoding 与 content hash；accepted fixture 和内容
篡改、未知字段、未知 schema rejected fixtures 由 `cargo xtask nir-artifact` 精确验证。
该里程碑不包含 LLVM、Host ABI、公共 NIR 或 `.nexc` 扩展。详细边界见
[`nir-artifact-0.0.5.md`](nir-artifact-0.0.5.md)。

### `0.0.6`：冻结 Bootstrap Profile

* 明确自托管源码可使用的语法、类型、泛型、所有权和标准库 API。
* 禁止 compiler core 直接使用文件系统、网络、进程、系统时钟或无序迭代。
* Bootstrap Stdlib 与通用 stdlib 分层，版本和内容哈希进入 manifest。

验收：profile lint 能拒绝越界能力；stdlib 可由 C0 确定性构建；版本升级有兼容和回滚
fixture。

交付状态：已实现。`CompilerOptions::bootstrap_v1()` 选择 `.ft`-only 的纯编译器能力面，
以 `E6201` 至 `E6203` 拒绝可变绑定/赋值、`while`/`break`/`continue` 和 ambient `print`；
默认 `application` profile 保持 Language 1.0 行为。canonical dump 固定
`compilationProfile`，使相同源码在不同能力面下成为不同构建输入。
`bootstrap/profile/bootstrap-profile-v1.json` 冻结允许的语法、值、所有权、迭代顺序和
禁止的 Host capability，其 SHA-256 进入顶层 manifest。

Bootstrap Stdlib `0.0.1` 提供 `Option`/`Result`、span/diagnostic、纯数组与文本工具、
stable insertion-order map/set/bit set、StringBuilder 和 identity/generation-checked
persistent Arena。manifest 固定排序源码清单、源码树 digest 与 profile-aware canonical
build digest；正反 source insertion order 必须得到相同 dump。升级 fixture 接受
`0.0.0 -> 0.0.1`，回滚 fixture 拒绝 `0.0.1 -> 0.0.0`。
`cargo xtask bootstrap-profile` 验证上述合同和 observable behavior；
`cargo xtask bootstrap-contract --rebuild-stage0` 还会重建固定 `nexac 0.0.1`，使用 lossless
CST 只把 import suffix 从 `.ft` 临时投影为历史 `.nexa`，并要求该 C0 check 完整 stdlib。
详细边界见
[`bootstrap-profile-stdlib-0.0.6.md`](bootstrap-profile-stdlib-0.0.6.md)。

### `0.0.7` 至 `0.0.11`：纵向迁移编译器核心

每个阶段遵循同一 PR 模板：先冻结 Rust observable contract，再实现 Futao slice，补充
accepted/rejected 与 fuzz seeds，运行 differential，记录并消除差异，最后才允许默认
测试路径调用新实现。禁止把类型检查放入 Parser，也禁止 codegen 直接读取 token/CST。

| 版本 | 必须比较的 observable contract |
|---|---|
| `0.0.7` | token kind、text range、trivia、lexical diagnostics |
| `0.0.8` | lossless CST、recovery events、parse diagnostics |
| `0.0.9` | module graph、symbol identity、scope/visibility diagnostics |
| `0.0.10` | inferred types、generic substitution、ownership/match diagnostics |
| `0.0.11` | HIR/MIR/NIR、diagnostics ordering、compiler driver result |

`0.0.7` 交付状态：已实现。`nexa_parser::lex_source` 冻结 Rust reference
observable；`bootstrap/compiler/src/lexer.ft` 是在 `futao-bootstrap-v1` 下编译的
纯 Futao FSM lexer。Host 只显式提供 Unicode scalar 与 UTF-8 byte-offset 表，
application-only driver 以严格版本协议返回 snapshot，不把 `print` 带入 compiler core。
4 个 accepted、3 个 rejected 与 4 个 fuzz seed 全部运行真实 Rust/Futao
adapter，token kind/range/trivia 与 lexical diagnostic 零差异。差异失败时
保留双侧 canonical snapshot；不存在 suppression list。Rust Lexer 仍为默认路径，
直到后续 parser 与 driver slice 验证完成。详细合同见
[`futao-lexer-differential-0.0.7.md`](futao-lexer-differential-0.0.7.md)。

`0.0.8` 交付状态：已实现。`Parse::parser_diagnostics` 将 parser-only diagnostics
从既有组合诊断中稳定分离；Rust/Futao adapter 以 schema 1 比较完整 balanced
lossless CST event stream、具体 `Error` node recovery range，以及 diagnostic code、
severity、label style 和 UTF-8 byte range。纯 Futao parser 直接调用已验证 lexer，
并在 `futao-bootstrap-v1` 下拒绝 ambient Host capability。8 个 accepted、8 个
rejected 与 5 个 fuzz seed 均以真实 adapter 运行且无分类差异；512-byte bounded
mutation gate 继续 fail closed。Rust Parser 仍为默认路径。详细合同见
[`futao-parser-differential-0.0.8.md`](futao-parser-differential-0.0.8.md)。

`0.0.9` 交付状态：已实现。真实 Futao resolver 在已验证 lexer/parser 之上构建
确定性 module graph、symbols、visibility、function/block/match-arm/closure/for scopes、
local/generic bindings、record fields、union variants 与 resolved names，并返回
`E2001`、`E2002`、`E4002` 至 `E4005`。schema 1 快照比较 module graph、symbols、scopes、
bindings、names 和 diagnostics 六类 observable；accepted/rejected 图与 4 个 fuzz seed
共 6 个 case 全部由 Rust/Futao 真实 adapter 执行且零差异。严格
`FUTAO-RESOLVER-2` 协议、双侧快照保留、source-aware runtime failure、snapshot validation
和双侧 SHA-256 digest 均 fail closed；未知 enum tag 不再回退成合法 observable，owner-local
child identity 与声明 span 必须匹配 lowered source declaration contract，payload owner 也必须是
源码中声明的 variant。Rust Resolver 仍为默认路径。详细合同见
[`futao-resolver-differential-0.0.9.md`](futao-resolver-differential-0.0.9.md)。

`0.0.10` 首个 slice 已实现并完成合同收口：Rust/Futao adapter 以 schema 1 比较已解析、
带稳定 source span 的表达式表，覆盖全部 12 种节点，严格验证 node shape、输入绑定的
node count、2048 条 bounded diagnostics、canonical diagnostic order 和未知 tag，并以
semantic oracle 固定 accepted/rejected 结果。fixture JSON 的 unknown 字段与 optional 字段中
kind-incompatible 的非空值在 typed construction 前即被拒绝；1025 nodes、2048/2049 diagnostics、
schema/tag、span、缺失/尾随字段与 forward child reference 均有回归门禁。`E3001`、`E3002`、
`E3003` 均 fail closed；
泛型替换、match exhaustiveness、ownership 和 mutable capture 仍是后续 `0.0.10` slices。
详细合同见
[`futao-type-checker-0.0.10.md`](futao-type-checker-0.0.10.md)。

Phase B5 的 parser scalability 前置已实现：chunked persistent sequence 取代大数组逐项
复制，固定顺序叶批处理避免多个分治 runner 叠加越过 64 层调用限制；独立 release gate
在每文件 64,000,000 step ceiling 下解析排序后的 9 文件 bootstrap compiler parser 源码图，
且保持 `0.0.7`/`0.0.8` observable 与 512-byte mutation ceiling 不变。详细边界见
[`futao-parser-self-graph-scalability-0.0.8.md`](futao-parser-self-graph-scalability-0.0.8.md)。

### `0.0.12` 至 `0.0.14`：建立自举链

`0.0.12` 交付 C1，且 C1 能编译自身、Bootstrap Stdlib 和至少一个覆盖模块、泛型、
ownership、错误路径的真实项目。`0.0.13` 自动构建到 C3，并以
`normalize(C2) == normalize(C3)` 为门槛。`0.0.14` 在相同 gate 下形成第二个连续 RC，
验证性能预算、资源上限、Stage 0 重建和 Rust fallback。

任何 Stage mismatch 都必须保存输入、C0/C1/C2/C3 manifest、canonical dump 和最小复现；
不得通过扩充 normalization 忽略有语义影响的差异。

### `0.1.0`：切换默认实现

只创建版本提升与发布材料 PR，不在该 PR 混入新语言能力。PR 必须证明 ADR-011 的
清单逐项满足。合入后从 `mvp` 创建 tag，发布自托管实现；Rust 实现继续参与 CI 和
differential，直到后续 ADR 单独批准移除。

当前 release workflow 会拒绝 `0.0.x` 范围外的工具链。`0.1.0` 发布 PR 必须把该临时
限制替换为实际执行 C0/C1/C2/C3、differential、跨平台复现与连续 RC 证明的 gate；
仅删除版本限制不构成验收。

## PR 切片与验证

* 每个 PR 从最新 `mvp` 创建独立分支，并以 `mvp` 为 base。
* 一个 PR 只交付一个 observable contract；机械生成内容和行为变更分开。
* 新语言行为必须同时包含 accepted 与 rejected/compile-fail 测试。
* 自托管阶段必须附 Rust/Futao differential 摘要；不允许未分类差异。
* 本地至少运行 `make check`；涉及文档时运行 `pnpm --dir docs build`。
* 阶段门槛由 CI 证明，required checks 全部通过后 squash merge。

## 风险与回滚

| 风险 | 控制与回滚 |
|---|---|
| Rust 与 Futao 实现一起产生相同错误 | 保留独立 verifier、rejected corpus 和后续 diverse compilation 路径 |
| 产物格式过早稳定 | `0.1.0` 前仅承诺同工具链内部 schema；公共 component 由 ADR-009 另行冻结 |
| Bootstrap Profile 追逐完整 2.0 平台 | 仅纳入实现 compiler core 的最小子集，其他能力按 ADR-004 至 ADR-009 推进 |
| Stage 构建不可复现 | fail closed，保留 artifacts，回退上一 `0.0.x` 与 Rust C0 |
| 自托管编译器性能不足 | 在切换默认前维持 Rust 默认，使用固定 workload 设定时间和内存预算 |
| 大 PR 难以审查 | 按编译阶段和 observable contract 切片，所有合并均 squash |

## 非目标与后续顺序

`0.1.0` 不要求用 Futao 重写 allocator、verifier、LLVM binding、Wasm backend、Host
adapter、package downloader 或签名验证，也不承诺稳定 Native ABI 或公共 bytecode。

完整平台能力继续遵循已确定顺序：

```text
ADR-004 内存布局
  -> ADR-005 Host ABI
  -> ADR-006 错误模型
  -> ADR-007 Async
  -> ADR-008 Wasm/UI Host
  -> ADR-009 包与签名
```

这些 ADR 中被 Bootstrap Profile 实际依赖的最小子集可以进入相应 `0.0.x`，其完整
实现则独立拆分 PR 和版本，不得借自举名义扩大 `0.1.0` 的范围。
