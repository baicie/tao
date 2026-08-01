# Architecture Decision Records

本目录记录 Futao（原 Nexa）的架构决策及其演进关系。ADR 记录的是决策约束，不代表对应实现已经交付。

当前 Nexa Language 1.0 规范仍是兼容性基线。ADR-001 至 ADR-009 描述 Nexa 2.0 的目标架构；在相关实现和规格完成前，不应据此宣称 2.0 能力已经可用。

## Decision Log

| ADR | Status | Scope |
|-----|--------|-------|
| [ADR-000](000-bootstrap-compiler.md) | Accepted | 渐进式混合自举、内部 NIR 边界、Bootstrap Profile 与可复现 Stage 产物 |
| [ADR-001](001-language-architecture-ownership-runtime.md) | Accepted | Nexa 2.0 语言定位、所有权模型、运行时与后端总边界 |
| [ADR-002](002-ownership-borrowing-move-drop.md) | Accepted | Copy、Move、Borrow、`mut`、`take`、Drop、Shared 与 Weak 的精确定义 |
| [ADR-003](003-nir-llvm-backend-stable-abi.md) | Accepted | MIR/NIR 分工、LLVM 隔离、Verifier 与稳定 ABI 边界 |
| [ADR-004](004-runtime-memory-layout.md) | Accepted | Allocator provenance、Arena、集合布局、OOM 与 Drop glue |
| [ADR-005](005-host-abi-capabilities-handles.md) | Accepted | Host IDL、Capability、版本协商与 typed resource handle |
| [ADR-006](006-errors-panic-defer-abi.md) | Accepted | Result、`?`、`try/catch`、cleanup plan、panic 与跨 ABI 失败 |
| [ADR-007](007-async-structured-concurrency.md) | Accepted | Async state machine、structured concurrency、取消与 `Send`/`Sync` |
| [ADR-008](008-wasm-js-ffi-ui-host.md) | Accepted | Wasm memory descriptor、JavaScript adapter 与跨平台 UI Host |
| [ADR-009](009-package-lockfile-artifacts-signing.md) | Accepted | Package、lockfile、稳定组件、签名、provenance 与可复现构建 |
| [ADR-010](010-futao-language-name.md) | Accepted | Futao 正式名称、`.ft`、工具链标识与历史兼容迁移边界 |
| [ADR-011](011-toolchain-versioning-and-self-hosting-gate.md) | Accepted | `0.0.x` 自举迭代、`0.1.0` 发布门槛与 `mvp` 合并治理 |

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
    v
ADR-004 runtime memory layout
    |
    v
ADR-005 Host ABI and resource handles
    |
    v
ADR-006 errors, panic, defer, and ABI failure
    |
    v
ADR-007 async and concurrency
    |
    v
ADR-008 Wasm, JavaScript FFI, and UI Host
    |
    v
ADR-009 package, lockfile, component artifact, and signing
```

## Implementation Order

后续实现必须按依赖顺序推进，而不是把 Accepted 状态解释为已交付：

1. [ADR-004](004-runtime-memory-layout.md)：先建立可验证布局、allocator provenance 与 Drop glue。
2. [ADR-005](005-host-abi-capabilities-handles.md)：在稳定 descriptor 上实现 Host ABI 与 capability。
3. [ADR-006](006-errors-panic-defer-abi.md)：统一 Result、cleanup plan 与 panic 边界。
4. [ADR-007](007-async-structured-concurrency.md)：让 task、取消与并发安全复用 ownership/cleanup 合同。
5. [ADR-008](008-wasm-js-ffi-ui-host.md)：在 Host/async 合同上实现 Wasm、JS 与 UI adapter。
6. [ADR-009](009-package-lockfile-artifacts-signing.md)：最后冻结分发、组件加载、签名与 provenance 链路。

每一阶段都需要对应的 accepted、rejected/compile-fail、跨后端 conformance 和安全边界测试，才能从“已接受设计”进入“已实现能力”。

ADR-010 是横跨上述阶段的命名决策，不改变 ADR-004 至 ADR-009 的技术依赖顺序。历史文档中的
Nexa 标识保留用于追溯；新的 2.0 示例和后续规范使用 Futao 标识。

ADR-011 定义自举工具链的发布门槛。ADR-004 至 ADR-009 是完整平台能力的依赖顺序，
不是要求在 `0.1.0` 前一次交付的 release critical path；自举只实现其中被 Bootstrap
Profile 实际依赖的最小子集。逐版本交付见
[Futao 0.1.0 自举实施计划](../implementation/self-hosting-0.1.0.md)。
