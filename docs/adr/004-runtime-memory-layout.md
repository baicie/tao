# ADR-004：Allocator、Arena、集合与运行时内存布局

* **状态**：Accepted
* **日期**：2026-08-01
* **目标版本**：Nexa Language 2.0
* **关联 ADR**：

  * ADR-001：Nexa 2.0 语言架构、所有权与运行时模型
  * ADR-002：所有权、借用、Move 与确定性 Drop 语义
  * ADR-003：NIR、LLVM 后端边界与稳定 ABI
* **实现优先级**：P0
* **兼容性范围**：不修改 Nexa Language 1.0 已冻结语义

当前已交付的实现切片包括 [`Shared<T>`/`Weak<T>` 控制块](../implementation/adr-004-shared-weak.md)
和 [`StorageBox<T>` 唯一所有权](../implementation/adr-004-owned-box.md)。这些切片不等同于
ADR-004 全部实现；部分初始化、跨后端布局和稳定外部描述符仍按独立验收门槛推进。

---

## 1. 背景

ADR-002 已经定义谁拥有值、何时 Move、何时 Borrow，以及何时执行 Drop。ADR-003
要求 NIR 在进入具体后端前携带已经确定的布局与释放语义，但没有冻结这些语义如何映射到运行时存储。

如果不补充这一层合同，不同后端可能对以下问题作出不兼容决定：

* 长度、容量和对齐采用什么单位
* 空集合和 zero-sized type 是否真的分配内存
* 重新分配后由哪个 allocator 释放
* 部分初始化的值应 Drop 哪些字段
* Arena 中有析构行为的值是否会被跳过
* `String`、`Vec<T>`、`Array<T>`、`Box<T>` 和 `Shared<T>` 的不变量
* OOM 在普通应用和 Freestanding 环境中的行为
* 哪些布局可以出现在稳定 ABI 中

本 ADR 冻结运行时必须遵守的语义不变量，而不冻结某个 LLVM 版本、指针宽度或私有结构体字段顺序。

---

## 2. 决策摘要

1. 每个具体 target 以其经过验证的 data layout 作为大小和对齐来源。
2. 编译器为每个具体化类型生成 `LayoutId`、大小、对齐、值类别和 Drop glue。
3. 私有 Native Layout 只保证同一工具链、target 和编译单元契约内一致。
4. 稳定 ABI 只能使用 ADR-003 定义的 descriptor、handle 或显式 `@repr(C)` 类型。
5. 每次堆分配都具有不可伪造的 allocator provenance；释放必须回到兼容 allocator。
6. `String` 是拥有 UTF-8 字节的不可变值；长度和容量以字节计。
7. `Vec<T>` 是唯一所有权的可变连续序列；长度和容量以元素计。
8. `Array<T>` 是不可变序列，运行时可以共享或采用 COW，但共享不可观察。
9. `Box<T>` 具有唯一所有权；`Shared<T>`/`Weak<T>` 使用显式控制块并遵守一次 Drop。
10. zero-sized type 不要求真实分配，但仍参与元素个数、Drop 次数和所有权分析。
11. Arena 的批量回收不能跳过语言要求的 Drop；带资源值必须登记清理或被拒绝放入无 Drop Arena。
12. Application Profile 的普通分配 OOM 默认 abort；fallible 与 Freestanding API 返回显式错误。
13. Move 不复制分配，Drop 只处理仍处于已初始化状态的 place。
14. 所有大小、乘法、对齐和容量增长必须进行溢出验证。

---

## 3. 布局合同

NIR 中每个可物化类型必须引用规范化布局记录：

```text
Layout {
  id: LayoutId,
  size: U64,
  align: U32,
  value_kind: Immediate | InlineAggregate | OwnedHeap | SharedHandle,
  drop_glue: Option<DropGlueId>,
  fields: [FieldLayout],
  target: TargetLayoutId
}
```

不变量：

* `align` 是 target 支持的非零二次幂。
* `size` 包含内部 padding，但不包含外部分配器元数据。
* 字段 offset 必须满足字段对齐且落在聚合大小内。
* `size + align`、`count * stride` 等运算在布局阶段检查溢出。
* 不完整、递归无间接层或 target 不可表示的布局在代码生成前拒绝。
* Debug 与 Release 可以选择不同私有表示，但不能改变语言可观察语义。

普通 Record、Tuple 和 Class 的字段顺序不是稳定 ABI。编译器可以重排私有物理字段，
但字段初始化、求值和 Drop 顺序仍由 ADR-002 的语言顺序决定。

Tagged Union 的 tag 宽度、niche optimization 和 payload 重叠属于私有实现。
跨稳定边界时必须使用显式 tagged descriptor 或已验证的 `@repr(C)` 表示。

---

## 4. Zero-sized type 与空值

大小为零的值仍然是完整语言值。

* `Vec<Zst>` 的 `length` 正常增长，迭代产生正确次数。
* 如果 ZST 具有 Drop glue，每个已初始化逻辑元素仍 Drop 一次。
* 对 ZST 的分配不得要求 allocator 返回唯一物理地址。
* 空 `String`、空 `Vec<T>` 和空 `Array<T>` 可以使用对齐哨兵，不要求堆分配。
* 安全代码不得通过地址比较观察 ZST 或空值是否共享哨兵。
* `Slice<T>` 即使长度为零，其 pointer 仍必须满足 non-null/aligned descriptor 约束，
  除非对应外部 ABI 明确允许 null-empty 规范形式。

编译器不得用“没有分配”推导“没有 Drop”。

---

## 5. Allocator 合同

运行时 allocator 的语义接口为：

```text
allocate(layout) -> Result<Allocation, AllocationError>
grow(allocation, old_layout, new_layout) -> Result<Allocation, AllocationError>
shrink(allocation, old_layout, new_layout) -> Result<Allocation, AllocationError>
deallocate(allocation, layout) -> Unit
```

`Allocation` 包含运行时可验证的 provenance。具体实现可以把 provenance 放在对象头、
side table、allocator handle 或编译器已知路径中，但不得依靠调用方猜测。

必须满足：

* `deallocate` 使用与原分配兼容的 allocator、大小和对齐。
* `grow`/`shrink` 成功后旧 allocation 失效；失败时旧 allocation 保持有效。
* 请求大小为零时可以不调用底层 allocator。
* 成功的非零分配满足请求对齐且覆盖完整大小。
* allocator 返回的内存初始为未初始化字节，不自动构造 `T`。
* 自定义 allocator 只能在 `unsafe` 实现，其安全包装必须维护这些不变量。

Application Profile 提供进程默认 allocator。显式 allocator 创建的值必须保留 provenance，
并在 Move、容器增长和函数返回后继续使用原 allocator。

Freestanding Profile 不存在隐式默认堆；程序必须提供 allocator、固定缓冲区或明确拒绝堆分配。

---

## 6. `String`、`Vec<T>` 与 `Array<T>`

### 6.1 `String`

`String` 的概念私有表示包含 owned allocation、byte length、byte capacity 和 allocator provenance。
确切字段、small-string optimization 与 pointer tagging 不属于语言或稳定 ABI。

不变量：

* 已初始化区间始终是合法 UTF-8。
* `0 <= byte_length <= byte_capacity`。
* capacity 表示可容纳字节数，不包含隐式 NUL。
* `String` 不保证 NUL 结尾；嵌入 NUL 是合法内容。
* 只读访问不会使已有 borrow 失效。
* 任何可能重分配的构建操作要求唯一可变访问。
* Move 转移 allocation 与 provenance；Clone 产生独立逻辑值。

运行时可以实现 SSO、共享只读常量或 COW。发生共享时，修改路径必须先确保唯一可写，
且优化不能改变 Drop、相等性或 allocator 边界。

### 6.2 `Vec<T>`

`Vec<T>` 的概念私有表示包含 owned allocation、element length、element capacity、元素布局与 provenance。

不变量：

* `0 <= length <= capacity`。
* `[0, length)` 是已初始化 `T`；其余 capacity 是未初始化存储。
* allocation 的对齐至少为 `align_of(T)`。
* capacity 增长在分配前验证 `capacity * size_of(T)`。
* `push` 只有在元素完全写入后才提交新 length。
* `pop` 先从已初始化区间移出元素，再减小 length。
* 重分配只能发生在唯一访问下，并使旧 pointer/slice 失效。
* Drop 只按语言规定顺序处理 `[0, length)`。

ZST 的 capacity 可以采用内部哨兵表示，但 API 不得暴露该哨兵。

### 6.3 `Array<T>`

`Array<T>` 是不可变序列。它可以内联、引用只读 allocation、采用持久化结构或共享缓冲区。

* `Array<T>` 的索引与迭代顺序稳定。
* 共享优化不能产生用户可观察的对象身份。
* `toVec()` 返回唯一可变所有权；必要时复制。
* `Vec.freeze()` 消费 Vec，并可零拷贝转移 allocation。
* Array slice 只 Borrow，不取得 backing storage 所有权。

---

## 7. `Box<T>`、`Shared<T>` 与 `Weak<T>`

`Box<T>` 拥有一个 `T` allocation：

* Move 只转移唯一 owner。
* Drop 先运行 `T` 的 Drop glue，再由原 allocator 释放 allocation。
* 从未完成初始化的 `Box<T>` 不得作为安全值发布。

`Shared<T>` 与 `Weak<T>` 使用概念控制块：

```text
SharedControl {
  strong_count,
  weak_count,
  value_state,
  allocator_provenance,
  T storage
}
```

规则：

* `Shared.clone()` 明确增加强引用；普通 Move 不增加计数。
* 最后一个强引用释放时，`T` 恰好 Drop 一次。
* 控制块在所有 weak reference 消失后释放。
* `Weak.upgrade()` 只能在 value 仍存活时获得新强引用。
* 计数溢出是不可恢复的 runtime invariant failure，必须 abort。
* 单线程和线程安全计数实现可以不同，但 `Send`/`Sync` 能力必须真实反映实现。
* 强引用环可能泄漏，但不得造成 use-after-free。

控制块布局和计数宽度不进入稳定 ABI；跨边界共享使用 typed handle。

---

## 8. Arena

Arena 拥有一组 allocation，并在 Arena scope 结束时统一回收存储。

安全 Arena API 必须选择以下一种模式并在类型中可见：

1. `DropArena`：为带 Drop glue 的已初始化值登记清理，并按反向登记顺序执行。
2. `PlainArena`：只接受无需 Drop 的类型；其他类型在编译期拒绝。

共同规则：

* Arena borrow、slice 或引用不得逃逸 Arena scope。
* Arena-owned value 不得直接放入更长生命周期的 owner、task 或 `Shared<T>`。
* Handle 必须携带 Arena identity/generation，并在 Arena 销毁后拒绝访问。
* Reset 等价于结束当前 generation；先完成所需 Drop，再回收存储。
* Arena 批量释放不能替代 File、Socket、Host Handle 等资源的 Drop。
* 取消、panic 隔离或提前返回必须走同一清理计划。

Arena 的块大小、增长算法和 chunk 链表是私有实现。

---

## 9. Drop glue 与部分初始化

Drop lowering 为每个需要清理的具体化类型生成 `DropGlueId`。

Drop glue 的输入必须足以确定：

* 具体类型与私有布局版本
* 仍然已初始化的字段或元素
* union 当前 active variant
* allocator 或 Arena provenance
* owned、shared 或 handle 释放策略

构造聚合值时，编译器维护初始化状态。若字段初始化中途通过 `Result` 早退，
只 Drop 已成功初始化的字段，并遵循 ADR-002 的反向初始化顺序。

Move 后 source place 变为未初始化；其 Drop flag 被清除。优化可以消除物理 flag，
但必须证明所有控制流上的 Drop trace 等价。

Drop glue 不能：

* 猜测 allocator
* 读取未初始化 padding
* 重复释放已 Move 字段
* 让用户可恢复错误从 Drop 中传播
* 穿越稳定 ABI 触发 unwind

---

## 10. OOM、边界检查与安全

Application Profile 的普通 `String`、`Vec<T>`、`Box<T>` 和 `Shared<T>` 构造在 OOM 时 abort。
显式 `try_*` API 与 fallible allocator 返回 `AllocationError.OutOfMemory`。

Freestanding 构建必须在 artifact metadata 中声明 OOM 策略，且不同优化级别不得改变它。

运行时必须拒绝或终止以下情况：

* 大小或容量计算溢出
* allocator provenance 不匹配
* 无效对齐
* forged/stale Arena handle
* Shared 计数溢出或下溢
* 越界 slice、length 大于 capacity 或错误元素 stride

来自 Host、FFI、缓存或 artifact 的布局数据一律不可信，必须在创建安全值前验证。
Release 优化不能移除边界验证，除非 NIR 证明条件恒真。

---

## 11. 公开边界与兼容性

以下内容是 Nexa 2.0 的稳定语义：

* UTF-8、长度单位、元素顺序和所有权行为
* allocator provenance 与“谁分配谁释放”
* OOM profile
* 初始化与 Drop 次数
* Arena 不逃逸规则

以下内容保持私有且可变化：

* 指针、长度、容量的字段顺序和宽度
* SSO、COW、niche、pointer tagging
* Vec 增长因子
* Box/Shared 对象头和计数实现
* Arena chunk 策略
* LLVM struct type 与 target-specific calling convention

稳定 C/Host/Wasm 边界必须通过 descriptor、typed handle、版本和 ABI hash 传递。
任何试图导出普通内部 `String`、`Vec<T>`、`Array<T>`、`Box<T>` 或 `Shared<T>` 的接口均被拒绝。

---

## 12. 诊断

本 ADR 使用 `E5700-E5799` 作为内存布局与 allocator 合同诊断范围。

| 错误码 | 含义 |
|--------|------|
| `E5701` | 类型布局不可表示或大小溢出 |
| `E5702` | 非法或 target 不支持的对齐 |
| `E5710` | 容量或 allocation 大小溢出 |
| `E5720` | allocator provenance 不匹配 |
| `E5730` | Arena borrow/value 逃逸 |
| `E5731` | 带 Drop 类型进入 PlainArena |
| `E5740` | 未初始化或已 Move place 被 Drop |
| `E5750` | 内部 runtime layout 泄漏到稳定 ABI |

诊断必须指出类型、目标平台、所需大小/对齐、来源 allocator 或逃逸路径；
不得只暴露 LLVM assertion、裸地址或 allocator 内部指针。

---

## 13. 被拒绝的方案

### 13.1 冻结所有 Native 对象布局

这会阻断 SSO、COW、target-specific 优化和 runtime 演进，并把 LLVM 细节变成公共承诺。

### 13.2 所有分配统一使用进程全局 allocator

这无法支持 Freestanding、Arena、插件隔离和明确资源预算，也掩盖跨模块错误释放。

### 13.3 Arena 结束时只释放字节

这会跳过资源 Drop，使 Arena 成为文件、handle 和引用计数泄漏入口。

### 13.4 默认引用计数所有值

这与单一所有权和小运行时目标冲突，并在每次赋值中引入隐藏原子或计数成本。

### 13.5 依赖 null 表示所有空值

null pointer 不能统一满足 slice 对齐、ZST 与外部 ABI 规则，容易把“空”与“无效”混淆。

---

## 14. 后果

正面结果：

* Native、Wasm 与参考执行器可共享一套内存不变量。
* runtime 可以持续优化私有布局而不破坏稳定 ABI。
* 自定义 allocator、Arena 和 Freestanding 具有明确安全边界。
* Drop conformance 可以按轨迹测试，而不依赖物理表示。

成本与限制：

* NIR Verifier 必须理解 layout、初始化状态和 provenance。
* runtime 需要可裁剪的 memory、collections 与 shared 模块。
* FFI 必须生成 wrapper，不能直接传递高级内部值。
* `DropArena` 需要维护清理登记，不能保证纯 bump allocation 的最低成本。

---

## 15. 分阶段实现

### Phase M0：Target Layout

* `TargetLayoutId`、`LayoutId` 与规范化 layout table
* ZST、聚合、union 和递归布局验证
* NIR layout verifier 与 snapshot

### Phase M1：Allocator 与 Owned Storage

* 默认和 fallible allocator
* provenance
* `Box<T>` 与部分初始化清理
* OOM profile

### Phase M2：String 与集合

* UTF-8 `String`
* `Vec<T>` 增长、freeze 与 Drop
* immutable `Array<T>` 与 Slice
* ZST conformance

### Phase M3：Shared 与 Arena

* `Shared<T>`/`Weak<T>` 控制块
* `DropArena` 与 `PlainArena`
* generation handle 和 reset 清理

### Phase M4：跨后端与稳定边界

* Native/Wasm 布局差异测试
* C/Host descriptor wrapper
* allocator/Drop trace conformance
* Debug allocator 与泄漏检测

---

## 16. 测试要求

接受测试至少覆盖：

* 空和非空 `String` 的 UTF-8、Clone、Move 与 Drop
* `Vec<T>` push/pop/grow/freeze，以及分配失败后原值仍有效
* ZST Vec 的长度、迭代次数和逐元素 Drop
* `Box<T>`、嵌套 Record、Tagged Union 的一次 Drop
* `Shared.clone()`、Weak upgrade 和最后强引用释放
* DropArena 的反向清理和 PlainArena 的无 Drop 类型
* 部分初始化、`?` 早退和 panic 隔离的 Drop trace
* 相同程序在 reference、Native 和 Wasm 上的可观察结果一致

拒绝或 compile-fail 测试至少覆盖：

* `count * stride` 溢出
* Arena borrow 或 value 逃逸
* 带 Drop 类型进入 PlainArena
* 已 Move 值再次 Drop
* 使用错误 allocator 释放
* forged layout/handle、无效对齐和 length 大于 capacity
* 将内部 `Vec<T>`/`String` 布局直接导出稳定 ABI

测试不得断言私有字段顺序、Vec 增长因子或 SSO 阈值。

---

## 17. 验收标准

ADR-004 进入实现完成状态前必须满足：

1. 所有可代码生成类型都有经过 verifier 的 target layout。
2. `String`、`Vec<T>`、`Array<T>`、`Box<T>`、`Shared<T>` 和 Arena 不变量均有测试。
3. 所有 allocation 的释放路径可证明使用正确 provenance。
4. partial initialization 与 Move 后 Drop trace 在所有后端一致。
5. Application 与 Freestanding OOM 行为可配置、可检查且不随优化级别改变。
6. 稳定 ABI conformance 证明内部 runtime layout 没有泄漏。
7. accepted 与 rejected 测试同时存在，并包含 ZST、溢出和资源清理边界。

---

## 18. 最终决定

Nexa 2.0 将内存安全语义建立在：

```text
Target-verified Layout
+ Allocator Provenance
+ Explicit Initialization State
+ Deterministic Drop Glue
+ Non-escaping Arena Access
+ Stable External Descriptors
```

内部布局可以演进；UTF-8、所有权、释放来源、失败行为和 Drop 次数不能漂移。
