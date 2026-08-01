# Architecture Decision Records

本目录记录 Nexa 的架构决策及其演进关系。ADR 记录的是决策约束，不代表对应实现已经交付。

当前 Nexa Language 1.0 规范仍是兼容性基线。ADR-001 至 ADR-003 描述 Nexa 2.0 的目标架构；在相关实现和规格完成前，不应据此宣称 2.0 能力已经可用。

## Decision Log

| ADR | Status | Scope |
|-----|--------|-------|
| [ADR-000](000-bootstrap-compiler.md) | Proposed | 渐进式混合自举、Bootstrap Profile 与可复现 Stage 产物 |
| [ADR-001](001-language-architecture-ownership-runtime.md) | Accepted | Nexa 2.0 语言定位、所有权模型、运行时与后端总边界 |
| [ADR-002](002-ownership-borrowing-move-drop.md) | Accepted | Copy、Move、Borrow、`mut`、`take`、Drop、Shared 与 Weak 的精确定义 |
| [ADR-003](003-nir-llvm-backend-stable-abi.md) | Accepted | MIR/NIR 分工、LLVM 隔离、Verifier 与稳定 ABI 边界 |

## Dependency Order

```text
ADR-000 bootstrap route
    |
    v
ADR-001 language and runtime architecture
    |
    v
ADR-002 ownership, borrowing, and Drop
    |
    v
ADR-003 NIR, LLVM, and ABI boundaries
    |
    +--> ADR-004 runtime memory layout
    |        |
    |        +--> ADR-005 Host ABI and resource handles
    |        |
    |        +--> ADR-006 errors, panic, defer, and ABI failure
    |                 |
    |                 +--> ADR-007 async and concurrency
    |                           |
    |                           +--> ADR-008 Wasm, JavaScript FFI, and UI Host
    |
    +--> ADR-009 package, lockfile, component artifact, and signing
```

## Next Decisions

### ADR-004: Allocator, Arena, Vec, String, and Runtime Memory Layout

这是下一份必须完成的 ADR。它应冻结所有权规则落到运行时表示后的最小契约，包括 allocator provenance、`String`/`Vec<T>` 的长度与容量不变量、arena 值是否可逃逸、zero-sized type、alignment、out-of-memory 行为和 Drop glue 输入。它还必须区分内部可变布局与稳定 ABI descriptor，避免把优化实现误当公开 ABI。

验收时应提供布局表、allocator 跨边界规则、失败路径、MIR 到 NIR 的 lowering 示例，以及 `String`、`Vec<T>`、`Box<T>`、`Shared<T>` 的 Drop/clone/move conformance cases。

### ADR-005: Host ABI, Capability, and Resource Handles

在 ADR-003/004 之后定义版本化 Host ABI。核心决策包括 capability 的声明与授予、owned/borrowed handle 模式、host call 的同步边界、句柄代数、版本协商、ABI hash、资源泄漏检测和撤销行为。文件、网络、时间、窗口等宿主能力都应通过该边界提供，不进入 parser、HIR 或 MIR 的特殊分支。

验收时应包含 host descriptor schema、版本不兼容诊断、handle 生命周期状态机、mock host conformance，以及 C/Wasm 两种宿主的等价调用轨迹。

### ADR-006: Result, Panic, defer, and Cross-ABI Failure

冻结可恢复错误与不可恢复失败的边界：`Result<T, E>`、`?`、`try/catch`、panic、abort、`defer`、Drop 的执行顺序，以及它们穿过函数、线程、Host ABI 和 C ABI 时的行为。默认不得让 panic 穿越稳定 ABI，也不能依赖 LLVM 或平台异常栈展开来定义语言语义。

验收时应给出正常返回、早退、嵌套 defer、部分初始化、Drop 中失败、host failure 和 panic 的规范化控制流及 Drop trace。

### ADR-007: Async State Machines, Structured Concurrency, and Concurrency Safety

在所有权与失败模型稳定后，定义 `async/await` 的状态机 lowering、跨暂停点 owned state、取消、task tree、join、deadline、backpressure、`Send`/`Sync` 等能力边界，以及 actor/channel/mutex 的最小集合。普通 Borrow 不得跨越 `await`，取消必须触发确定的清理路径。

验收时应提供状态机布局、不允许跨 `await` 的 borrow 测试、取消 Drop trace、并发安全 compile-fail cases 和 deterministic scheduler conformance。

### ADR-008: Wasm, JavaScript FFI, and Cross-Platform UI Host

基于 Host ABI 定义 Wasm linear-memory descriptor、JavaScript adapter、事件批处理、异步 host call、字符串/数组所有权和 UI capability。DOM/WebView 应作为第一个 UI Host，而不是语言语义；UI tree、renderer contract 和 platform view 仍需保持后端中立。

验收时应实现同一程序在 native reference host 与 browser host 上的输出、错误、Drop 和 host-call trace 对比，并建立 ABI 边界的拷贝次数与 patch batching 基线。

### ADR-009: Package Format, Lockfile, Stable Component Artifacts, and Signing

定义 source package、compiler artifact 和 stable component artifact 的区别；冻结 package identity、semantic version/edition、target/profile、feature/capability declaration、lockfile resolution、content hash、signature、provenance 和 reproducible build metadata。内部 NIR/MIR 不应因打包需求被错误冻结为永久公共格式。

验收时应覆盖离线解析、锁文件确定性、依赖替换攻击、签名验证、跨目标 artifact 选择、ABI hash 不兼容和 Stage 产物复现。
