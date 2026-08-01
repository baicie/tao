# ADR-011：工具链版本与自举发布门槛

* **状态**：Accepted
* **日期**：2026-08-01
* **决策范围**：Futao 编译器工具链版本、默认开发分支、发布与合并流程
* **关联 ADR**：ADR-000、ADR-003、ADR-009、ADR-010

## 背景

仓库已经交付 Rust 编写的 Nexa Language 1.0 参考实现。本 ADR 接受时 Cargo 工具链版本为
`0.0.1`，该版本随后固定为 Stage 0。语言正式名称已经由 ADR-010 确定为 Futao，但编译器核心仍由 Rust
实现，自举产物边界也尚未在 ADR-000 与 ADR-003 之间完成统一。

如果仅按功能数量提升版本，`0.1.0` 将无法表达“语言足以实现并编译自身”这一关键
工程边界。默认分支切换到 `mvp` 后，也需要统一 PR、CI 和发布来源，避免变更绕过
同一条验证链路。

## 决策

### 工具链版本

1. Rust bootstrap/reference baseline 固定为 `0.0.1`；自举里程碑从 `0.0.2` 递增。
2. 自举实现期间只发布 `0.0.x`，并满足 `0.0.1 <= version < 0.1.0`。
3. 小版本号表示已通过门槛的工具链里程碑，不承诺稳定公共 API。
4. 不预占全部版本号；计划中的版本可以在范围不变时拆分，但不得跳过验收门槛。
5. 只有本 ADR 的 `0.1.0` 门槛全部通过，发布 PR 才能将工具链提升到 `0.1.0`。
6. 应用包自己的 `0.1.0` 版本不受此约束；本 ADR 仅约束 Futao 编译器工具链。
7. 在自举 gate 尚未实现前，release workflow 必须拒绝 `0.0.x` 范围外的工具链；
   `0.1.0` 发布 PR 必须以真实的自动化自举 gate 替换该临时限制，不能只删除检查。

### `0.1.0` 的含义

`0.1.0` 表示完成可复现的混合自举：

```text
C0 = 固定并可重建的 Rust Stage 0
C1 = C0(Futao compiler source)
C2 = C1(Futao compiler source)
C3 = C2(Futao compiler source)

normalize(C2) == normalize(C3)
```

进入 `0.1.0` 必须同时满足：

* Lexer、Parser、Resolver、Type Checker、HIR/MIR/NIR lowering 和 compiler driver
  core 使用 Futao 编写。
* `C0 -> C1 -> C2 -> C3` 在 CI 中全自动执行。
* C2 与 C3 的规范化、目标无关编译器产物完全一致。
* Rust 与 Futao 编译器 differential suite 不存在未解释差异。
* Linux、macOS 和 Windows 生成相同的目标无关产物。
* 自托管编译器能编译自身、Bootstrap Stdlib 和仓库中的真实示例。
* 产物经过独立 Rust verifier，资源上限和结构化诊断仍有效。
* Stage 0 来源、编译器输入、Bootstrap Stdlib、schema 与输出哈希写入 manifest。
* 至少两个连续的 `0.0.x` release candidate 通过同一完整门槛。
* 自托管编译器成为默认实现时，Rust 编译器仍保留为可测试、可回滚的 fallback。

Runtime、allocator、artifact verifier、LLVM/Wasm backend、Host adapter 和密码学验证
初期可以继续使用 Rust。混合边界不阻止进入 `0.1.0`，但必须被版本化、验证且不向
编译器核心泄漏平台私有语义。

### 分支与合并

1. `mvp` 是默认开发、CI、发布和 PR 基准分支。
2. 不直接向 `mvp` 推送功能、修复或文档变更。
3. 每项工作从最新 `mvp` 创建独立分支；自动化分支使用 `codex/` 前缀。
4. PR 必须明确以 `mvp` 为 base，且只包含一个可独立验证的交付切片。
5. 所有 required checks 和审查完成后使用 squash merge，并删除已合并远程分支。
6. 发布 tag 必须指向 `mvp` 上的提交，且与工具链 Cargo 版本完全一致。

## 后果

* `0.1.0` 成为可执行的工程门槛，而不是日期或功能数量目标。
* 自举期间可以继续发布有用的 `0.0.x` 工具链，但不能提前宣称自托管完成。
* ADR-004 至 ADR-009 仍按内存布局、Host ABI、错误模型、Async、Wasm/UI Host、包与
  签名的依赖顺序推进；`0.1.0` 只要求自举所需子集，不要求一次交付全部平台能力。
* ADR-000 必须先与 ADR-003 的内部 NIR 边界对齐并被接受，之后才能冻结自举产物。

## 实施

逐版本交付、验收、回滚和 PR 切片记录在
[《Futao 0.1.0 自举实施计划》](../implementation/self-hosting-0.1.0.md)。
