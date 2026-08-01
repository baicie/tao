# ADR-007：Async 状态机、Structured Concurrency 与并发安全

* **状态**：Accepted
* **日期**：2026-08-01
* **目标版本**：Nexa Language 2.0
* **关联 ADR**：

  * ADR-001：Nexa 2.0 语言架构、所有权与运行时模型
  * ADR-002：所有权、借用、Move 与确定性 Drop 语义
  * ADR-003：NIR、LLVM 后端边界与稳定 ABI
  * ADR-004：Allocator、Arena、集合与运行时内存布局
  * ADR-005：Host ABI、Capability 与资源 Handle
  * ADR-006：Result、Panic、defer 与跨 ABI 错误模型
* **实现优先级**：P1
* **兼容性范围**：不修改 Nexa Language 1.0 已冻结语义

---

## 1. 背景

Nexa 保留 TypeScript-shaped `async/await`，但不采用 JavaScript Promise、microtask queue
或隐式共享对象语义。ADR-002 禁止普通 borrow 跨 `await`，ADR-003 要求 async frame
保持内部不稳定，ADR-006 规定取消必须复用 cleanup plan。

仍需决定 task 何时启动、谁拥有 task、父作用域何时可以退出、取消如何清理、跨线程值需要
满足什么能力，以及 channel、deadline 和 Host async 如何避免无界队列和 detached task 泄漏。

本 ADR 优先定义单线程事件循环和结构化并发语义；多线程 worker、actor 和共享内存 runtime
必须在不改变这些语义的前提下扩展。

---

## 2. 决策摘要

1. `async` 函数降低为拥有其跨暂停状态的静态状态机。
2. 调用 async 函数创建 lazy、Move-only task；由 `await` 或 task scope 的 `spawn` 开始调度。
3. 普通 borrow、mutable borrow、slice、mutex guard 和 Arena borrow 不得跨 `await`。
4. 跨暂停点只保存 Owned、Copy、Shared、static data 或 runtime 明确认可的 owned handle。
5. 默认并发是 structured concurrency：父 scope 不能带着 live child 退出。
6. 正常退出等待 children；error、cancel 或 panic 隔离会取消 children 并等待其 cleanup 完成。
7. 取消是独立控制 outcome，不伪装成用户错误类型，也不由普通 catch 处理。
8. 取消是 cooperative，但 task Drop 必须能在不恢复用户 body 的情况下同步执行 frame cleanup。
9. `Task<T, E>` 的完成结果只能被消费一次；共享观察必须使用显式 shared task primitive。
10. 调度器不能并发 poll 同一 task；wake 可以合并但不能丢失从 Pending 到 Ready 的转换。
11. 跨线程 Move 要求 `Send`；跨线程 shared borrow 要求 `Sync`。
12. 默认 channel 有界并提供 backpressure；unbounded channel 必须显式选择并受预算限制。
13. deadline/timeout 依赖显式 time capability，不存在 ambient clock。
14. async frame、waker 和 scheduler ABI 不稳定；跨 Host 使用 stable task/operation handle。
15. Native、Wasm 和 deterministic scheduler 必须共享状态、取消、Drop 与 Result 语义。

---

## 3. Task 创建与启动

调用：

```text
const task = fetchData(request);
```

只完成：

* 对参数执行签名规定的 Copy/Borrow/Move 检查
* Move task 所需 owned 参数进入 frame
* 创建 `Created` 状态

它不保证立即执行函数 body。task 在以下操作之一发生时进入调度：

* `await task`
* structured scope 的 `spawn(task)`
* 显式 runtime supervisor 接受 task ownership

lazy start 避免“构造值即产生隐藏并发”。如果 task 从未启动就被 Drop，runtime 直接执行 frame
中已拥有值的 cleanup，不运行用户 body。

`Task<T, E>` 是 Move-only。普通赋值或传参转移 ownership，不增加观察者。已经 await/join
并取出 outcome 的 task 进入 `Consumed`，不能再次 await。

---

## 4. Async frame 与 lowering

概念 frame：

```text
AsyncFrame {
  state_tag,
  cancellation_state,
  owned_parameters,
  live_locals,
  initialized_places,
  child_scope,
  awaited_operation,
  result_slot,
  cleanup_plan
}
```

私有字段顺序、state tag 数值、frame allocation 和 waker 表示不属于语言或稳定 ABI。

Lowering 必须在每个 suspension point 确定：

* resume state
* 跨暂停存活的 owned places
* 已注册 defer 与 Drop action
* 当前 child scope
* awaited task/Host operation ownership
* cancel continuation
* completion/result slot

局部值如果在 `await` 后不再使用，应在暂停前按普通最后使用和 Drop 规则结束，避免无谓扩大 frame。
后端可以压缩 frame，但必须保持 cleanup trace 与状态验证等价。

---

## 5. Task 状态机

规范状态为：

```text
Created -> Ready -> Running -> Pending -> Ready
                    |            |
                    |            +-> Cancelling
                    +-> Completed(Ok | Err)
                    +-> Cancelling -> Cancelled
                    +-> Panicked(isolated runtime only)

Completed/Cancelled/Panicked -> Consumed -> Released
```

不变量：

* 同一 task 同时最多有一个 poll/resume 执行者。
* 只有 `Running` 可以修改 frame 的用户状态。
* 返回 Pending 前，awaited operation 必须已注册 wake path 或再次检查 ready，避免 lost wakeup。
* result slot 只提交一次。
* terminal state 不得恢复执行用户 body。
* frame 只在 terminal outcome 已处理且所有 owned state 已清理后释放。
* stale task handle 通过 generation 检查拒绝。

重复 wake 可以合并；重复 completion、并发 poll 和 terminal-state wake 是 runtime invariant failure。

---

## 6. Borrow 与暂停点

普通 borrow 不得跨 `await`，包括间接保存在：

* local/temporary
* closure capture
* slice/view
* iterator state
* mutex/rwlock guard
* Arena reference/handle borrow
* Host call borrowed descriptor

编译器以 suspension liveness 分析判断，而不是仅检查源码行距。

允许跨 `await`：

* frame 自己拥有的 Move value
* Copy value
* `Shared<T>`，满足所在 executor 的线程能力
* immutable static data
* 明确声明 async-safe 的 owned Host handle

解决 borrow 错误的方式是缩短作用域、提前复制小值、Move ownership、显式 Clone/Shared，
而不是自动把 borrow 提升为 Shared 或堆分配。

---

## 7. Structured concurrency

所有 spawn 默认属于一个词法或 runtime task scope。scope 为每个 child 分配稳定的创建序号，
并持有 child ownership 直到 join 完成。

scope 退出规则：

* 正常路径：停止接受新 child，等待所有 child terminal，再按创建序号返回 outcomes。
* 父级 `Err`/return：请求取消所有未完成 child，等待 cleanup，然后保留父级原 outcome。
* 父级取消：向 children 传播取消，等待 cleanup，父级最终为 Cancelled。
* isolated panic：取消并清理同一隔离 scope；panic report 交给 supervisor。

默认 API 不允许 fire-and-forget。确需长生命周期后台任务时，必须把 task Move 给显式 `Supervisor`：

* Supervisor 具有独立生命周期和资源预算。
* 它必须提供 shutdown/cancel/join。
* Host/application 退出前必须关闭 supervisor。
* library 不能静默创建进程级 detached task。

多个 child outcome 的集合按创建序号稳定排序，不依赖完成顺序。显式 `race` 或 completion-order API
可以观察调度竞争，但其非确定性必须在类型/API 名称和 Deterministic Profile 限制中可见。

---

## 8. 取消

取消是 cooperative control signal。检查点包括：

* `await`
* channel send/receive
* task scope join
* async Host operation
* 显式 `checkCancellation()`

runtime 在检查点观察到请求后：

```text
stop entering new user statements
-> cancel owned child/operation handles
-> run ADR-006 cleanup plan in reverse order
-> commit Cancelled outcome
```

规则：

* cleanup 中的 Drop/defer 是同步、不可 await 的。
* 取消不能打断正在执行的普通同步表达式；CPU-bound code 必须显式检查。
* Drop 一个 pending task 等价于请求取消并完成 frame cleanup，不允许遗留 owned state。
* cancel 与 normal completion 竞争时只能提交一个 terminal outcome。
* operation 已提交的外部副作用不会回滚；API 必须通过 domain protocol 表达幂等性。
* 取消不是 `Err(Cancelled)`，除非某个 library 显式选择把它转换为 domain error。

异步资源 release 必须拆成显式 async close；Drop 仍只能执行不等待的兜底 release/cancel。

---

## 9. Result、panic 与 task outcome

Async 函数声明的 `Result<T, E>` 在成功执行 body 后成为：

```text
Completed(Ok(T)) | Completed(Err(E))
```

runtime 层还可能产生：

```text
Cancelled
Panicked(report)  // only across an isolation boundary
```

`await` 在当前 task 被取消时传播取消；在 child 完成时提取其 `Result`。需要观察 runtime outcome 的
supervisor/join API 使用显式 `JoinOutcome<T, E>`，不会把 panic 自动转换成用户的 `E`。

默认 panic-abort 会终止进程或 Wasm instance；只有隔离 runner 能报告 `Panicked`。
普通 `try/catch` 不捕获 Cancelled 或 Panicked。

---

## 10. Scheduler 合同

第一阶段 runtime 使用单线程 cooperative executor。

Scheduler 必须保证：

* ready task 最终获得 poll，除非其 scope 已取消或 runtime shutdown。
* 同一 task 不并发 poll。
* wake registration 与 Pending 提交不存在 lost wakeup。
* 单次 poll 有可配置预算，长期同步循环需要显式 yield/checkpoint。
* task queue、timer 和 Host completions 受实例资源预算约束。
* shutdown 停止接收新 task，取消 scopes，并完成所有同步 cleanup。

核心语言不承诺 ready queue 的具体公平算法或真实并发完成顺序。Deterministic scheduler 使用
稳定 task 序号、虚拟时钟和可记录 Host completion 序列，用于 conformance 和 replay。

多线程 executor 可以后续加入，但不得改变单 task 状态机、Result、取消和 Drop 语义。

---

## 11. `Send` 与 `Sync`

`Send` 表示值可以通过 Move 跨 worker/thread；`Sync` 表示对值的 shared borrow 可以跨线程使用。
它们是编译器推导的安全能力，不要求复制 Rust trait surface。

基本规则：

* 只包含 `Send` 字段的 owned aggregate 自动 `Send`。
* 包含 thread-affine Host handle、borrow、Arena reference 或非线程安全 runtime pointer 的类型不 `Send`。
* `Shared<T>` 跨线程要求控制块线程安全且 `T: Sync`。
* immutable pure value 通常 `Sync`。
* `Cell<T>`、UI handle 和 thread-local resource 默认不 `Sync`。
* `Mutex<T>` 在 `T: Send` 且 runtime 实现满足合同时可以 `Send + Sync`。
* mutex/rwlock guard 不 `Send` 且不能跨 `await`。
* `unsafe` 手动声明能力必须列出并验证 safety invariants。

单线程 async task 不自动要求 `Send`；只有可能迁移到 worker 的 executor/scope 才要求整个 frame `Send`。

---

## 12. Channel、Actor 与共享状态

最小并发原语按以下顺序提供：

```text
bounded Channel<T>
-> worker/Actor mailbox
-> thread-safe Mutex/RwLock/Atomic
```

Channel 规则：

* send Move `T` 进入 channel；成功后 sender 不再拥有。
* bounded channel 满时 send Pending，形成 backpressure。
* receiver 取得唯一 ownership。
* 最后 sender Drop 后，receiver 在 drain 完成后观察 Closed。
* receiver 关闭后，pending sender 恢复并得到显式 closed outcome。
* 取消 pending send 时，如果 ownership 已提交，outcome 必须明确返还或处理 `T`，不能重复 owner。
* unbounded channel 必须显式命名、设置 budget，并在超限时返回错误。

Actor 独占其 mutable state，通过 owned message 交互。共享内存修改只通过显式同步类型，
安全代码不允许普通 mutable alias 跨 task/thread。

---

## 13. Deadline、timeout 与 Host async

Deadline/timeout 需要 `time` capability。Deterministic Host 使用虚拟时钟；没有 time capability
的程序不能隐式读取 wall clock 或创建 timer。

Async Host 调用使用 owned operation handle：

```text
start operation with owned inputs
-> Pending(operation handle)
-> Host completion wakes task
-> validate output
-> commit Complete | Cancelled
```

同步 borrow 不得保存在 operation handle 中。Host 必须 copy 输入或 take owned buffer/handle。

取消 operation 时：

* runtime 发送一次 cancel request。
* completion 与 cancel 使用 generation/state CAS 选择唯一结果。
* late completion 的 owned output 由 Host adapter 释放。
* capability 撤销阻止新 operation，并取消或隔离已有 operation。
* operation handle 的 close 在撤销后仍可执行。

Wasm/JavaScript promise adapter 的具体映射由 ADR-008 定义。

---

## 14. 稳定边界

以下保持私有：

* AsyncFrame 布局与 state tag
* poll function signature
* waker、ready queue 与 timer wheel
* executor object 与 thread pool
* channel ring-buffer layout

稳定 Host/component ABI 只暴露：

* versioned async function schema
* typed task/operation/stream handle
* owned input/output descriptors
* poll/subscribe/cancel/close 操作
* Result/error/capability metadata

Artifact 必须声明 async runtime ABI major、required Host functions、threading model 和 panic strategy。
Loader 在执行前拒绝不兼容 executor/Host。

---

## 15. 诊断与安全

本 ADR 使用 `E5900-E5999` 作为 async 与并发安全诊断范围。

| 错误码 | 含义 |
|--------|------|
| `E5901` | 普通 borrow 跨 `await` |
| `E5902` | async frame 包含不允许的 Arena/guard borrow |
| `E5910` | task 未 join、cancel 或移交 supervisor |
| `E5911` | task outcome 被重复消费 |
| `E5920` | 非 `Send` 值移入 worker scope |
| `E5921` | 非 `Sync` 值被跨线程 shared borrow |
| `E5930` | defer/Drop 在取消 cleanup 中尝试 await |
| `E5940` | Host async operation 保存同步 borrow |
| `E5950` | channel/queue 缺少边界或预算 |
| `E5960` | task/operation handle stale 或状态转换非法 |

诊断必须给出 borrow 创建点、suspension point、跨线程边界和 task scope 退出点。
Scheduler/Host 输入是不可信边界；generation、状态、长度、waker 与 completion 必须验证，
且 trace 不记录任务 payload、capability token 或敏感 Host 数据。

---

## 16. 被拒绝的方案

### 16.1 JavaScript Promise 作为语言语义

Promise 的 eager execution、microtask ordering、隐式共享和异常传播与 Nexa 的 ownership/Result 不一致。

### 16.2 默认 detached task

它让资源、错误、取消和 shutdown 失去 owner，是长期泄漏和隐藏工作的来源。

### 16.3 自动把跨 await borrow 提升为 Shared

这会引入隐藏 allocation/refcount，并把程序员没有选择的共享身份变成语义。

### 16.4 取消直接中断任意指令

异步抢占可能在不变量更新一半时执行 cleanup，无法保证安全 Drop。

### 16.5 默认无界 channel

它把 producer/consumer 速率差转换为不可控内存增长。

### 16.6 在稳定 ABI 暴露 poll/frame 布局

这会冻结 executor 与编译器 lowering，阻止 Native/Wasm 使用不同实现。

---

## 17. 后果

正面结果：

* task、child、Host operation 和资源都有明确 owner。
* 取消沿正常 cleanup plan 执行，不引入第二套析构语义。
* 单线程、worker 和 Wasm 可以共享相同 async 状态机合同。
* bounded channel 与 budget 默认限制内存压力。
* `Send`/`Sync` 防止安全代码产生跨线程悬空引用和数据竞争。

成本与限制：

* lazy task 与 JavaScript Promise 行为不同，需要工具和文档明确提示。
* 默认禁止 detached task，后台服务必须显式管理 supervisor。
* CPU-bound task 需要 cooperative checkpoint。
* async close 必须显式 await，不能依赖 Drop。
* executor、Host adapter 与 verifier 需要维护较严格的状态机。

---

## 18. 分阶段实现

### Phase A0：State Machine

* async HIR/MIR 与 suspension liveness
* owned frame、result slot 和 poll verifier
* lazy Task 与 single-consumer await
* borrow-across-await diagnostics

### Phase A1：Executor 与取消

* 单线程 ready queue/waker
* cooperative cancellation
* frame cleanup 与 task Drop
* deterministic scheduler

### Phase A2：Structured Scope

* spawn/join/cancel tree
* stable child ordering
* supervisor 与 shutdown
* task/resource budgets

### Phase A3：Channel 与 Host Async

* bounded channel/backpressure
* owned operation handle
* deadline/virtual time
* completion/cancel race verifier

### Phase A4：Worker 与线程能力

* `Send`/`Sync` structural inference
* worker/Actor
* thread-safe Shared/Mutex/Atomic
* cross-thread stress and model tests

---

## 19. 测试要求

接受测试至少覆盖：

* lazy creation、await start、spawn start 和 unstarted task Drop
* state machine 每个 suspension/resume 与一次 result commit
* owned/Copy/Shared 值跨多个 await
* nested task scope 正常 join、父 error、取消和 shutdown
* defer/Drop 在 cancel 时的严格逆序 trace
* cancel/complete、wake/Pending 和 Host late-completion 竞争
* bounded channel backpressure、close 和取消时 ownership
* single-thread 与 deterministic scheduler 的等价结果

拒绝或 compile/conformance-fail 测试至少覆盖：

* borrow、slice、Arena value 或 guard 跨 await
* 未处理 child/detached library task
* task outcome 重复消费或 terminal task 再 poll
* 非 Send frame 移入 worker、非 Sync shared borrow
* defer/Drop await
* Host operation 保存 borrowed input
* unbounded queue 无预算、stale handle 和重复 completion

并发 race 测试必须可通过 deterministic schedule replay 重现；真实线程 stress test 作为补充。

---

## 20. 验收标准

ADR-007 进入实现完成状态前必须满足：

1. 所有 async 函数降低为可验证的 owned frame，且 frame 不含普通 borrow。
2. task 创建、启动、poll、完成、取消、消费和释放状态转换均被 verifier 覆盖。
3. 每个 structured scope 在退出前处理所有 child，无 orphan resource。
4. cancel、Result early return 与 normal completion 共享 ADR-006 cleanup plan。
5. channel 和 Host async 在取消竞争中保持唯一 ownership。
6. `Send`/`Sync` compile-fail tests 阻止跨线程数据竞争。
7. reference、Native、Wasm 和 deterministic scheduler 的 outcome/Drop/Host trace 可比较。

---

## 21. 最终决定

Nexa 2.0 的异步与并发模型为：

```text
Lazy Owned Task
+ Static Async State Machine
+ No Borrow Across Await
+ Structured Task Ownership
+ Cooperative Cancellation
+ Deterministic Cleanup
+ Bounded Communication
+ Explicit Send/Sync
```

`async/await` 保留熟悉的表面语法，但执行、错误、取消和共享均服从 Nexa 的所有权模型。
