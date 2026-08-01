# ADR-005：Host ABI、Capability 与资源 Handle

* **状态**：Accepted
* **日期**：2026-08-01
* **目标版本**：Nexa Language 2.0
* **关联 ADR**：

  * ADR-001：Nexa 2.0 语言架构、所有权与运行时模型
  * ADR-002：所有权、借用、Move 与确定性 Drop 语义
  * ADR-003：NIR、LLVM 后端边界与稳定 ABI
  * ADR-004：Allocator、Arena、集合与运行时内存布局
* **实现优先级**：P0
* **兼容性范围**：不修改 Nexa Language 1.0 已冻结语义

---

## 1. 背景

Nexa 程序需要访问文件、网络、时间、随机数、进程、窗口和设备。这些能力在不同平台上
由操作系统、浏览器、嵌入式 runtime 或测试 harness 提供，不能成为 parser 或后端的特殊语义。

ADR-003 选择版本化 Host ABI、typed function ID、descriptor 和 handle；ADR-004 进一步禁止
跨边界暴露私有内存布局。本 ADR 冻结 Host ABI 的授权、协商、调用和资源生命周期合同。

目标是让同一个经过验证的 NIR 程序能够连接 Native Host、Wasm Host 或 deterministic test Host，
同时不获得 manifest 之外的 ambient authority。

---

## 2. 决策摘要

1. 所有平台服务都通过版本化 Host ABI 提供，不作为语言内建语句。
2. Host 接口由规范化 IDL 描述，并生成 caller stub、host binding、verifier schema 与 ABI hash。
3. 调用使用稳定 `HostFunctionId`，不在运行时按字符串搜索函数。
4. 程序声明 capability，部署者授予 capability；有效权限是二者交集。
5. capability 默认拒绝、可缩减、可撤销，不存在隐式文件系统、网络、时钟或环境权限。
6. 外部资源使用 typed、generation-checked handle；Nexa 源码不能伪造其数值表示。
7. 每个参数明确为 `copy`、`borrow`、`mut_borrow`、`take`、`return_owned` 或 `handle`。
8. `take` 在预检成功、进入 Host 实现前提交；提交后即使操作返回错误，caller 也不再拥有旧 handle。
9. borrow 只在一次同步调用期间有效，Host 不得保存；长期保留必须 clone 或 take。
10. Panic、unwind、平台异常和裸 errno 不得穿越 Host ABI。
11. Host 错误使用稳定 status/category/detail descriptor，并与内部 `Result<T, E>` 通过生成 wrapper 转换。
12. 谁分配谁释放；owned 输出必须附带 release function 或 typed owned handle。
13. 加载器在执行程序代码前完成版本、ABI hash、capability、target 与资源预算验证。
14. 同步调用是基础合同；异步、取消和 UI 批处理分别由 ADR-007 和 ADR-008 细化。

---

## 3. Host IDL 与描述符

Host 模块以规范化 IDL 定义：

```text
host module fs@1 {
  capability filesystem;

  function open(
    path: borrow Utf8View,
    options: copy FileOpenOptions
  ) -> Result<return_owned Handle<File>, FsError>;

  function read(
    file: borrow Handle<File>,
    buffer: mut_borrow ByteSlice
  ) -> Result<U64, FsError>;

  function close(
    file: take Handle<File>
  ) -> Result<Unit, FsError>;
}
```

规范化 schema 必须包含：

* module ID 与 major version
* `HostFunctionId`
* 参数和返回类型 descriptor
* ownership mode
* required capability 与可选 scope
* sync/async call kind
* error domain
* resource limit/accounting class
* since/deprecated version

生成 ABI hash 的输入是规范化 schema，不包含文档、参数显示名、源文件路径或生成器时间戳。
同一 schema 必须在所有机器上产生相同 hash。

`HostFunctionId` 由版本化注册表分配。ID 不复用；删除的 ID 保留 tombstone，避免旧 artifact
把新函数误认为旧函数。源代码名称可以重命名显示，但 stable ID 与语义不能静默改变。

---

## 4. 版本协商与加载

Artifact 必须声明：

```text
minimum_host_abi
maximum_tested_host_abi
required_modules
required_function_ids
required_capabilities
host_abi_hashes
resource_budget_requirements
```

加载顺序固定为：

```text
parse metadata
-> validate bounds and signature
-> choose compatible Host ABI major/minor
-> validate required function schemas and ABI hashes
-> compute effective capabilities
-> validate resource budgets
-> instantiate program
```

程序代码不能在协商完成前执行。版本规则：

* Patch 只能修正文档或不改变合同的实现缺陷。
* Minor 可以新增 function、capability、error variant 或有默认值的 optional field。
* Existing function 的参数、ownership、错误语义和 capability 不能在 minor 中收紧或改变。
* Major 可以破坏兼容性，但必须使用新 module/version identity。
* Host 可以高于 `maximum_tested_host_abi`，但只能按程序选定的兼容视图暴露行为。

单个应用实例只绑定一个选定版本的同名 Host module，避免同一资源被两个不兼容合同解释。

---

## 5. Capability 模型

Capability 是不可伪造的授权，不是布尔配置或可猜测字符串。

有效权限计算为：

```text
declared by package
intersection granted by deployment
intersection delegated to component/task
intersection currently not revoked
```

基础 capability namespace 包括：

```text
filesystem
network
time
random
environment
process
ui
clipboard
camera
microphone
gpu
database
```

Capability 可以带受验证 scope，例如只读目录、允许的网络 origin、单个窗口或数据库 namespace。
scope 必须采用结构化 descriptor，不能依赖字符串前缀等不安全匹配。

规则：

* 未声明或未授予的 capability 在实例化或调用前拒绝。
* 组件只能显式接收父级委托的子集，不能扩大权限。
* 文件 handle 不自动授予打开其他路径的权限。
* 时间和随机数也属于 capability，Deterministic Profile 默认不提供。
* capability token 不可序列化进日志、包、lockfile 或用户可读错误。
* 错误信息可以报告缺少的 capability 名称，但不得泄漏授权 token。

Manifest 中的 capability 是最大请求，不代表安装或运行时已经授权。

---

## 6. Handle 表示与状态机

稳定概念 descriptor 为：

```text
Handle {
  slot: U32,
  generation: U32,
  kind: U32,
  rights: U32
}
```

字段宽度可以在未来 ABI major 中改变；当前合同冻结的是语义：

* `slot + generation` 标识一个 Host instance 内的活资源。
* `kind` 必须与函数签名所需 resource kind 相同。
* `rights` 只能是创建时 capability 的缩减结果。
* table entry 同时记录 owner state、revocation epoch 和 destructor。
* handle 不能跨 Host instance、进程或 artifact 直接重用。

Owned handle 状态机：

```text
Vacant
  -> Alive(owner = caller)
  -> Borrowed(call-scoped)
  -> Alive(owner = caller)

Alive(owner = caller)
  -> Transferred(owner = host)
  -> Alive(owner = returned caller) | Closed

Alive -> Revoked -> Closed
Closed -> Vacant(next generation)
```

状态转换必须原子地更新 table，stale generation 永远不能重新获得新资源访问权。

语言层资源通常是 Move-only。只有 IDL 明确提供 `clone` 时才能创建第二个 owned handle；
clone 可以降低 rights，但不能扩大。`close(take handle)` 即使报告底层关闭错误也消费 handle，
避免调用方重复关闭不确定状态的资源。

---

## 7. Ownership 与调用提交点

每次调用分为预检与执行：

```text
preflight:
  function/version/capability/type/handle/generation/rights/budget

commit:
  lock mut_borrow entries
  transfer take arguments
  enter Host implementation

finish:
  validate output
  publish return_owned values
  release borrows
  return status
```

预检失败时没有 ownership 变化。commit 完成后：

* `take` 参数对 caller 永久失效，无论结果是 Ok 还是 Err。
* `borrow` 和 `mut_borrow` 在调用返回时结束。
* Host 不能把 borrow pointer、descriptor 或 handle borrow 存入长期状态。
* `mut_borrow` 期间同一 entry 不允许其他访问。
* 输出只有在完整验证后才发布给安全 Nexa 代码。
* 部分创建的 owned 输出由 Host 清理，不能通过未初始化 out parameter 泄漏。

可能重入 Nexa 的 Host function 必须在 IDL 中显式标记 `reentrant`。基础同步函数默认不重入，
不得在持有 `mut_borrow` 时发起未声明 callback。

---

## 8. 错误合同

稳定调用使用：

```text
NexaStatus
+ success out descriptor
+ optional NexaHostError
```

`NexaHostError` 至少包含：

```text
domain_id
code
retry_class: Never | Immediate | Backoff | AfterExternalChange
message: optional owned UTF-8 handle
details: optional versioned descriptor
```

规则：

* status 与 out parameter 初始化状态不能矛盾。
* 未知 minor error code 映射为 domain 的 `Unknown`，不能按 success 处理。
* OS errno、JavaScript exception 或 browser rejection 必须在 Host 内转换。
* error message 是诊断文本，不作为程序控制流合同。
* 可恢复失败映射到 `Result`; invariant failure 终止或隔离 Host instance。
* Panic、C++ exception、Rust panic 和 Wasm trap 不得直接穿越边界。

ADR-006 冻结 `Result`、panic、defer 与 Drop 的完整交互顺序。

---

## 9. 内存与 descriptor 边界

Borrowed descriptor 只在调用期间有效：

* UTF-8 view 使用 byte length，不保证 NUL 结尾。
* Slice 声明 element type、length、stride、alignment 和 mutability。
* Host 在读取前验证所有 offset、length、stride 和整数运算。
* `mut_borrow` 输出只能写入声明的已分配范围。
* Host 不得把 caller memory 当作自己的 allocation 释放。

Owned 输出必须选择：

1. Host-owned typed handle，稍后调用 Host release；或
2. Nexa 提供的受限 output buffer，由 Nexa allocator 负责；或
3. 明确版本化的 create/clone/release 函数组。

禁止跨 allocator 释放，禁止公开普通 `String`/`Vec<T>`/`Shared<T>` 私有布局，
禁止依赖 Native pointer 在 Wasm 或远程 Host 中具有意义。

---

## 10. 撤销、预算与隔离

Capability 撤销后：

* 新调用立即返回 `CapabilityRevoked`。
* 相关 live handle 标记为 revoked，除 `close`/release 外不能执行新操作。
* 已进入的同步调用允许完成；结果发布前再次应用输出验证。
* close 始终可调用且不要求原 capability，防止撤销导致资源泄漏。
* 异步调用的取消与完成竞争由 ADR-007 定义。

每个 Host instance 必须支持可配置预算：

* live handle 数
* owned output bytes
* 并发或排队调用数
* 每类资源上限
* 可选 deadline/CPU accounting

超限返回结构化错误，不得通过整数回绕、静默截断或产生无表项 handle 表示。

不同应用实例使用隔离 handle table 和 capability set。一个实例崩溃、撤销或关闭不能释放
另一个实例的资源。

---

## 11. 同步、异步与平台映射

Host IDL 明确区分：

```text
sync function
async function
stream function
callback/reentrant function
```

本 ADR 只冻结同步调用的 borrow 生命周期与提交点。异步函数不能通过保存同步 borrow 实现，
其跨暂停数据必须使用 owned buffer/handle，并遵守 ADR-007 的 task、取消和 cleanup 合同。

Native Host 可以使用 C-compatible generated binding；Wasm Host 使用 linear-memory descriptor
和相同 schema/ID；浏览器 Adapter 把 JS exception 转换为 Host error。三者必须产生等价的
ownership、capability 和错误轨迹，具体映射由 ADR-008 冻结。

---

## 12. 可观测性与安全日志

Host tracing 可以记录：

* function ID、选定 ABI version 和结果 category
* capability 名称与 scope 类型
* handle kind、generation mismatch 类型和生命周期事件
* 请求大小、持续时间和预算使用量

默认不得记录：

* capability token
* 文件内容、网络 payload、剪贴板、密钥或用户输入
* 原始内存地址
* 未经脱敏的路径、URL query 或错误 details

调用 trace 的排序在 Deterministic Host 中必须稳定。生产 Host 的时间与调度信息不成为语言语义。

---

## 13. 诊断

沿用 ADR-003 的 `E5600-E5699` Host ABI 诊断范围。

| 错误码 | 含义 |
|--------|------|
| `E5601` | Host ABI major 不兼容 |
| `E5602` | required function ID 缺失 |
| `E5603` | schema/ABI hash 不匹配 |
| `E5610` | capability 未声明、未授予或已撤销 |
| `E5620` | handle kind 或 rights 不匹配 |
| `E5621` | stale/closed handle generation |
| `E5622` | 已消费 handle 再次使用 |
| `E5630` | ownership mode 或 borrow 生命周期不合法 |
| `E5640` | Host 输出 descriptor 无效 |
| `E5650` | Host 资源预算超限 |

诊断必须区分 load-time incompatibility、call preflight failure 和 Host operation failure。
结构化字段至少包含 module/function ID、expected/actual version、capability 名称和 handle kind；
不得包含 token、裸地址或敏感 payload。

---

## 14. 被拒绝的方案

### 14.1 运行时字符串查找与动态参数数组

它把拼写、类型和所有权错误推迟到运行时，无法生成稳定 binding 或 ABI hash。

### 14.2 直接暴露操作系统 fd、pointer 或 JavaScript object

这些值可伪造、跨平台含义不同，并绕过 generation、kind、rights 与 capability 检查。

### 14.3 Manifest 声明即自动授权

包作者不能替部署者决定实际权限；声明只是请求上限。

### 14.4 Host 默认保存所有 borrow

这会把一次调用的临时 view 变成悬空引用，并迫使所有参数进行隐藏复制。

### 14.5 错误时把 `take` 所有权隐式退回

外部操作可能已经部分消费或关闭资源，自动退回会制造重复 owner 和 double-close。

### 14.6 Minor 版本静默改变现有函数

只比较版本号无法发现 ownership 或布局漂移；已有 schema 必须保持 hash 稳定。

---

## 15. 后果

正面结果：

* Native、Wasm、浏览器和测试 Host 共享同一个类型化合同。
* capability 不再依赖 ambient process state。
* generation handle 阻止大部分 stale-use 和 type confusion。
* generated binding 集中处理 descriptor、错误和 allocator 转换。
* 加载前拒绝不兼容 artifact，避免执行到一半才发现缺失能力。

成本与限制：

* Host registry、IDL 编译器、binding generator 和 conformance suite 都是必需组件。
* 每次调用具有 preflight 与 handle table 开销。
* 复杂零拷贝接口必须显式证明 borrow 生命周期，不能默认保存 pointer。
* capability scope、撤销和预算需要部署/runtime 配置。

---

## 16. 分阶段实现

### Phase H0：Schema 与 Registry

* 规范化 Host IDL
* module/function/capability ID registry
* schema canonicalization 与 ABI hash
* artifact requirement metadata

### Phase H1：同步调用

* generated Nexa stub 与 mock Host binding
* preflight/commit/finish 状态机
* borrowed descriptor 与 structured error
* panic/exception containment

### Phase H2：资源 Handle

* generation table、kind、rights 与 owner state
* clone、take、close、revocation
* leak detection 与 resource budget

### Phase H3：Capability 与加载器

* manifest declaration 与 deployment grant
* structured scope 和 delegation
* load-time negotiation 与 diagnostics
* Deterministic Host

### Phase H4：跨平台 Conformance

* Native C Host
* Wasm/browser adapter（依赖 ADR-008）
* 等价 host-call/Drop/error trace
* compatibility fixtures 和 fuzzing

---

## 17. 测试要求

接受测试至少覆盖：

* compatible minor version 协商和 optional function 缺失
* manifest 声明、部署授权和子组件权限缩减
* open/read/close 的 owned/borrow/take 状态轨迹
* clone 降权、撤销后 release、slot 重用 generation 递增
* preflight 失败时 caller 保留 take 参数
* commit 后 operation error 仍消费 take 参数
* Host-owned string/buffer 使用正确 release function
* mock Native/Wasm Host 的调用、错误和 Drop trace 等价

拒绝或 conformance-fail 测试至少覆盖：

* 未知 function ID、major mismatch 和 ABI hash mismatch
* 未声明、未授予、scope 越界和已撤销 capability
* forged、stale、wrong-kind、wrong-rights 和 cross-instance handle
* Host 保存 borrow、重叠 `mut_borrow` 或未声明重入
* success status 搭配未初始化输出
* Host exception/panic 越过 wrapper
* 跨 allocator 释放和资源预算溢出

---

## 18. 验收标准

ADR-005 进入实现完成状态前必须满足：

1. Host IDL 可确定性生成 schema、stable ID、binding 和 ABI hash。
2. Loader 在用户代码执行前验证版本、函数、capability、hash 与预算。
3. Handle table 对 kind、generation、rights、owner 和 instance 全部执行验证。
4. ownership commit point 在成功、失败、撤销和关闭路径上都有 conformance trace。
5. Panic、平台异常和无效输出不能进入安全 Nexa 代码。
6. Native、Wasm 与 deterministic mock Host 对同一合同产生等价可观察行为。
7. accepted 与 rejected 测试覆盖 capability 缩减、stale handle 和 allocator 边界。

---

## 19. 最终决定

Nexa Host ABI 建立在：

```text
Canonical IDL
+ Stable Function IDs
+ Version and ABI Hash Negotiation
+ Least-authority Capabilities
+ Typed Generation Handles
+ Explicit Ownership Commit
+ Structured Failure
```

平台实现可以不同，但权限、资源生命周期、错误和稳定边界不能因 Host 而漂移。
