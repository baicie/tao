# ADR-008：WebAssembly、JavaScript FFI 与跨平台 UI Host

* **状态**：Accepted
* **日期**：2026-08-01
* **目标版本**：Nexa Language 2.0
* **关联 ADR**：

  * ADR-001：Nexa 2.0 语言架构、所有权与运行时模型
  * ADR-003：NIR、LLVM 后端边界与稳定 ABI
  * ADR-004：Allocator、Arena、集合与运行时内存布局
  * ADR-005：Host ABI、Capability 与资源 Handle
  * ADR-006：Result、Panic、defer 与跨 ABI 错误模型
  * ADR-007：Async 状态机、Structured Concurrency 与并发安全
* **实现优先级**：P1
* **兼容性范围**：不修改 Nexa Language 1.0 已冻结语义

---

## 1. 背景

WebAssembly 是 Nexa 的第二生产目标。浏览器与 WebView 同时是最现实的 JavaScript FFI
和跨平台 UI Host，但 JavaScript 对象、Promise、DOM、GC 与 exception 不能成为 Nexa 的语言语义。

ADR-003 已要求 Wasm 不暴露 Native pointer；ADR-005 定义版本化 Host IDL、capability 和 typed handle；
ADR-007 定义 async operation handle 与取消。本 ADR 冻结它们在 linear memory、JavaScript adapter、
事件循环和 UI patch protocol 上的映射。

目标不是在语言中内建浏览器 API，而是使相同 NIR 与组件合同能够连接 browser、WebView、
headless test Host 和后续 native renderer。

---

## 2. 决策摘要

1. 第一 Wasm 后端使用 `NIR -> LLVM Wasm target -> Wasm module`，后续可替换为独立后端。
2. Wasm 与 Native 共享 ownership、Drop、Result、panic、async、capability 和 verifier 语义。
3. 跨 Wasm 边界只使用 scalar、linear-memory descriptor、typed handle 和版本化 component type。
4. JavaScript binding 从 Host/component IDL 生成，不在安全代码中动态反射任意 JS object。
5. 同步 borrow 只在一次调用期间有效；异步 JS/Host 调用必须取得 owned copy 或 handle。
6. JS `BigInt` 映射 64-bit integer；不得用可能丢精度的 `Number` 静默转换。
7. JS exception/rejected Promise 在 adapter 内转换为结构化 Host error；Nexa panic 不转成普通 JS error 合同。
8. JS GC finalizer 只能作为泄漏兜底，资源正确性依赖显式 close/dispose 与 owned handle。
9. DOM/WebView 是第一 UI Host，不是语言语义或唯一 renderer。
10. UI 使用 typed node/resource ID、schema property ID、validated patch batch 和 batched event delivery。
11. safe UI API 不暴露裸 DOM node、任意 JavaScript object、raw HTML 或脚本执行。
12. UI handle 具有 main-thread affinity，默认不 `Send`/`Sync`。
13. patch batch 先完整验证后应用；validation failure 不产生部分 tree mutation。
14. 大型 binary/media/GPU 数据通过 owned resource handle 传递，不塞入通用 patch payload。
15. Browser 与 Native reference UI Host 必须对同一输入产生等价逻辑 tree、event、error 与 Drop trace。

---

## 3. Wasm target 与模块边界

首个生产配置为单实例、单 linear memory、无共享线程的 Wasm target。具体 wasm32 ABI、LLVM 版本和
内部 symbol 不稳定，稳定组件只承诺 ADR-003/005 的 component metadata 与 Host contract。

Wasm module 必须声明：

```text
component_abi_version
host_abi_range
required_import_ids
required_capabilities
memory_model
maximum_memory_pages
panic_strategy
oom_strategy
async_runtime_abi
export_abi_hashes
```

Loader 在实例化前验证 metadata、签名、imports、limits 与 capabilities。Start function 在验证完成前
不得执行用户代码或产生 Host side effect。

Wasm trap 表示实例级 invariant failure，默认终止该实例；它不能被普通 `try/catch` 捕获或伪装成
domain `Result.Err`。可恢复 bounds、Host 和 allocation failure 必须在 trap 前以显式 Result 表达。

---

## 4. Linear-memory descriptor

Wasm 内存 view 使用规范化概念 descriptor：

```text
MemoryView {
  memory_id: U32,
  offset: U64,
  byte_length: U64,
  element_type: AbiTypeId,
  element_count: U64,
  stride: U64,
  alignment: U32,
  access: Read | ReadWrite,
  lifetime: Call | Owned
}
```

当前 wasm32 adapter 验证 `offset` 和所有计算结果可表示为 32-bit memory index。使用 U64 的 schema
是为了统一 metadata 与未来 memory64，不表示当前 target 自动支持 memory64。

每次边界访问必须验证：

* `memory_id` 存在且属于当前实例。
* `offset + byte_length` 不溢出且在当前 memory bounds 内。
* `element_count * stride` 不溢出且不超过 byte length。
* offset 满足 alignment，stride 满足 element ABI。
* ReadWrite view 没有与 active borrow 冲突。
* UTF-8 descriptor 在构造安全 `String`/view 前通过验证。

JS `WebAssembly.Memory.grow` 可能使旧 `ArrayBuffer`/typed-array view 失效。Adapter 不能缓存 view
跨调用或跨 grow，必须在每次使用前从当前 memory 重新取得。

---

## 5. 内存所有权与复制

Wasm 边界遵循“谁分配谁释放”：

* Call-lifetime input view 由 caller 保持存活，callee 不得保存或释放。
* Owned input 在 ABI commit point 后转移给 callee。
* Owned output 携带 allocation/handle identity 和 release operation。
* Adapter 创建的临时 copy 在调用完成后由 adapter 清理。
* async call 不能保存 call-lifetime view；必须 copy 或 take owned allocation/handle。
* memory grow/reallocate 后旧 offset/view 只按其明确 ownership 状态处理。

String 使用 UTF-8 bytes；JS string 与 Nexa `String` 的基础适配需要一次编码或解码 copy。
短期优化可以使用 intern/owned string handle，但不得改变 Unicode、Drop 或 release 语义。

Typed array 的同步 read-only borrow 可以零拷贝；任何可能重入 JS、await、grow memory 或长期保存的路径
必须升级为 owned transfer/copy。Adapter 必须通过生成 schema 决定，不能按运行时猜测。

---

## 6. JavaScript 类型映射

Generated adapter 使用以下基础映射：

| Nexa/ABI 类型 | JavaScript 表示 | 约束 |
|---------------|-----------------|------|
| Bool | `boolean` | 只接受 true/false |
| U8/U16/U32/I8/I16/I32 | `number` | 验证整数与范围 |
| U64/I64 | `bigint` | 禁止静默 Number 转换 |
| F32/F64 | `number` | 保留既定 NaN/overflow 规则 |
| UTF-8 String | `string` | 边界编码/解码或 owned handle |
| Slice | TypedArray/DataView | 仅 schema 指定的调用生命周期 |
| Record | generated frozen object | 字段白名单、完整验证 |
| Tagged Union | `{ tag, value }` | tag 使用稳定 schema ID |
| Resource | generated class wrapper | 内含不可伪造 typed handle |
| Result | `{ ok, value/error }` | 不依赖 throw 控制流 |

`undefined`、`null`、symbol、function、prototype chain、Proxy 和任意 object 不会自动映射到安全 Nexa 类型。
Optional/nullable 必须在 IDL 中显式声明。

Adapter 对 object 只读取 schema 列出的 own data properties，拒绝 accessor/proxy side effect 进入安全转换。
需要动态 JS 的代码放在审计过的 unsafe adapter，并输出已验证的稳定类型。

---

## 7. JavaScript import/export 与错误

IDL 为 import 和 export 生成唯一 binding；Nexa core 不解析诸如 `window.localStorage` 的动态属性链。
平台 adapter 可以把该名称解析为实现，但 artifact 只依赖 stable module/function ID。

调用规则：

* JS import 在 adapter 内捕获 synchronous exception，并转成 declared Host error domain。
* rejected Promise 转成 async operation 的 structured error completion。
* 未声明的 exception shape 映射为 `HostError.Unknown`，message 脱敏。
* Nexa exported `Result` 基础 binding 返回 discriminated result object。
* ergonomic wrapper 可以选择 throw/reject，但不属于稳定 component ABI。
* Nexa panic/trap 终止或隔离 instance，adapter 只报告 `Panicked/Trapped` outcome。

异步 export 返回 Promise wrapper，但 Promise 只是 JavaScript 适配层。核心 task 仍服从 ADR-007 的 lazy、
single-consumer、structured cancellation 语义。`AbortSignal` 可以映射为 cancel request，不能恢复已取消 task。

---

## 8. JavaScript 资源 wrapper

每个 resource kind 生成封闭 wrapper：

```text
class FileHandle {
  private instance
  private typedGenerationHandle
  close()/dispose()
}
```

规则：

* Handle 数值不作为公共可写字段。
* 每次方法调用验证 instance、kind、generation、rights 和 state。
* `close`/`dispose` 幂等只指 wrapper 本身不重复调用 Host close；底层 close result 仍按合同返回。
* ownership transfer 后 wrapper 立即标记 consumed。
* `FinalizationRegistry` 可以报告或请求兜底 release，但不保证及时执行。
* capability 撤销后只允许 release/close。
* wrapper 不通过 prototype mutation 获得额外方法或 rights。

资源安全不能依赖浏览器何时运行 GC。

---

## 9. UI Host 定位

UI 是 Host capability，不是关键字、HIR 特例或固定 DOM 模型。

第一实现目标：

```text
Nexa application
-> versioned UI Host IDL
-> generated Wasm/JS adapter
-> DOM or WebView renderer
```

后续 native renderer 可以实现同一逻辑合同，但本 ADR 不承诺跨平台 pixel-perfect 输出。
平台字体、accessibility、输入法、窗口系统和 GPU 差异由 Host descriptor 暴露，不进入语言类型系统。

安全应用只持有：

* `WindowHandle`
* `UiRootHandle`
* `UiNodeId`
* `EventHandlerId`
* image/font/GPU 等 typed resource handle

它不持有 DOM Node 或任意 JS object。

---

## 10. UI tree 与 patch protocol

应用提交 versioned `UiPatchBatch`：

```text
UiPatchBatch {
  root,
  sequence,
  operations: [Create | SetProperty | Insert | Move | Remove | BindEvent],
  resource_refs,
  schema_version
}
```

Property、element 和 event 使用 schema ID，不使用 runtime 任意字符串属性写入。

Host 在应用前完整验证：

* root/node/resource handle 属于当前 instance 且 generation 有效。
* sequence 单调且不重复。
* parent/child 关系不会形成环或跨 root。
* property type、event payload 和 capability 匹配 schema。
* batch 大小、文本、深度和资源引用在预算内。
* URL、clipboard、camera 等敏感操作具有独立 capability。

Validation failure 不改变逻辑 tree。验证成功后 Host 按 operation 顺序应用；若平台 renderer 在 commit
中发生不可恢复失败，Host 隔离/重建 root 并返回结构化 renderer failure，而不向应用伪造部分成功。

Remove 会递归释放 Host tree ownership；应用持有的独立 resource handle 仍按自身 owner 规则关闭。

---

## 11. 事件与主线程

Host 将平台事件转换为 versioned、白名单 payload：

```text
UiEventBatch {
  root,
  sequence,
  events: [{ handler_id, event_type, payload }]
}
```

事件按 Host 已观察顺序进入 bounded channel。高频 move/scroll/input 事件可以按 schema 声明 coalescing，
但 click、submit、key transition 等离散事件不得静默合并。

规则：

* Handler 使用 stable ID，不跨 ABI 保存 Nexa closure pointer。
* stale/removed handler 事件被验证后丢弃并可记录计数。
* event payload 不包含 DOM object、JS function 或未经验证的 platform pointer。
* queue 满时按 event class 采用 coalesce、backpressure 或显式 overflow error。
* UI tree/handle main-thread affine，默认不 `Send`/`Sync`。
* worker task 通过 owned message/patch batch 与 UI task 交互。
* 事件 callback 进入 structured task scope，不能产生未管理 detached task。

---

## 12. UI 安全边界

Safe UI API 默认：

* 文本节点只写 text content，不解释 HTML。
* 不提供 raw HTML、eval、inline script 或任意 property escape hatch。
* URL 使用结构化类型并在 navigation/network capability scope 内验证。
* CSS/style 使用 schema property/value，不拼接未验证脚本文本。
* 文件、剪贴板、摄像头、麦克风和通知分别请求 capability。
* Event payload 在进入 Nexa memory 前进行长度、UTF-8、enum 和 range 验证。

需要 raw DOM/JS 的 library 必须位于 unsafe Host adapter，声明额外 capability，并把输出降级为已验证 descriptor。
unsafe 不自动获得权限，也不能把 JS object 伪装成 safe handle。

---

## 13. 性能与批处理基线

第一版本需要建立可回归的边界基线：

* 每个 render transaction 默认一次 patch Host call。
* 未改变属性不进入 patch。
* scalar/property ID 直接编码，不做字符串查找。
* 每个 changed text 最多一次 UTF-8 boundary copy。
* event 在一个 Host turn 内批量交付。
* 大型 byte/image/font 数据通过 resource handle 复用。
* adapter 不为每个 node 创建长期 JS closure。

具体 patch binary layout、batch 上限和 diff algorithm 可以演进，但必须记录：

* Host call 数
* copied bytes
* allocation 数
* patch/event queue 高水位
* apply/dispatch latency

性能优化不能跳过 schema、capability、generation 或 bounds 验证。

---

## 14. Component Model 与兼容性

Wasm Component Model 可以作为未来外部编码，但不能重新定义 Nexa 语义。

映射必须保留：

* owned/borrow/take
* resource handle kind 与 generation
* Result/error domain
* async/cancel
* capability requirement
* UTF-8 与 list ownership
* ABI version/hash

若 Canonical ABI 无法直接表达某项约束，生成 adapter/handle，而不是暴露 async frame 或 runtime object。
Component Model 版本升级不能自动改变现有 Nexa component ABI hash。

---

## 15. 诊断

本 ADR 使用 `E6000-E6099` 作为 Wasm、JS adapter 与 UI Host 诊断范围。

| 错误码 | 含义 |
|--------|------|
| `E6001` | linear-memory descriptor 越界、溢出或未对齐 |
| `E6002` | memory grow 后使用 stale view |
| `E6010` | JavaScript 值无法安全映射到 IDL 类型 |
| `E6011` | 64-bit integer 发生精度不安全转换 |
| `E6020` | async adapter 保存 call-lifetime borrow |
| `E6030` | resource wrapper stale、wrong-instance 或 consumed |
| `E6040` | UI patch schema、sequence 或 tree invariant 无效 |
| `E6041` | UI batch/event budget 超限 |
| `E6050` | thread-affine UI handle 被跨 worker 发送 |
| `E6060` | 缺少 UI/网络/设备 capability |

诊断必须提供 module/function/schema/node ID、expected/actual 类型、offset/length 和 capability 名称，
但不得泄漏 raw pointer、capability token、DOM object、用户文本或 JS stack 中的敏感值。

---

## 16. 被拒绝的方案

### 16.1 直接把 JavaScript 作为第二语言后端语义

这会让 GC、Promise、exception、number 和 object identity 反向定义 Nexa 行为。

### 16.2 Wasm 边界传递裸 pointer

Pointer 缺少 memory identity、length、ownership 与 lifetime，且不能安全跨实例或 memory grow。

### 16.3 用 JS GC 管理 Nexa resource

Finalizer 不及时、不确定，也无法可靠报告 close error。

### 16.4 把 DOM 写入做成编译器内建

这会耦合 parser/HIR 与浏览器，阻断 WebView、native renderer 和 headless Host。

### 16.5 每个 UI 属性一次 Host call

调用开销和重入使大型 UI 不可用，也难以验证 transaction 一致性。

### 16.6 Safe API 提供 raw HTML/任意 JS object

它绕过 schema、类型、capability 与注入防护，必须留在显式 unsafe adapter。

---

## 17. 后果

正面结果：

* Wasm 不成为另一套语言语义。
* Generated adapter 集中处理精度、UTF-8、exception 和 memory bounds。
* UI 可在 DOM/WebView 首发，同时保留 native/headless Host 路径。
* 批处理与 typed IDs 降低边界调用和字符串动态分派成本。
* capability、thread affinity 与 schema 验证形成清晰安全边界。

成本与限制：

* JS interop 不是任意对象的零配置调用，需要 IDL 与生成 binding。
* String 和复杂 object 通常需要 copy 或 handle。
* UI Host 需要 schema、patch verifier、event queue 和 renderer recovery。
* 第一版不承诺 shared-memory Wasm threads 或跨平台 pixel parity。

---

## 18. 分阶段实现

### Phase W0：Wasm Core

* LLVM wasm32 target
* module metadata 与 import verifier
* linear-memory descriptor/bounds checks
* panic/OOM/Drop conformance

### Phase W1：JavaScript Adapter

* IDL generated import/export
* scalar/String/record/union/Result mapping
* resource wrapper 与 explicit dispose
* exception/Promise error conversion

### Phase W2：Async Host

* owned operation handle
* Promise/AbortSignal bridge
* cancel/late completion cleanup
* structured scope integration

### Phase W3：UI Host

* DOM/WebView renderer
* typed patch batch 与 event batch
* main-thread task/channel
* capability、安全和 budget verifier

### Phase W4：Conformance 与优化

* Native reference/headless UI Host
* logical tree/event/Drop trace comparison
* copy/call/allocation baseline
* Component Model adapter experiment

---

## 19. 测试要求

接受测试至少覆盖：

* scalar、BigInt、UTF-8、record、union、Result 的往返
* empty/ZST slice、memory grow 与合法 aligned view
* sync zero-copy borrow 和 async owned copy/handle
* JS exception、Promise rejection、AbortSignal cancel 与 late completion
* resource close/consume/finalizer fallback 的一次 release
* UI create/update/move/remove、event ordering/coalescing 和 root teardown
* 同一程序在 Native reference Host 与 browser Host 的 output/error/Drop/Host-call trace
* patch batching 的 call/copy/allocation 性能基线

拒绝或 conformance-fail 测试至少覆盖：

* offset/length/stride 溢出、越界、wrong memory 和 stale view
* Number 到 I64/U64 的精度丢失
* accessor/proxy/unknown object 进入 safe binding
* async call 保存 borrowed TypedArray
* forged/stale/cross-instance resource wrapper
* UI tree cycle、wrong root、stale node、unknown property 和 sequence replay
* raw HTML/script、未授权 URL/device 和 UI handle 跨 worker

---

## 20. 验收标准

ADR-008 进入实现完成状态前必须满足：

1. Wasm module 在执行前完成 metadata、import、memory、capability 与 budget 验证。
2. Generated JS binding 覆盖所有稳定 IDL 类型且不依赖动态 object reflection。
3. 每个 borrow/owned memory descriptor 的生命周期和 release 路径可验证。
4. Promise/cancel/late completion 不产生重复 output owner 或资源泄漏。
5. UI patch 在完整验证后批量提交，事件通过 bounded typed channel 返回。
6. safe UI surface 不暴露 DOM/JS/raw HTML escape hatch。
7. Native 与 browser Host 的逻辑 tree、Result、Drop 和 call trace conformance 通过。

---

## 21. 最终决定

Nexa 的 Web 与 UI 路线为：

```text
NIR
-> Wasm with verified linear-memory descriptors
-> generated JavaScript Host adapter
-> versioned typed UI patch/event protocol
-> DOM/WebView first, other renderers later
```

JavaScript 和 DOM 是 Host 实现，不是 Nexa 语言的内存、错误、异步或对象模型。
