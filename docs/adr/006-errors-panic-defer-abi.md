# ADR-006：Result、Panic、defer 与跨 ABI 错误模型

* **状态**：Accepted
* **日期**：2026-08-01
* **目标版本**：Nexa Language 2.0
* **关联 ADR**：

  * ADR-001：Nexa 2.0 语言架构、所有权与运行时模型
  * ADR-002：所有权、借用、Move 与确定性 Drop 语义
  * ADR-003：NIR、LLVM 后端边界与稳定 ABI
  * ADR-004：Allocator、Arena、集合与运行时内存布局
  * ADR-005：Host ABI、Capability 与资源 Handle
* **实现优先级**：P0
* **兼容性范围**：不修改 Nexa Language 1.0 已冻结语义

---

## 1. 背景

Nexa 已确定以 `Result<T, E>` 表达可恢复失败，以确定性 Drop 和 `defer` 完成清理，
并将 panic 默认映射为 abort。仍需冻结这些机制在正常返回、提前返回、部分初始化、
Host 调用、异步取消和稳定 ABI 上的精确交互。

如果后端把 `try/catch` 实现成平台异常，把 `?` 当作特殊 return，或者分别插入 Drop 与 defer，
同一源程序可能在解释器、Native 和 Wasm 上产生不同的清理顺序。错误路径还可能重复关闭
已经 Move 的资源，或让 panic/unwind 穿越 C、Host 和组件边界。

本 ADR 将失败划分为显式值、结构化控制退出和不可恢复终止，并为所有可清理退出建立
一套统一的 cleanup plan。

---

## 2. 决策摘要

1. 可恢复失败只能通过 `Result<T, E>` 或显式协议状态表达。
2. `Result<T, E>` 是普通 tagged union；其私有布局不属于稳定 ABI。
3. `?` 只做一次求值，并把 `Err` Move 到当前 propagation target。
4. `try/catch` 是 `Result` 的结构化控制流语法，不捕获 panic，不执行 runtime unwind。
5. 每个作用域生成一个 cleanup stack；局部 Drop 与 `defer` 按注册逆序执行。
6. 正常结束、`return`、`break`、`continue`、`?` 和异步取消都执行所离开作用域的 cleanup。
7. panic 默认立即 abort，不保证执行 Drop 或 `defer`。
8. Drop 和 defer cleanup 必须返回 `Unit`，不能把新的可恢复错误隐式替换原结果。
9. 可能失败的资源关闭必须由显式消费 API 返回 `Result`；Drop 仅做不可失败的兜底释放。
10. 部分初始化只清理已经提交为 initialized 的字段、元素和临时值。
11. `take` 调用开始后所有权不因 `Err` 自动退回；需要返还时必须放进错误值。
12. 稳定 ABI 使用 status、out value 和 error descriptor，不暴露内部 `Result` 布局。
13. 外部异常、trap 或 panic 必须在边界内转换为错误或终止隔离实例。
14. Debug、Release、reference、Native 和 Wasm 必须具有相同 cleanup trace。

---

## 3. 失败分类

Nexa 区分三类结果：

| 类别 | 表示 | 是否执行 cleanup | 是否可被普通 `catch` 处理 |
|------|------|------------------|-----------------------------|
| 可恢复失败 | `Result.Err(E)` | 是 | 是 |
| 结构化退出 | return/break/continue/cancel | 是 | 否或由所属结构处理 |
| 不可恢复失败 | panic/abort/invariant failure | 默认否 | 否 |

OOM 服从 ADR-004：普通 Application allocation OOM 属于 abort；fallible allocator 返回
`AllocationError.OutOfMemory`。Host operation failure 服从 ADR-005，先转换为结构化错误再进入 `Result`。

不得用 null、未初始化 out parameter、线程本地 errno、日志文本或隐式异常表达安全 API 的失败。

---

## 4. `Result<T, E>`

`Result<T, E>` 的语义形态为：

```text
Result<T, E> = Ok(T) | Err(E)
```

它遵守普通 tagged union 规则：

* 同一时刻只有 active variant 的 payload 处于 initialized。
* Move `Result` 会 Move active payload。
* Drop 只处理 active payload，且恰好一次。
* `T` 和 `E` 可以是 Copy 或 Move 类型。
* `Result` 不执行隐式错误类型转换。
* 错误转换必须通过显式映射或编译器可验证的 `from` 合同。

编译器可以对私有 `Result` 使用 niche optimization，但 tag、payload offset 和 niche 不是语言合同。
稳定 ABI 必须使用显式 result descriptor。

函数签名中的错误类型是公开 API 的一部分。新增 variant 可以是 source-compatible，
但只有调用方已经要求 exhaustive unknown handling 时才是 stable component ABI compatible。

---

## 5. `?` 传播

表达式：

```text
const value = operation()?;
```

概念 lowering：

```text
temporary = evaluate operation exactly once
switch temporary {
  Ok(value): move value into destination
  Err(error):
    move/convert error into propagation result
    run cleanup for every exited scope
    return Err(error)
}
```

规则：

* propagation target 是最近的函数返回或 `try` recovery region，在类型检查时确定。
* error conversion 先成功构造目标 error，再进入 cleanup；转换自身失败必须显式嵌套 Result。
* 返回 payload 先 Move 到独立 return slot，再执行当前作用域 cleanup。
* 已 Move 到 return slot 的值不再由原 place Drop。
* cleanup 不能读取已经 Move 的 payload。
* `?` 不捕获 panic，也不创建异常表。

NIR 必须把分支、return slot、Drop 和 defer 调用全部表示为普通控制流。

---

## 6. `try/catch`

`try/catch` 只处理其 recovery region 中传播的 `Result.Err`：

```text
try {
  const value = load()?;
  use(value);
} catch {
  case LoadError.NotFound() => recover();
  case LoadError.Invalid(message) => report(message);
}
```

语义：

* `try` region 有一个显式 error continuation。
* `?` 向该 continuation 分支前，先清理离开的内层作用域。
* catch pattern 对 error payload 执行普通 match ownership 规则。
* catch 必须穷尽 error type，或显式保留/重新传播 unmatched error。
* catch arm 可以返回普通值、`Result` 或再次使用 `?`。
* panic、abort、Wasm trap 和外部 exception 不进入 catch arm。

`try/catch` 不是 JavaScript、C++ 或 LLVM exception semantics，不要求栈展开 runtime。

---

## 7. Cleanup stack

每个词法作用域维护按程序执行动态形成的 cleanup stack。以下事件会注册 cleanup action：

* 一个带 Drop glue 的 local/temporary 完成初始化
* 一个 `defer` 语句执行到注册点
* 一个部分聚合字段完成初始化
* runtime guard 或 Host handle 成功取得所有权

离开作用域时，已注册 action 按严格逆序执行。嵌套作用域先清理最内层。

例如：

```text
first = createFirst()      // register Drop(first)
defer releaseLease()       // register defer
second = createSecond()    // register Drop(second)
return value
```

退出轨迹：

```text
move value to return slot
Drop(second)
releaseLease()
Drop(first)
return
```

这一个规则同时确定局部变量、defer 和临时值的相对顺序。后端不得分别排序后再拼接。

---

## 8. `defer`

`defer expression;` 在控制流到达该语句时注册，而不是进入作用域时注册。

规则：

* 未执行到的分支不会注册 defer。
* 同一作用域按注册逆序执行。
* defer expression 必须产生 `Unit`，且不能包含跨 cleanup 边界的 `await`。
* fallible cleanup 必须在 defer body 内显式处理结果，不能使用 `?` 改写正在退出的结果。
* defer 对所用变量形成延伸到执行点的 use/borrow。
* 被 defer 捕获的 Owned value 在 action 执行前不能被其他路径 Move。
* defer 自身创建的局部值在 action 完成前按普通规则清理。
* defer 中 panic 仍遵循 panic strategy；默认 abort，并停止剩余 cleanup。

需要观察关闭错误时，应在主控制流显式调用消费方法：

```text
const file = File.open(path)?;
const closeResult = file.close();
return closeResult;
```

Drop/defer 只适合 `closeIgnoringError`、release、unlock 等不可失败或已明确吸收错误的兜底操作。

---

## 9. Drop 与失败

用户 Drop 方法必须是同步 `Unit`：

```text
function drop(mut self): Unit;
```

它不能：

* 返回 `Result`
* 使用 `?` 传播到正在退出的函数
* 是 `async`
* Move 受保护的 self/field
* 让 panic 穿越稳定 ABI

如果底层 release 失败，Drop 实现只能：

* 使用不会失败的 runtime release contract；或
* 把失败报告给诊断/telemetry sink 后继续；或
* 在破坏安全不变量时 panic/abort。

它不能把失败悄悄替换原函数的 `Ok`/`Err`。需要业务处理的关闭失败必须使用显式 `close(take self)`。

---

## 10. 部分初始化与所有权转移

初始化状态在 MIR/NIR 中按 place 或字段追踪。

* action 只在值完整写入并提交 initialized 后注册。
* 构造失败只清理已提交字段，顺序为提交逆序。
* union payload 只有在 tag 提交后才成为 active variant；提交前由构造临时值负责清理。
* 容器 length 只有在元素写入完成后递增。
* Move 清除 source 的 cleanup action 或对应 Drop flag。
* `take` 调用进入 callee 后，即使返回 `Err`，source 也不重新 initialized。

若失败需要返还 ownership，错误类型必须明确携带值：

```text
Result<Unit, PushError<T>>
```

从 `PushError<T>` 中取出 `T` 会建立新的 owner，不是回滚原 Move。

---

## 11. Panic、abort 与隔离

默认 Application 和 Freestanding panic strategy 为：

```text
panic -> report minimum diagnostic -> abort process/instance
```

因此默认不承诺：

* 栈展开
* 局部 Drop
* defer
* user Drop
* catch recovery

这使 panic 只适合边界检查失败、内部不变量破坏和显式不可恢复终止。

测试 runner、plugin Host 或 server 可以在进程、worker 或 Wasm instance 级隔离 panic。
隔离层只得到 `Panicked` outcome 和脱敏报告，不能恢复发生 panic 的 Nexa stack。

未来若增加 unwind profile，必须由新 ADR 定义，并在所有 C/Host/Wasm 边界 catch；
它不能静默改变本 ADR 的默认语义或 artifact `panic_strategy` metadata。

---

## 12. Async 与取消

`async` 函数中的 `Result` 与同步函数相同。`?` 把 error 写入 task result slot，
然后执行当前 async frame 的 cleanup plan。

结构化取消不是 `Result.Err(E)`，也不由普通 catch 捕获。取消在安全暂停点进入 cancel continuation：

```text
mark cancelled
-> stop executing user body
-> run registered defer and Drop in reverse order
-> finish task as Cancelled
```

cancel cleanup 不允许 `await`，不得无限阻塞。完整 task tree、join 与取消竞争由 ADR-007 冻结。

---

## 13. 跨 ABI 失败

稳定边界禁止直接传递内部 `Result<T, E>`，使用：

```text
status
+ initialized success out parameter when status == OK
+ initialized error descriptor when status != OK
```

边界 wrapper 必须：

1. 验证输入 descriptor。
2. 初始化 ownership tracking，但不提前发布输出。
3. 调用内部函数并检查显式 Result。
4. 把 active payload 转换到 stable descriptor。
5. 只发布与 status 一致的 out parameter。
6. 清理未发布的 partial output。

规则：

* error code/category 是合同；message 仅供诊断。
* 未知可扩展 error code 映射为 `Unknown`。
* C errno、JS exception、Host rejection 和 OS status 在 wrapper 内转换。
* panic/unwind/trap 不得伪装成普通 domain error，除非隔离边界明确产生 `Panicked` outcome。
* ABI caller 只释放已标记为 initialized/owned 的输出。

---

## 14. 诊断与安全

本 ADR 使用 `E5800-E5899` 作为错误 lowering 与 cleanup 合同诊断范围。

| 错误码 | 含义 |
|--------|------|
| `E5801` | `?` 没有兼容 propagation target |
| `E5802` | error conversion 不明确或不兼容 |
| `E5810` | catch 未穷尽且未重新传播 |
| `E5820` | defer 返回可失败值或试图 `?` 传播 |
| `E5821` | defer 使用已 Move 值 |
| `E5830` | Drop 签名可失败、async 或非法 Move |
| `E5840` | cleanup plan 重复或遗漏 initialized place |
| `E5850` | panic/unwind 可能穿越稳定 ABI |
| `E5860` | ABI status 与 out parameter 状态矛盾 |

诊断必须指出 propagation target、被退出的 scope、cleanup action 注册点与 Move 来源。
Panic report 和外部错误必须脱敏，不得泄漏 capability token、原始内存或未授权 payload。

---

## 15. 被拒绝的方案

### 15.1 以异常作为默认可恢复错误

异常隐藏在函数签名之外，需要栈展开，并使 FFI、Wasm 和 Drop 交互复杂化。

### 15.2 让 `try/catch` 同时捕获 Result 与 panic

这会混淆可恢复 domain failure 与内存/不变量失败，并让调用方误以为 panic 后状态可继续使用。

### 15.3 Drop 返回错误并覆盖原结果

多个 cleanup failure 缺少稳定优先级，且会丢失原始结果。业务关闭错误必须显式处理。

### 15.4 Panic 默认 unwind

它增加 runtime、ABI 与优化复杂度，并与已接受的 panic-abort、小产物目标冲突。

### 15.5 defer 总是在函数入口注册

这会执行从未到达的分支 cleanup，并访问未初始化值。

### 15.6 错误时自动回滚 Move

callee 或 Host 可能已部分消费资源，隐式回滚会制造重复 owner。

---

## 16. 后果

正面结果：

* 所有可恢复路径都能在 CFG/NIR 中验证，无需平台异常模型。
* Drop 与 defer 的相对顺序唯一且可测试。
* `Result` API 对 AI、IDE、FFI 和调用方明确可见。
* panic 边界简单，不会意外穿越 Host/C/Wasm。
* 同一 cleanup plan 可复用于同步早退与异步取消。

成本与限制：

* 函数签名必须携带错误类型，调用方需要显式传播或处理。
* 需要处理关闭失败的代码不能只依赖 RAII。
* MIR 必须建模 return slot、partial initialization 与动态 defer 注册。
* 默认 abort 会放弃 panic 路径上的进程内清理，可靠服务需要实例或进程隔离。

---

## 17. 分阶段实现

### Phase E0：Result CFG

* `Result` tagged union
* `?` propagation target 与 explicit branch
* `try/catch` recovery region
* error conversion checking

### Phase E1：Cleanup Plan

* local/temporary Drop registration
* defer dynamic registration
* unified reverse-order cleanup edges
* partial initialization 与 return slot

### Phase E2：Panic 与 Runtime

* panic-abort lowering
* minimal panic report
* Drop/defer panic behavior
* isolated runner outcome

### Phase E3：ABI Wrapper

* result descriptor
* status/out initialization verifier
* C/Host error conversion
* cross-backend failure trace

### Phase E4：Async Integration

* task result slot
* cancel continuation
* async frame cleanup
* ADR-007 structured task conformance

---

## 18. 测试要求

接受测试至少覆盖：

* `Ok`、`Err`、nested Result 和 Move-only payload
* `?` 的一次求值、error conversion 与 return slot
* nested try/catch、重新传播和 exhaustive match
* 正常结束、return、break、continue、`?` 的 cleanup trace
* local Drop 与 defer 交错注册的严格逆序
* 部分 Record/union/Vec 初始化失败
* 显式 close failure、take 后 Err 和错误中返还 ownership
* Host/C wrapper 的 success、error 与 unknown code

拒绝或 compile-fail 测试至少覆盖：

* 无 propagation target 或错误类型不兼容的 `?`
* 非穷尽 catch
* defer 中传播 Result、await 或使用已 Move 值
* async/fallible Drop 与 Drop 中 Move protected field
* cleanup 重复 Drop、读取未初始化 place
* status/out parameter 矛盾
* panic/unwind 穿越 C、Host 或 Wasm boundary

panic-abort 测试必须在子进程或隔离实例中运行，不能终止测试 harness。

---

## 19. 验收标准

ADR-006 进入实现完成状态前必须满足：

1. `?` 与 `try/catch` 完全降低为可验证 CFG，不依赖异常栈展开。
2. 所有清理退出共享同一 cleanup plan，并有规范化 trace。
3. partial initialization、Move 和 return slot 不产生遗漏或重复 Drop。
4. Drop/defer 不会隐式替换正在返回的结果。
5. panic strategy 进入 artifact metadata，且稳定 ABI 不允许 unwind 穿越。
6. reference、Native 与 Wasm 对 accepted/error/cancel 路径产生相同可观察结果。
7. accepted 与 rejected 测试覆盖 Host failure、panic、defer 和资源关闭。

---

## 20. 最终决定

Nexa 2.0 的错误模型为：

```text
Recoverable failure: Result<T, E>
Structured exit: explicit CFG + cleanup plan
Resource cleanup: deterministic Drop + Unit defer
Unrecoverable failure: panic -> abort
Stable boundary: status + descriptors, never internal Result/unwind
```

错误必须在类型或协议中可见；清理必须在控制流中可验证；panic 不能伪装成普通异常。
