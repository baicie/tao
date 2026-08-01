# ADR-003：NIR、LLVM 后端边界与稳定 ABI

* **状态**：Accepted
* **日期**：2026-08-01
* **目标版本**：Nexa Language 2.0
* **关联 ADR**：

  * ADR-000：Nexa 编译器自举路线
  * ADR-001：Nexa 2.0 语言架构、所有权与运行时模型
  * ADR-002：所有权、借用、Move 与确定性 Drop 语义
* **实现优先级**：P0
* **兼容性范围**：不修改 Nexa Language 1.0 已冻结语义

---

## 1. 背景

Nexa 计划采用 LLVM 作为第一生产后端，生成：

* Linux 原生程序
* macOS 原生程序
* Windows 原生程序
* 后续的 Freestanding 和嵌入式目标
* 初期 WebAssembly 目标

现有编译器已经具备：

```text
Source
→ Lossless CST
→ HIR
→ Typed HIR
→ CFG MIR
→ Reference Interpreter
```

但 MIR 当前主要服务于：

* 语言语义表达
* 参考解释执行
* 控制流验证
* Conformance
* 测试

LLVM 后端还需要更接近机器执行模型的表示，包括：

* 具体值表示
* 调用约定
* Drop 插入结果
* 聚合类型布局
* 泛型实例
* 异常与 Panic 路径
* ABI 分类
* Host 调用
* 目标平台信息

如果 LLVM 后端直接读取高层 MIR 并自行重新解释所有权、Drop、泛型和数据布局，将产生以下问题：

1. LLVM 后端成为第二个语义实现。
2. Reference Interpreter 与 Native Backend 容易出现行为差异。
3. 后续 Wasm、C 或其他后端必须重复复杂逻辑。
4. 自举编译器需要直接理解 LLVM。
5. LLVM 类型和数据布局可能泄漏到语言规范。
6. 后端升级可能意外形成语言 Breaking Change。
7. 很难定义稳定的 Host ABI 和模块接口。

因此需要在 MIR 与具体后端之间建立一个后端中立但已经完成语言语义决策的表示：

> **NIR：Nexa Native Intermediate Representation。**

NIR 不负责名称解析、类型推断和所有权推断，而负责将已经确定的 Nexa 程序表示成可验证、可布局、可交给 LLVM/Wasm 等后端的低层形式。

---

## 2. 决策摘要

本 ADR 作出以下决定：

1. 在 Typed MIR 与具体代码生成后端之间引入 NIR。
2. MIR 表达语言语义；NIR 表达已确定的执行与 ABI 语义。
3. 所有权、Borrow、Drop、泛型解析和调用目标必须在进入 NIR 前确定。
4. LLVM 后端不得重新执行名称解析、类型检查、所有权推断或 Drop 推断。
5. NIR 初期是编译器内部格式，不作为跨版本公共字节码。
6. NIR 必须有独立 Verifier。
7. LLVM 封装在独立后端 crate/module 中。
8. 编译器核心不得直接依赖 LLVM 类型。
9. 第一生产代码生成目标为 LLVM Native AOT。
10. WebAssembly 初期可以复用 LLVM Wasm Target，后续允许独立 Wasm Backend。
11. Nexa 不在 2.0 初期承诺稳定的 Native-to-Native Nexa ABI。
12. Nexa 稳定 ABI 分为：

    * 稳定 C ABI 互操作层
    * 稳定 Nexa Host ABI
    * 稳定组件/插件 ABI 子集
13. 编译器内部 NIR 序列化、LLVM IR 和对象布局默认不稳定。
14. 所有公开 ABI 类型必须显式声明表示方式。
15. ABI 不允许依赖 Rust ABI、LLVM 版本或未声明的结构体布局。
16. 泛型默认不跨稳定 ABI 边界暴露。
17. `String`、`Array`、`Result` 等高级类型通过稳定 ABI 描述符传递，而不是暴露内部 Runtime 布局。
18. ABI 版本必须进入产物元数据，并在加载前验证。
19. 安全 Host ABI 必须表达所有权、生命周期、错误和 Capability。
20. Debug 与 Release 必须保持相同语言和 ABI 语义。

---

## 3. 编译管线

Nexa 2.0 的目标编译管线为：

```text
Nexa Source
  ↓
Lossless CST
  ↓
Resolved HIR
  ↓
Typed HIR
  ↓
Semantic CFG MIR
  ↓
Ownership / Drop Lowering
  ↓
Monomorphization or Generic Strategy Selection
  ↓
NIR
  ↓
NIR Verification
  ↓
LLVM / Wasm / Other Backend
  ↓
Object / Wasm Module
  ↓
Linker
  ↓
Executable / Library / Component
```

每一层拥有明确职责。

---

## 4. MIR 与 NIR 的职责边界

### 4.1 MIR 的职责

MIR 负责表达 Nexa 语言语义，包括：

* 控制流
* 表达式求值顺序
* 局部变量
* 函数调用
* Match
* Loop
* Return
* Panic
* Copy
* Move
* Shared Borrow
* Mutable Borrow
* Drop 语义
* `?` 传播
* `try/catch` 的结构化控制流
* 闭包调用
* Async 降低前的语义结构

MIR 中仍允许存在较高层的语言类型：

```text
String
Array<T>
Vec<T>
Result<T, E>
ClassId
EnumId
ClosureId
ResourceId
```

---

### 4.2 NIR 的职责

NIR 接收已经完成语义决策的程序，并表达：

* 后端无关的具体控制流
* 具体函数实例
* ABI 函数签名
* 值分类
* 具体聚合布局描述
* 对象创建与释放
* Drop 调用
* Runtime Intrinsic
* Host 调用
* 闭包环境布局
* Async 状态机布局
* Panic/Abort 路径
* Target-independent Pointer/Handle 操作
* 后端可执行的整数、浮点、内存和控制流操作

NIR 不允许包含：

* 未解析名称
* 源码路径查找
* 重载候选
* 未推断泛型
* 未确定所有权模式
* 未确定 Borrow 生命周期
* 未插入 Drop 的 Owned Local
* 未解析字段名
* 未解析 Enum Variant 名
* 未确定调用目标
* OXC AST 或 CST 节点
* LLVM 类型和 LLVM 指令对象

---

### 4.3 后端的职责

LLVM Backend 只负责：

* 将 NIR 类型映射为 LLVM 类型
* 将 NIR 指令映射为 LLVM IR
* 根据 Target Data Layout 完成合法布局
* 生成目标文件
* 生成 Debug Info
* 调用优化管线
* 处理目标相关 Calling Convention
* 生成链接所需符号和 Section

LLVM Backend 不负责：

* 类型检查
* 泛型推断
* Borrow Check
* Move Check
* Drop 决策
* Match 穷尽性
* 模块可见性
* 函数重载
* 错误类型推断
* 闭包逃逸分析

---

## 5. NIR 的定位

NIR 是：

```text
后端中立
强类型
SSA 友好
可验证
显式控制流
显式内存操作
显式调用目标
显式 ABI
```

NIR 不是：

* 公共字节码
* 永久稳定序列化格式
* 用户可直接编写的语言
* TypeScript AST
* LLVM IR 的文本替代品
* Runtime 对象格式
* 包管理分发格式

---

## 6. NIR 模块模型

一个 NIR Module 至少包含：

```text
NirModule
├── module_id
├── target_agnostic_metadata
├── type_definitions
├── global_definitions
├── function_declarations
├── function_definitions
├── intrinsic_declarations
├── host_imports
├── exports
├── constants
└── debug_source_map
```

建议结构：

```rust
pub struct NirModule {
    pub module_id: ModuleId,
    pub types: Vec<NirTypeDef>,
    pub globals: Vec<NirGlobal>,
    pub functions: Vec<NirFunction>,
    pub imports: Vec<NirImport>,
    pub exports: Vec<NirExport>,
    pub constants: Vec<NirConstant>,
    pub metadata: NirMetadata,
}
```

该结构仅为概念模型，不冻结 Rust API。

---

## 7. NIR 类型系统

### 7.1 标量类型

NIR 至少支持：

```text
I1
I8
I16
I32
I64
U8
U16
U32
U64
F32
F64
Char32
Unit
Never
```

`Bool` 降低为逻辑 `I1`，但 ABI 边界可以规定使用固定宽度整数表示。

---

### 7.2 指针与引用分类

NIR 必须区分以下概念：

```text
RawPtr<T>
OwnedPtr<T>
BorrowPtr<T>
MutBorrowPtr<T>
Handle<Kind>
FunctionRef
```

这些类型不一定对应不同机器指针宽度，但 Verifier 必须保留其语义差异。

#### `RawPtr<T>`

* 仅由 Unsafe 或后端内部产生
* 允许地址运算
* 不自动 Drop
* 不保证非空
* 不具有安全生命周期

#### `OwnedPtr<T>`

* 表示唯一拥有的堆值
* 必须最终 Move 或 Drop
* 不能隐式复制
* 可以转化为临时 Borrow

#### `BorrowPtr<T>`

* 只读
* 不拥有对象
* 不允许逃逸定义范围
* 不能执行 Drop

#### `MutBorrowPtr<T>`

* 唯一可变访问
* 不拥有对象
* 不允许逃逸
* 与其他 Borrow 互斥

#### `Handle<Kind>`

用于：

* Host Resource
* DOM Node
* File
* Socket
* GPU Resource
* Runtime Managed Entry

Handle 不等于裸指针。

---

### 7.3 聚合类型

NIR 支持：

```text
Struct
Tuple
TaggedUnion
FixedArray
SliceDescriptor
StringDescriptor
ClosureEnvironment
AsyncFrame
```

聚合类型在 NIR 中使用字段索引，不使用源级字段名称执行访问。

字段名称仅保留在 Debug Metadata 中。

---

### 7.4 Nexa Record

普通结构 Record 在进入 NIR 后拥有确定字段顺序。

字段顺序由编译器根据规范决定，不允许 LLVM 自行排序。

对于内部非 ABI 类型，编译器可以重新排序字段优化布局，但必须满足：

* 不改变语言可观察行为
* Drop 顺序仍按语言声明顺序定义
* Debug Info 能映射回源字段
* 不用于稳定 ABI 时才能进行

稳定 ABI Record 禁止自动重新排序。

---

### 7.5 Class

Class 在 NIR 中降低为名义聚合类型。

第一阶段不支持继承，因此不需要：

* VTable 继承树
* Base Object Offset
* 虚基类
* Dynamic Cast

方法降低为普通函数，显式接收 `self`：

```text
User.rename(mut self, name)
```

降低为概念签名：

```text
fn User_rename(*mut User, StringBorrow) -> Unit
```

具体 LLVM 类型由后端生成。

---

### 7.6 Tagged Union

Tagged Union 降低为：

```text
tag
+
payload storage
```

概念表示：

```rust
struct TaggedUnion {
    tag: VariantTag,
    payload: PayloadStorage,
}
```

要求：

* Tag 宽度由编译器确定
* Variant ID 稳定于单次编译产物
* 稳定 ABI Enum 必须显式冻结 Tag 值
* Payload 对齐满足所有 Variant
* Drop 根据 Tag 选择当前 Payload
* 不允许后端重新分配 Variant 编号

内部 Enum 的 Tag 编号可以在编译器版本间变化。

---

### 7.7 `Option<T>`

`Option<T>` 的内部表示允许使用布局优化，例如：

* 空指针优化
* 无效 Handle 值
* 未使用 Enum Tag
* 显式 Tag + Payload

但优化必须满足：

* 语言语义不变
* Debug 与 Release 语义一致
* 稳定 ABI 边界不能依赖未声明的 Niche Optimization
* 对外 ABI 默认使用显式 Descriptor 或冻结表示

---

### 7.8 `Result<T, E>`

内部 `Result<T, E>` 可以使用优化布局。

跨稳定 ABI 时默认使用：

```text
status tag
+
success/error union payload
```

或使用生成的 ABI Descriptor。

不允许把内部优化后的 `Result` 布局直接暴露给 C 或 Host。

---

## 8. 值类别

NIR 将语言值分类为：

```text
Immediate
InlineAggregate
OwnedHeap
Borrowed
SharedHandle
HostHandle
Function
Closure
```

### 8.1 Immediate

包括：

* 整数
* 浮点
* Bool
* Char
* 小型无 Payload Enum
* Function ID

可以直接存在于寄存器。

---

### 8.2 InlineAggregate

适用于较小的：

* Tuple
* Record
* Enum
* 固定数组

具体是否内联由编译器与 Target ABI 决定。

是否内联不是语言可观察行为。

---

### 8.3 OwnedHeap

大值可以降低为唯一拥有的堆对象。

Move 时只移动所有权句柄，不深度复制。

---

### 8.4 SharedHandle

`Shared<T>` 通过 Runtime Intrinsic 操作：

```text
shared_new
shared_clone
shared_drop
shared_downgrade
weak_upgrade
```

NIR 不要求后端内联引用计数实现。

---

## 9. NIR 指令模型

NIR 应采用显式基本块与 Terminator。

概念形式：

```text
function
├── parameters
├── locals
├── blocks
└── return ABI
```

建议核心指令：

```text
const
copy
move
load
store
borrow_shared
borrow_mut
end_borrow
aggregate_init
field_get
field_set
tag_get
payload_get
call
call_indirect
call_intrinsic
call_host
drop
alloc
dealloc
memcpy
memmove
compare
integer_op
float_op
cast
select
```

Terminator：

```text
goto
branch
switch
return
abort
unreachable
```

---

## 10. SSA 与内存形式

NIR 可以采用混合形式：

* 标量与不可变临时值使用 SSA
* 可寻址 Local 使用显式 Stack Slot
* 聚合值允许拆分为 SSA 字段
* Borrow 指向稳定 Place
* Async Frame 和 Closure Environment 使用显式存储

NIR 不要求所有值强制进入内存，也不要求所有值纯 SSA。

后端可以执行：

* Mem2Reg
* Scalar Replacement
* Dead Store Elimination
* Copy Elision
* Drop Elision

但不得改变可观察 Drop 与 Host 调用顺序。

---

## 11. 所有权在 NIR 中的表达

ADR-002 中的所有权结果必须在 NIR 中显式保留。

调用示例：

```nexa
function consume(
  take first: A,
  second: B,
  mut third: C
): Unit;
```

NIR 调用概念上为：

```text
arg0 = move first
arg1 = borrow_shared second
arg2 = borrow_mut third

call consume(arg0, arg1, arg2)

end_borrow arg2
end_borrow arg1
```

NIR Verifier 必须验证：

* Move 后源不可再次读取
* Borrow 不执行 Drop
* `take` 参数接收 Owned 值
* `mut` 参数接收唯一 Mutable Borrow
* Borrow 生命周期包含 Call
* Call 返回后临时 Borrow 按规则结束

LLVM Backend 不允许重新判断参数是 Borrow 还是 Move。

---

## 12. Drop Lowering

进入 NIR 前，MIR 必须完成 Drop Elaboration。

NIR 中必须明确：

* 哪些控制流边执行 Drop
* 哪些值已经 Move
* 哪些字段需要 Drop
* Drop Glue 调用目标
* Panic Abort 路径是否跳过 Drop

示例：

```nexa
function example(): Unit {
  const first = createFirst();
  const second = createSecond();
}
```

降低后概念控制流：

```text
first = call createFirst
second = call createSecond
drop second
drop first
return
```

后端可以证明 Drop 无副作用时消除，但不能默认省略用户可观察 Drop。

---

## 13. Drop Glue

每个需要 Drop 的类型生成或引用一个 Drop Glue 函数。

概念签名：

```text
drop_in_place<T>(MutBorrowPtr<T>) -> Unit
```

Drop Glue 负责：

1. 调用用户自定义 Drop。
2. 按语言规定顺序 Drop 字段。
3. 释放拥有的堆存储。
4. 处理当前 Enum Variant Payload。
5. 对 Vec、String、Shared 等调用 Runtime Intrinsic。

Drop Glue 不是公开 Nexa API。

稳定 ABI 不允许外部代码直接依赖 Drop Glue 符号名称。

---

## 14. 泛型与 NIR

### 14.1 第一阶段策略

LLVM 第一阶段采用单态化：

```text
function map<T, U>
```

根据实际使用生成：

```text
map<Int, String>
map<User, UserDto>
```

NIR 中每个实例拥有独立 Function Instance ID。

---

### 14.2 泛型身份

必须区分：

```text
GenericDefinitionId
GenericInstanceId
TypeArguments
```

后端只接收已确定的实例，不能再次推断类型参数。

---

### 14.3 包体积控制

后续允许两种代码生成模式：

#### Speed Mode

* 单态化
* 强内联
* 特化
* 更大产物

#### Size Mode

* 代码共享
* 类型擦除
* Dictionary Passing
* 较少特化

两种模式必须保持相同语言语义。

---

### 14.4 ABI 边界

稳定 ABI 不直接暴露开放泛型函数。

禁止：

```text
export function sort<T>(...)
```

作为稳定 C/Host ABI。

可以导出：

* 已单态化实例
* Type Descriptor 驱动的动态接口
* 组件接口语言生成的泛型等价物
* 固定类型包装函数

---

## 15. 闭包表示

闭包降低为：

```text
Function Pointer / Function ID
+
Environment Pointer
```

概念结构：

```text
Closure {
    invoke: ClosureInvokeId,
    environment: OwnedPtr<Environment>
}
```

Environment 包含已经确定的捕获：

* Copy 捕获
* Move 捕获
* Shared 捕获

普通 Borrow 不允许存在于逃逸 Closure Environment。

闭包调用 ABI 概念上为：

```text
invoke(environment, args...) -> result
```

闭包内部布局不属于稳定 ABI。

跨 ABI 暴露回调时必须使用显式 Callback Descriptor。

---

## 16. Async 表示

Async 函数降低为状态机。

概念结构：

```text
AsyncFrame {
    state_tag,
    owned_captures,
    live_locals,
    pending_child,
    result_storage
}
```

Async Frame 中不得保存普通 Borrow。

NIR 必须明确：

* 构造 Frame
* Poll/Resume
* 状态切换
* 完成值
* 取消路径
* Frame Drop

初期 Async Runtime ABI 可以保持内部不稳定。

跨 Host 边界使用稳定 Task Handle，而不是暴露 Async Frame 布局。

---

## 17. Runtime Intrinsic 边界

NIR 可以调用后端中立 Intrinsic。

建议分类：

```text
Memory Intrinsic
String Intrinsic
Array/Vec Intrinsic
Shared/Weak Intrinsic
Panic Intrinsic
Host Intrinsic
Async Intrinsic
Debug Intrinsic
```

概念例子：

```text
nexa.alloc
nexa.dealloc
nexa.string_from_utf8
nexa.vec_reserve
nexa.shared_clone
nexa.panic_abort
nexa.host_call
```

Intrinsic 使用稳定逻辑 ID，不使用源级函数名查找。

---

## 18. Intrinsic Registry

编译器必须建立 Intrinsic Registry。

禁止继续通过：

```text
if function_name == "print"
```

判断特殊函数。

建议模型：

```rust
pub enum IntrinsicId {
    MemoryAlloc,
    MemoryDealloc,
    StringLength,
    StringConcat,
    VecReserve,
    SharedClone,
    SharedDrop,
    PanicAbort,
    HostCall,
}
```

Intrinsic Registry 决定：

* 类型签名
* 所有权模式
* 是否可失败
* 是否有副作用
* 是否可以常量折叠
* 是否可以内联
* 后端 Lowering 方法

---

## 19. LLVM 后端边界

LLVM 相关代码只能存在于专用层：

```text
crates/
  nexa_nir/
  nexa_nir_verify/
  nexa_codegen/
  nexa_codegen_llvm/
  nexa_link/
```

Compiler Core 依赖方向：

```text
nexa_compiler
  → nexa_hir
  → nexa_mir
  → nexa_nir
  → nexa_codegen abstraction

nexa_codegen_llvm
  → nexa_nir
  → LLVM binding
```

禁止：

```text
nexa_hir → LLVM
nexa_mir → LLVM binding
nexa_parser → LLVM
nexa_typechecker → LLVM DataLayout
```

---

## 20. LLVM 类型映射

LLVM Backend 根据 NIR 和 Target Data Layout 映射类型。

示例方向：

```text
I32       → i32
I64       → i64
F64       → double
RawPtr<T> → ptr
Struct    → LLVM struct
Function  → LLVM function type
```

NIR 不直接记录：

* LLVM Context
* LLVM TypeRef
* LLVM ValueRef
* LLVM AttributeRef
* LLVM CallingConv 数字

这些属于后端实现细节。

---

## 21. 目标数据布局

NIR 必须区分：

```text
逻辑类型布局
目标物理布局
稳定 ABI 布局
```

### 21.1 逻辑布局

由 Nexa 类型系统定义字段和 Variant。

### 21.2 目标物理布局

由 Target 决定：

* Pointer Width
* Alignment
* Endianness
* Register Classification
* Aggregate Passing
* Stack Alignment

### 21.3 稳定 ABI 布局

由 ABI 规范显式冻结，不依赖后端默认选择。

公开 ABI 类型必须声明：

* 字段顺序
* 固定宽度类型
* 对齐
* Padding 规则
* Endianness 要求
* Tag 类型
* 所有权规则

---

## 22. ABI 分层

Nexa 将 ABI 分为四层。

### 22.1 编译器内部 ABI

包括：

* MIR API
* NIR Rust API
* NIR 序列化
* Drop Glue 名称
* LLVM IR
* 内部 Runtime 调用

**不稳定。**

可以在任意 Compiler Minor Version 中变化。

---

### 22.2 Nexa Native ABI

用于同一工具链版本生成的 Nexa 模块互相链接。

初期：

* 不承诺跨编译器版本稳定
* 由 Link Metadata 检查兼容
* 可以采用更高效的内部布局
* 允许 Niche Optimization
* 允许内部 Name Mangling 变化

---

### 22.3 Nexa Host ABI

用于 Nexa 程序与稳定 Host Runtime 交互。

这是稳定 ABI 的核心。

要求：

* 版本化
* 可验证
* 固定类型描述
* 明确所有权
* 明确错误
* 明确 Capability
* 不暴露内部对象布局
* 不暴露 Rust ABI
* 不暴露 LLVM 类型

---

### 22.4 C ABI

用于与操作系统和现有原生库交互。

仅支持 C 可表达的安全子集：

* 固定宽度整数
* 浮点
* 裸指针
* 长度
* C-compatible Struct
* 函数指针
* 显式错误码

高级 Nexa 类型需要 Wrapper。

---

## 23. 稳定 ABI 原则

稳定 ABI 必须遵循：

```text
显式优于推断
Descriptor 优于内部布局
Handle 优于裸地址
固定宽度优于平台模糊类型
版本化优于静默兼容
生成绑定优于手写转换
```

稳定 ABI 不得直接暴露：

* 普通 `String` 内部结构
* `Array<T>` 内部结构
* `Vec<T>` Capacity 布局
* `Shared<T>` 引用计数结构
* Closure Environment
* Async Frame
* Trait/Interface VTable 内部布局
* 默认 Class 物理布局
* 泛型实例内部符号
* Rust `Result`
* LLVM Struct Layout

---

## 24. ABI 类型描述符

稳定 Host ABI 使用显式 Descriptor。

### 24.1 String Descriptor

建议：

```c
typedef struct NexaStringView {
    const uint8_t* data;
    uint64_t length;
} NexaStringView;
```

规则：

* UTF-8
* `length` 是字节数
* 不保证零结尾
* Borrow 生命周期由调用边界定义
* Host 不得长期保存，除非显式 Clone

Owned String 使用不同 Descriptor：

```c
typedef struct NexaOwnedString {
    uint64_t handle;
} NexaOwnedString;
```

释放通过 ABI 函数完成，而不是由 Host 猜测 Allocator。

---

### 24.2 Slice Descriptor

```c
typedef struct NexaSlice {
    const void* data;
    uint64_t length;
    uint64_t stride;
} NexaSlice;
```

Mutable Slice：

```c
typedef struct NexaMutSlice {
    void* data;
    uint64_t length;
    uint64_t stride;
} NexaMutSlice;
```

必须注明：

* 元素 ABI 类型
* 对齐
* 生命周期
* 是否允许 Host 写入
* 是否可以跨调用保存

---

### 24.3 Handle

稳定资源使用：

```c
typedef struct NexaHandle {
    uint32_t slot;
    uint32_t generation;
    uint32_t kind;
    uint32_t reserved;
} NexaHandle;
```

Handle 具体字段可以根据实现调整，但公开 ABI 一旦冻结必须版本化。

Handle 访问必须验证：

* Kind
* Generation
* Capability
* Ownership

---

### 24.4 Result Descriptor

稳定 ABI 不使用语言内部 `Result<T, E>` 布局。

使用：

```text
status code
+
success out parameter
+
error descriptor
```

或显式 Tagged Descriptor。

例如：

```c
NexaStatus nexa_file_open(
    NexaStringView path,
    NexaHandle* out_file,
    NexaError* out_error
);
```

---

## 25. 所有权跨 ABI

每个 ABI 参数必须标注：

```text
borrow
mut_borrow
take
return_owned
shared
handle
```

示例 IDL：

```text
host function file_write(
    file: borrow Handle<File>,
    data: borrow Slice<Byte>
) -> Result<U64, IoError>;
```

消费资源：

```text
host function file_close(
    file: take Handle<File>
) -> Result<Unit, IoError>;
```

规则：

* `borrow` 只在调用期间有效
* `take` 调用开始后所有权转移
* `return_owned` 由调用方负责释放
* `shared` 必须使用稳定 Shared Handle
* ABI 层不执行隐式 Clone

---

## 26. Host ABI

Host ABI 是 Nexa 与平台能力的主要边界。

Host 能力包括：

```text
filesystem
network
time
random
process
environment
ui
clipboard
camera
microphone
gpu
database
```

Host ABI 调用通过：

```text
HostFunctionId
+
Version
+
Capability
+
Typed Signature
```

而不是运行时字符串查找。

概念调用：

```text
call_host host.fs.open(args...)
```

Host ABI Verifier 必须验证：

* 函数 ID 存在
* ABI 版本兼容
* Capability 已授权
* 参数类型匹配
* 所有权模式匹配
* Result 类型匹配

---

## 27. Host ABI 版本

Host ABI 使用独立版本：

```text
Host ABI 1.0
Host ABI 1.1
```

版本策略：

* Patch：修正文档或实现 Bug，不改变布局
* Minor：兼容新增函数、Capability 或可选字段
* Major：允许破坏性改变

产物必须声明：

```text
minimum_host_abi
maximum_tested_host_abi
required_capabilities
```

加载器在执行前拒绝不兼容产物。

---

## 28. 组件与插件 ABI

未来组件系统不能直接依赖 Nexa Native ABI。

插件边界优先使用：

* Nexa Host ABI
* Wasm Component Model
* C-compatible ABI
* 生成式 IDL

插件不得假设：

* 相同 Allocator
* 相同 `String` 布局
* 相同 LLVM 版本
* 相同泛型单态化策略
* 相同 Class 布局
* 相同 Panic 实现

---

## 29. C ABI

Nexa 支持显式 C ABI：

```nexa
extern "C" function add(
  left: I32,
  right: I32
): I32;
```

导出：

```nexa
@export("nexa_add")
extern "C" function add(
  left: I32,
  right: I32
): I32 {
  return left + right;
}
```

C ABI 类型必须限制为：

* 固定宽度数值
* `repr(C)` Struct
* 裸指针
* 显式 Slice Descriptor
* 显式 Handle
* 函数指针
* `Unit`/void

禁止直接跨 C ABI 传递：

* 普通 Class
* 默认 Record
* 内部 Enum
* `String`
* `Array`
* `Vec`
* `Shared`
* Closure
* Async Task
* 泛型函数

除非通过显式 ABI Wrapper。

---

## 30. `repr` 策略

Nexa 提供显式表示标记。

### 30.1 默认表示

```nexa
type User = {
  id: Int;
  active: Bool;
};
```

默认布局不稳定。

编译器可以优化。

---

### 30.2 `@repr(C)`

```nexa
@repr(C)
class NativePoint {
  public x: F64;
  public y: F64;
}
```

要求：

* C 字段顺序
* C 对齐
* 仅允许 C-compatible 字段
* 禁止隐式 Drop 字段
* 禁止 Generic 未实例化类型
* 禁止非稳定 Enum
* 禁止内部引用

---

### 30.3 `@repr(stable)`

用于 Nexa Host ABI 或组件 ABI。

要求比 `repr(C)` 更严格：

* 固定宽度类型
* 明确字节序或限定目标
* 明确对齐
* 明确 Padding
* 明确 Tag
* 生成 ABI Hash
* 修改布局属于 Breaking Change

---

### 30.4 `@repr(transparent)`

允许单字段 Wrapper 使用字段 ABI：

```nexa
@repr(transparent)
opaque type UserId = U64;
```

要求只有一个非零大小字段。

---

## 31. Name Mangling

Nexa Native ABI 的内部符号名可以使用 Name Mangling。

Mangling 输入可以包括：

* Package ID
* Module Path
* Function Name
* Generic Instance
* Calling Convention
* Ownership Mode
* ABI Version

但内部 Mangling 不稳定。

公开符号必须显式导出名称：

```nexa
@export("nexa_plugin_init_v1")
function pluginInit(...): ...;
```

防止编译器升级改变外部符号。

---

## 32. 调用约定

### 32.1 内部调用约定

Nexa Native Internal Calling Convention 可以按 Target 优化。

不承诺跨版本稳定。

---

### 32.2 稳定 Host 调用约定

Host ABI 优先使用：

* C-compatible Calling Convention
* Handle/Descriptor 参数
* 显式 Out Parameter
* 显式 Error Code
* 禁止栈展开穿越边界

---

### 32.3 Ownership 参数与 Calling Convention

Nexa 函数：

```nexa
function enqueue(take task: Task): Result<Unit, Error>;
```

内部 ABI 可以将 `Task` 作为：

* 寄存器值
* Owned Pointer
* SRet/Indirect Aggregate
* Handle

但调用完成后 Caller 必须视为已 Move。

所有权是语义契约，不由寄存器传递方式决定。

---

## 33. 返回值 ABI

后端根据 Target ABI 选择：

* Register Return
* Multiple Register Return
* SRet Hidden Pointer
* Out Parameter

稳定 ABI 中复杂返回值优先使用 Out Parameter：

```c
NexaStatus function_name(
    Args...,
    ResultType* out_result,
    NexaError* out_error
);
```

这样避免不同平台聚合返回分类差异。

---

## 34. Panic、Abort 与 ABI

Nexa 默认：

```text
panic → abort
```

Panic 不允许穿越：

* C ABI
* Host ABI
* Plugin ABI
* Wasm Component ABI

Host ABI 函数如果遇到可恢复问题必须返回错误。

内部不变量破坏可以 Abort 当前进程或隔离实例。

未来即使增加 Unwind，也必须在 ABI 边界 Catch 并转换，不能让语言异常穿越外部调用约定。

---

## 35. OOM 与 ABI

默认 Application Profile：

```text
OOM → abort
```

可失败 Host API 使用显式错误。

ABI 不允许通过：

* Null Pointer
* 未初始化 Out Parameter
* 隐式异常

表达 OOM。

Freestanding 或可恢复分配接口必须返回明确状态：

```text
AllocationError.OutOfMemory
```

---

## 36. Allocator 边界

不得默认让不同模块相互释放对方 Allocator 分配的内存。

稳定规则：

> 谁分配，谁提供释放函数或 Owned Handle。

禁止：

```text
Host 使用 malloc
Nexa 使用自己的 allocator free
```

正确方式：

```text
nexa_string_create
nexa_string_clone
nexa_string_release
```

或：

```text
Owned Handle
+
Host ABI Release Function
```

---

## 37. LLVM 优化约束

LLVM 可以执行任意不改变语言可观察行为的优化。

必须保留：

* 左到右求值
* 每个表达式恰好求值一次
* Host 调用顺序
* 可观察 Drop 顺序
* Panic 检查语义
* 整数溢出检查
* Volatile/Atomic 语义
* Capability 检查
* Resource 关闭行为

LLVM Backend 必须正确标记：

* `readonly`
* `noalias`
* `nonnull`
* `noreturn`
* `nounwind`
* `mustprogress`

但只有在 NIR 语义证明成立时才能设置。

错误的 LLVM Attribute 会造成未定义行为，因此必须经过专门测试和审计。

---

## 38. `noalias` 与 Borrow

Nexa 的 Mutable Borrow 可以向 LLVM 提供 `noalias` 信息。

Shared Borrow 可以提供只读信息，但具体 Attribute 必须保守。

规则：

* 只有 Borrow Checker 证明唯一时才设置 `noalias`
* 不能因为源语言写了 `mut` 就忽略 FFI Alias 风险
* Unsafe Pointer 不默认拥有 `noalias`
* Host ABI 参数除非 Schema 保证，否则不设置激进 Alias Attribute

---

## 39. 内存安全与 LLVM Undefined Behavior

Nexa Safe 语义不能直接建立在未验证的 LLVM UB 假设上。

LLVM Backend 必须特别处理：

* 整数溢出
* 越界访问
* 未对齐访问
* 无效 Enum Tag
* Null Dereference
* Pointer Provenance
* Poison/Undef
* Shift 超宽
* 除零
* Signed Division Overflow
* Invalid Boolean Value

Safe Nexa 操作必须：

* 在进入危险 LLVM 指令前检查
* 或使用不会产生 UB 的 Lowering
* 或由 Verifier 证明条件成立

---

## 40. Bounds Check

Array、Vec、Slice 索引默认执行边界检查。

```nexa
value = items[index];
```

语义：

```text
if index >= length:
    panic BoundsError
```

后端可以在证明安全时消除检查。

Debug 与 Release 都不能静默改为未定义行为。

Unsafe API 可以提供 Unchecked Index。

---

## 41. 整数溢出

普通整数运算始终检查。

LLVM Lowering 可以使用：

* Overflow Intrinsic
* 显式比较
* Trap/Abort Block

不得给普通有符号加法设置错误的 `nsw`，除非已经证明不会溢出。

Wrapping API 才允许使用回绕语义。

---

## 42. NIR Verifier

NIR 必须在代码生成前验证。

Verifier 至少检查：

### 类型

* 指令输入输出类型一致
* Branch 条件是 Bool
* Return 类型匹配
* Field Index 合法
* Variant Payload 匹配

### 控制流

* 每个 Block 有 Terminator
* 所有目标 Block 存在
* SSA 定义支配使用
* Phi/Block Parameter 完整
* 不存在非法 Unreachable Use

### 所有权

* Owned 值恰好 Move 或 Drop
* Move 后不再使用
* Borrow 不 Drop
* Mutable Borrow 唯一
* Borrow 不逃逸
* Host ABI 所有权模式匹配

### ABI

* 导出类型可表示
* `repr(C)` 类型合法
* Stable ABI 类型有布局 Hash
* 泛型未泄漏
* Panic 不跨 ABI
* Out Parameter 初始化路径完整

### Runtime

* Intrinsic ID 存在
* Intrinsic 签名匹配
* Host Function ID 和 Capability 匹配
* Handle Kind 匹配

---

## 43. NIR 序列化

初期 NIR 可以提供调试序列化，用于：

* Snapshot Test
* Differential Test
* Compiler Debug
* Fuzzing
* Self-host Bootstrap 对比

但该格式默认：

```text
unstable
compiler-version-specific
not for package distribution
```

序列化头必须包含：

```text
magic
nir_schema_version
compiler_version
target_profile
feature_flags
content_hash
```

不兼容版本必须拒绝加载。

---

## 44. 稳定产物格式

NIR 不作为稳定分发格式。

长期可以定义独立：

```text
Nexa Component Artifact
或
Nexa Stable Artifact
```

稳定产物可能包含：

* Target-independent Component Interface
* Host ABI Requirements
* Wasm Module
* Native Slices
* Capability Manifest
* ABI Hash
* Debug Metadata
* Signature

其设计另行 ADR。

---

## 45. ABI Hash

每个稳定导出接口生成 ABI Hash。

Hash 输入包括：

* 导出名称
* ABI Version
* Calling Convention
* 参数类型
* 参数所有权模式
* 返回类型
* Error 类型
* Struct 字段顺序
* Alignment
* Enum Tag 值
* Capability
* Async/Sync 属性

不包括：

* 源码路径
* 注释
* Debug 名称
* LLVM 版本
* 内部函数地址
* 优化等级

加载器可以比较 ABI Hash 并拒绝不兼容组件。

---

## 46. 元数据

Native Object 或组件必须携带 Nexa Metadata：

```text
language_version
edition
compiler_version
runtime_abi_version
host_abi_version
target_triple
pointer_width
endianness
required_capabilities
exported_abi_hashes
unsafe_usage
panic_strategy
oom_strategy
```

Metadata 使用独立 Section 或 Sidecar Manifest。

---

## 47. Debug Info

Debug Info 保留：

* Source File
* Line/Column
* Variable Name
* Type Name
* Field Name
* Function Name
* Inlined Call Site
* Ownership Event
* Move Location
* Drop Location

Debug Info 不构成 Runtime Reflection。

Release 可以剥离 Debug Info，而不改变程序语义或 ABI。

---

## 48. Source Map

NIR 中每个有用户可见行为的指令应能映射到 Source Span，包括：

* Call
* Bounds Check
* Overflow Check
* Panic
* Drop
* Host Call
* Move
* Resource Close

这样 Native Runtime 错误能够回到 Nexa 源码位置。

---

## 49. 自举编译器关系

Nexa 自举编译器负责生成 NIR。

第一阶段：

```text
Nexa Compiler Frontend/Middle-end
  → NIR
Rust LLVM Backend
  → Native Object
```

自举完成不要求立即使用 Nexa 重写 LLVM Binding。

Nexa 编写的编译器必须能够：

* 构建 NIR Module
* 运行 NIR Verifier
* 序列化调试 NIR
* 调用后端抽象
* 比较 C1/C2/C3 输出

Rust 后端负责：

* LLVM API
* Target Machine
* Object Generation
* Linker Invocation
* Platform Toolchain

---

## 50. 自举可复现性

自举比较不直接比较 LLVM Object 字节，因为：

* LLVM 版本可能影响布局
* Object Section 顺序可能变化
* Debug Info 可能包含非确定数据

自举阶段首先比较：

```text
Normalized NIR
```

要求：

```text
C2(S) NIR == C3(S) NIR
```

然后再验证：

* ABI Hash 一致
* Export Metadata 一致
* Runtime Behavior 一致
* Release Artifact 在固定工具链下可复现

---

## 51. 后端接口

后端抽象概念：

```rust
pub trait CodegenBackend {
    fn name(&self) -> &str;

    fn capabilities(&self) -> BackendCapabilities;

    fn emit(
        &self,
        module: &NirModule,
        target: &TargetSpec,
        options: &CodegenOptions,
    ) -> Result<CodegenArtifact, CodegenError>;
}
```

能力包括：

```text
native_object
executable
dynamic_library
static_library
wasm_module
debug_info
lto
sanitizers
freestanding
```

该 Rust Trait 不属于稳定外部 API。

---

## 52. Target Specification

目标信息通过独立 `TargetSpec` 输入。

包含：

```text
target_triple
architecture
os
environment
pointer_width
endianness
c_abi
object_format
linker_flavor
panic_strategy
default_allocator
host_abi
cpu_features
```

Parser、HIR 和大多数类型检查不得依赖具体 Target。

以下情况可以在后期验证：

* `Usize`
* `Isize`
* `repr(C)`
* SIMD
* Atomic Width
* Freestanding Capability
* FFI Availability

---

## 53. Linker Driver

Nexa 工具链提供独立 Linker Driver 层。

职责：

* 收集 Object
* 选择系统链接器
* 链接 Nexa Runtime
* 链接系统库
* 注入 Startup
* 生成 Metadata
* 处理 RPath
* 处理 Static/Dynamic Library
* 产出最终 Artifact

Compiler Frontend 不直接执行平台链接命令。

---

## 54. Runtime 链接策略

Nexa Runtime 分为可裁剪模块：

```text
core
memory
panic
string
collections
shared
async
host
ui
debug
```

应用只链接实际使用模块。

目标：

* 无 async 程序不链接 Async Runtime
* 无 Shared 程序不链接 Shared Runtime
* Freestanding 不链接 OS Runtime
* 无反射程序不链接 Reflection Metadata
* Release 支持 Dead Stripping 和 LTO

---

## 55. 小包体积策略

LLVM Backend 与 Runtime 必须支持：

* Function Sections
* Data Sections
* Dead Code Elimination
* Thin/Full LTO
* Symbol Visibility
* Runtime Feature Splitting
* Panic Abort
* 无默认 RTTI
* 无默认完整 Reflection
* 泛型 Size Mode
* Debug Info 分离

小包体积不能仅依赖“没有 GC”。

---

## 56. WebAssembly

初期 Wasm 路线：

```text
NIR
→ LLVM Wasm Target
→ Wasm Module
```

后续允许：

```text
NIR
→ Dedicated Wasm Backend
```

Wasm ABI 不直接暴露 Native Pointer。

使用：

* Linear Memory Offset
* Handle
* Component Model 类型
* Host Import ID
* Explicit Memory Ownership

Wasm 与 Native 必须共享：

* 语言语义
* NIR Verifier
* 所有权规则
* Drop 规则
* Result 语义
* Capability 语义

---

## 57. JavaScript 后端

JavaScript 不是第一生产后端。

后续可能支持：

* Wasm + JS Adapter
* JavaScript FFI Binding
* NIR-to-JS 调试后端
* UI 开发热更新后端

JavaScript 后端不得成为 Nexa 核心语义的定义者。

若 JS 无法自然表达某些所有权优化，可以使用保守 Runtime Wrapper，但行为必须与 Reference Interpreter 一致。

---

## 58. C 后端

C 后端可以作为：

* Bootstrap 过渡
* Freestanding 支持
* 调试后端
* 不支持 LLVM 平台的备用方案

但 C 不是首要生产后端。

C 后端必须从 NIR 生成，不能读取 HIR 并重新实现语义。

---

## 59. 编译模式

建议提供：

```text
nexa check
nexa run
nexa build
nexa build --release
nexa build --target wasm32
```

### `check`

* Parse
* Resolve
* Type Check
* Ownership Check
* MIR
* NIR 可选
* 不调用 LLVM

### `run`

开发期可以：

* Reference Interpreter
* 低优化 LLVM
* 后续快速后端

### `build`

* LLVM AOT
* 基础优化
* Debug Info

### `build --release`

* 高优化
* LTO
* Dead Stripping
* Strip
* ABI Metadata
* 可复现构建选项

---

## 60. 诊断

后端诊断必须转换成 Nexa 结构化诊断。

不得直接向普通用户暴露：

* LLVM Assertion
* LLVM Type Dump
* C++ Stack Trace
* Raw Linker Command

诊断类别：

| 错误码范围         | 含义               |
| ------------- | ---------------- |
| `E5000–E5099` | NIR 构建错误         |
| `E5100–E5199` | NIR 验证错误         |
| `E5200–E5299` | ABI 错误           |
| `E5300–E5399` | LLVM Lowering 错误 |
| `E5400–E5499` | Linker 错误        |
| `E5500–E5599` | Target 不支持       |
| `E5600–E5699` | Host ABI 不兼容     |

示例：

```text
E5204: exported function uses an ABI-unstable type

  export function load(): String
                          ^^^^^^

`String` uses Nexa's internal runtime representation and cannot be
exported directly through the C ABI.

help: return `NexaOwnedString` or generate a C ABI wrapper
```

---

## 61. 测试策略

### 61.1 NIR Snapshot

为核心语言特性保存规范化 NIR：

* Function Call
* Move
* Borrow
* Drop
* Match
* Class
* Enum
* Closure
* Async
* Result
* Host Call

---

### 61.2 Differential Testing

同一程序分别运行：

```text
Reference MIR Interpreter
LLVM Debug
LLVM Release
Wasm
未来其他后端
```

比较：

* 标准输出
* 返回值
* Panic Span
* Drop Trace
* Host Call Trace
* 错误行为

---

### 61.3 ABI Conformance

为每个稳定 ABI 版本提供：

* C Header Test
* Nexa Host Stub
* Layout Test
* Alignment Test
* Ownership Test
* Error Test
* Version Negotiation Test
* Cross-compiler Test

---

### 61.4 LLVM Upgrade Test

升级 LLVM 前必须运行：

* 全部 Conformance
* NIR-to-LLVM Snapshot
* ABI Layout Test
* Object Metadata Test
* Performance Benchmark
* Package Size Benchmark
* Sanitizer Test

LLVM 升级不能自动改变稳定 ABI。

---

### 61.5 Sanitizer

开发工具链应支持：

* Address Sanitizer
* Undefined Behavior Sanitizer
* Memory Sanitizer
* Thread Sanitizer

Sanitizer 是后端验证工具，不替代 Nexa 的静态内存安全保证。

---

## 62. 性能基线

必须建立：

* 编译时间
* LLVM IR 生成时间
* LLVM 优化时间
* Link 时间
* Incremental Check 时间
* 产物大小
* 启动时间
* 内存峰值
* Runtime Benchmark

至少覆盖：

```text
Hello World
CLI Parser
自举编译器
大型泛型程序
UI 示例
Wasm 示例
并发示例
```

---

## 63. 安全基线

必须验证 Safe Nexa 不会通过正常语言构造产生：

* LLVM UB
* Use-after-free
* Double-free
* Invalid Enum Tag
* Uninitialized Read
* Out-of-bounds Access
* Misaligned Access
* Data Race
* Panic 穿越 FFI
* 错误 Allocator 释放

---

## 64. 实施阶段

### Phase N0：后端边界冻结

* 定义 MIR/NIR 职责
* 定义 NIR 核心类型
* 定义 NIR Function/Block/Instruction
* 定义 Codegen Backend 接口
* 禁止 Compiler Core 依赖 LLVM

### Phase N1：NIR Builder 与 Verifier

* NIR Module
* Type Table
* Function
* Basic Block
* Scalar Operation
* Call
* Return
* Branch
* Verification
* Text Dump
* Snapshot

### Phase N2：所有权 Lowering

* Copy/Move
* Borrow
* Drop
* Drop Glue
* Resource
* Shared Intrinsic
* Ownership Verification

### Phase N3：LLVM 最小后端

* Integer
* Bool
* Function
* Branch
* Call
* Return
* Object Generation
* Linux x86_64

### Phase N4：数据类型

* String
* Record
* Class
* Tagged Union
* Option
* Result
* Array
* Vec
* Closure

### Phase N5：平台

* macOS ARM64/x86_64
* Windows x86_64
* Linux ARM64
* Debug Info
* Linker Driver

### Phase N6：稳定 C ABI

* `repr(C)`
* Export Name
* C Header Generator
* String/Slice Descriptor
* Handle
* Error Descriptor
* ABI Test

### Phase N7：Host ABI

* Host Function Registry
* Capability
* Version Negotiation
* Host Descriptor
* ABI Hash
* Runtime Metadata

### Phase N8：Wasm

* LLVM Wasm
* Linear Memory ABI
* JS Adapter
* Browser Host
* Component Metadata

### Phase N9：优化

* LTO
* Size Mode
* Specialization
* Drop Elision
* Bounds Check Elimination
* Incremental NIR Cache

---

## 65. 验收标准

### 架构

* Compiler Core 不依赖 LLVM。
* LLVM Backend 只消费合法 NIR。
* 后端不执行语言级名称和类型解析。
* NIR 有独立 Verifier。
* Reference Interpreter 与 LLVM 使用同一语义输入。

### 所有权

* 所有 Move、Borrow 和 Drop 在 NIR 中明确。
* LLVM Backend 不重新推断 Drop。
* 每个 Owned 值恰好 Move 或 Drop。
* Host ABI 参数拥有明确所有权模式。

### ABI

* 默认 Nexa Native ABI 不对外承诺跨版本稳定。
* C ABI 仅允许显式兼容类型。
* Host ABI 版本化并具有 ABI Hash。
* `String`、`Array`、`Result` 不泄漏内部布局。
* Panic 不穿越稳定 ABI。
* 不跨模块错误释放 Allocator 内存。

### 跨平台

* Linux、macOS、Windows 行为一致。
* WebAssembly 与 Native 共享语言语义。
* 固定测试程序的输出、错误和 Drop Trace 一致。

### 性能

* `nexa check` 不依赖 LLVM。
* Debug 构建可以跳过高成本优化。
* Release 支持 LTO 和 Dead Stripping。
* Runtime 按功能裁剪。
* 无 tracing GC Runtime。

---

## 66. 被拒绝的方案

### 66.1 直接从 HIR 生成 LLVM

拒绝原因：

* 后端承担过多语言语义
* 难以验证
* 不利于多后端
* 自举边界不清楚

---

### 66.2 直接把 MIR 当永久公共字节码

拒绝原因：

* MIR 仍包含语言级结构
* 会冻结编译器优化空间
* 安全验证和运行时需求不同
* 当前没有明确 VM 分发需求

---

### 66.3 冻结完整 Nexa Native ABI

拒绝原因：

* 过早冻结对象布局
* 限制泛型和优化
* 限制 LLVM 升级
* Class、Closure、Async 尚未稳定
* 多平台 ABI 差异巨大

---

### 66.4 直接暴露 LLVM IR

拒绝原因：

* 与 LLVM 版本强绑定
* 不安全
* 不稳定
* 不能表达 Nexa Host Capability
* 不适合作为包或插件接口

---

### 66.5 使用 Rust ABI

拒绝原因：

* Rust ABI 不稳定
* Nexa 不能依赖 Rust 编译器内部实现
* 会阻断非 Rust Runtime 和自举

---

### 66.6 跨 ABI 直接传递 Nexa String/Vec

拒绝原因：

* Allocator 不一致
* 内部布局可变
* 所有权不清晰
* 版本升级风险高

---

## 67. 最终决定

Nexa 后端架构最终确定为：

```text
Language Semantics
  ↓
Typed MIR
  ↓
Ownership and Drop Lowering
  ↓
NIR
  ↓
NIR Verifier
  ↓
LLVM / Wasm / Other Backend
```

NIR 的核心职责为：

```text
后端中立
强类型
显式控制流
显式所有权
显式 Drop
显式 ABI
可验证
```

LLVM 的职责为：

```text
目标类型映射
LLVM IR 生成
机器优化
对象文件生成
Debug Info
目标平台适配
```

LLVM 不成为 Nexa 语言语义的一部分。

ABI 稳定策略最终确定为：

```text
编译器内部 ABI：
  不稳定

Nexa Native ABI：
  初期仅同工具链兼容，不承诺跨版本稳定

C ABI：
  对显式 @repr(C) 子集稳定

Nexa Host ABI：
  版本化、可验证、明确所有权与 Capability

组件 ABI：
  基于 Host ABI、C ABI 或 Wasm Component，而非内部布局
```

Nexa 不通过冻结全部对象布局换取表面的 ABI 稳定，而是通过：

```text
Descriptor
Handle
IDL
ABI Hash
Version Negotiation
Generated Binding
```

建立真正可维护的稳定边界。

---

## 68. 后续 ADR

本 ADR 的边界已由以下决策按依赖顺序细化：

1. [ADR-004：Allocator、Arena、集合与运行时内存布局](004-runtime-memory-layout.md)
2. [ADR-005：Host ABI、Capability 与资源 Handle](005-host-abi-capabilities-handles.md)
3. [ADR-006：Result、Panic、defer 与跨 ABI 错误模型](006-errors-panic-defer-abi.md)
4. [ADR-007：Async 状态机、Structured Concurrency 与并发安全](007-async-structured-concurrency.md)
5. [ADR-008：WebAssembly、JavaScript FFI 与跨平台 UI Host](008-wasm-js-ffi-ui-host.md)
6. [ADR-009：包、Lockfile、组件产物与签名](009-package-lockfile-artifacts-signing.md)
