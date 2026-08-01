# ADR-000：Futao 编译器自举路线

* **状态**：Accepted
* **日期**：2026-08-01
* **决策范围**：Futao 编译器、标准库最小子集、运行时、构建链与发布流程
* **优先级**：最高
* **前置关系**：本 ADR 优先于完整标准库、Host ABI、包管理器和 UI 路线
* **目标**：使 Futao 编译器能够使用 Futao 编写、编译自身，并形成可复现、可回滚、可验证的自举链

> [!IMPORTANT]
> 本 ADR 接受 ADR-003 与 ADR-009 的边界：自举 Stage 输出是同工具链内部、版本可变的
> target-neutral NIR，不是公共 VM bytecode，也不承诺 `.nca`。可分发稳定组件仍使用
> ADR-009 的 `.nexc`，不得包含供第三方解释的 MIR/NIR。历史正文中的 Nexa 名称按
> ADR-010 视为 Futao 的旧标识。

---

## 1. 背景

Nexa 当前已经具备一条由 Rust 实现的完整语言处理链：

```text
Nexa Source
  → Lexer
  → Parser / CST
  → Name Resolution
  → Type Checking
  → HIR
  → CFG MIR
  → Reference Interpreter
```

当前实现已经能够支撑：

* 多文件程序
* 静态类型系统
* 函数和闭包
* 泛型
* 记录类型
* Tagged Union
* 模式匹配
* HIR 和 MIR
* 解释执行
* Conformance Test
* 基础 Fuzz 和 Benchmark

因此，Nexa 已经具备讨论自举的前提。

但当前语言生态仍有几个限制：

* 编译器完全由 Rust 实现
* 标准库不完整
* 部分 Builtin 硬编码在 HIR、MIR 和解释器中
* 缺少可验证、可序列化的内部 NIR Stage 产物
* 缺少把自托管 compiler core 接入 Rust verifier/backend 的稳定内部边界
* 缺少包管理和锁文件
* 缺少高效的 Map、Set、StringBuilder、Arena 等编译器基础数据结构
* 当前测试主要证明 Rust 实现内部自洽，尚不能证明存在第二个独立实现

如果直接开始完整标准库或 UI，会出现一个问题：

> Nexa 的上层生态将长期建立在 Rust 编译器私有实现细节之上，而语言本身无法验证自己是否足以实现复杂软件。

因此，必须首先确定 Nexa 的自举路线。

---

## 2. 决策摘要

Nexa 采用**渐进式混合自举**，而不是一次性将整个工具链重写为 Nexa。

最终结构：

```text
┌────────────────────────────────────────────┐
│       Futao Compiler written in Futao      │
│ Lexer / Parser / Resolver / Type Checker   │
│ HIR / MIR Lowering / Diagnostics           │
├────────────────────────────────────────────┤
│       Internal Versioned NIR Artifact       │
├────────────────────────────────────────────┤
│       Runtime and Backends in Rust         │
│ Memory / Verifier / LLVM / Wasm / Host     │
└────────────────────────────────────────────┘
```

核心决策：

1. 使用现有 Rust 编译器作为 **Stage 0 编译器**。
2. 使用 Futao 重写编译器前端和中端。
3. 自举 compiler core 输出 target-neutral 内部 NIR，由 Rust verifier/backend/runtime 形成可运行编译器。
4. 内部 NIR 必须版本化、确定且可规范化比较，但不作为公共字节码或稳定分发格式。
5. Runtime、内存管理、独立 NIR verifier、LLVM/Wasm 后端和 Host adapter 初期继续使用 Rust。
6. 自举不要求将整个运行时重写成 Futao。
7. 建立专用的 Bootstrap Profile 和 Bootstrap Stdlib。
8. 编译器核心必须保持纯函数式接口，文件读写由外部 CLI Host 完成。
9. Stage 2 与 Stage 3 的内部 NIR 必须能够进行规范化后一致性比较。
10. Rust 编译器在 Futao 自举稳定至少两个版本后，才允许降级为备用实现。

---

## 3. 自举的定义

Nexa 将自举能力划分为五个等级。

### Level 0：非自举

```text
Rust Compiler
  → Compiles Nexa Applications
```

当前处于这一阶段。

### Level 1：前端自托管

Lexer、Parser、Resolver 使用 Nexa 编写，但仍由 Rust 编译器编译和运行：

```text
Rust Compiler
  → Nexa Frontend
  → Nexa Application
```

此阶段验证 Nexa 能否表达编译器基础逻辑。

### Level 2：完整编译器自托管

Lexer、Parser、Resolver、Type Checker、HIR 和 MIR Lowering 都使用 Nexa 编写：

```text
Rust Stage 0
  → Nexa Compiler
  → Compiles Nexa Applications
```

编译器已经由 Nexa 实现，但首次构建仍依赖 Rust Stage 0。

### Level 3：编译器自举

使用 Stage 0 编译 Futao 编译器得到 Stage 1，再使用 Stage 1 编译自身得到 Stage 2：

```text
C0 = Rust Compiler

C1 = C0(Futao Compiler Source)

C2 = C1(Futao Compiler Source)

C3 = C2(Futao Compiler Source)
```

满足：

```text
normalize(C2 artifact) == normalize(C3 artifact)
```

即认为完成基本自举。

### Level 4：可复现和可信自举

满足：

* Stage 2 与 Stage 3 目标无关产物确定性一致
* Linux、macOS、Windows 生成的目标无关产物一致
* 编译器版本、标准库版本和产物格式被锁定
* 支持从固定来源与校验和的最小可信 Stage 0 重新构建
* 支持独立实现或 Diverse Double Compilation 验证

Level 4 才是 Nexa 正式替换默认 Rust 编译器的条件。

---

## 4. 自举边界

### 4.1 使用 Nexa 重写的部分

以下部分进入自举范围：

```text
Lexer
Parser
Syntax Tree
Name Resolution
Module Resolution
Type Checker
Generic Instantiation
Pattern Exhaustiveness
HIR Construction
MIR Lowering
Diagnostic Construction
Compiler Driver Core
NIR Construction and Canonical Serialization
```

这些部分主要处理结构化数据和确定性算法，适合使用 Nexa 实现。

### 4.2 初期保留 Rust 的部分

以下部分初期不进行自举：

```text
Allocator and Memory Manager
NIR Verifier
Host ABI
File System Adapter
Process Management
Wasm Backend
Native Backend
JIT
Debugger Transport
Package Downloading
Cryptographic Verification
```

原因：

* 涉及底层内存和平台能力
* 需要较高的安全边界
* Nexa 当前缺少裸指针、手动内存管理和系统调用能力
* 重写这些部分不会直接验证语言前端设计
* 会显著延长自举周期

因此，本 ADR 中的“自举”定义为：

> 编译器前端和中端由 Nexa 编写，并能够编译自身；底层 Runtime 和机器相关后端继续由 Rust 提供。

---

## 5. 为什么不追求全栈自举

将 Runtime、内存管理和后端全部改写为 Futao，不是当前目标。

完全自举会形成以下依赖：

```text
编译器依赖运行时
运行时依赖编译器
标准库依赖两者
后端又依赖运行时数据布局
```

在 Nexa 尚未稳定时，这会导致：

* 无法确定问题发生在语言、编译器还是运行时
* 每次语言变更都需要同时修改多个自举层
* 初始引导链过长
* 调试能力下降
* 失去 Rust 工具链提供的安全边界
* 纯 AI 开发容易形成大规模内部自洽但不可验证的系统

采用混合自举后：

```text
Nexa 负责表达语言语义
Rust 负责实现可信执行底座
```

两者边界清晰。

---

## 6. Bootstrap Profile

自举编译器不能立即使用 Nexa 的全部新特性。

需要定义一个稳定的 **Bootstrap Profile**，编译器自身只能使用该子集。

### 6.1 允许使用

第一版允许：

* `Int`
* `Bool`
* `String`
* `Unit`
* `Array<T>`
* `Option<T>`
* `Result<T, E>`
* 记录类型
* Tagged Union
* 模式匹配
* 普通函数
* 泛型函数
* 闭包
* `if`
* `for`
* 显式模块导入
* 不可变局部变量
* 确定性标准库

### 6.2 暂不允许使用

编译器源码暂不使用：

* UI
* 网络
* 异步
* 线程
* 反射
* 宏
* 动态加载
* 插件
* 动态类型
* 不稳定实验语法
* 尚未经过两个稳定版本支持的新特性

### 6.3 两代编译器规则

新语言特性不能立即用于编译器自身。

规则：

```text
Nexa 1.1 引入特性 X
Nexa 1.2 继续支持并稳定特性 X
Nexa 1.3 才允许编译器源码使用特性 X
```

也就是：

> 编译器源码只能依赖至少被前两代稳定编译器支持的语言能力。

这样可以避免新编译器只能被自己编译的鸡生蛋问题。

---

## 7. Bootstrap Stdlib

自举不依赖完整标准库，而依赖一套小型、稳定、专用的 Bootstrap Stdlib。

目录建议：

```text
stdlib/
  bootstrap/
    core.nexa
    option.nexa
    result.nexa
    array.nexa
    string.nexa
    string_builder.nexa
    string_map.nexa
    int_map.nexa
    bit_set.nexa
    arena.nexa
    diagnostics.nexa
```

### 7.1 必须提供的能力

#### 基础类型

* `Option<T>`
* `Result<T, E>`
* `Ordering`
* `Span`
* `SourceId`

#### 数组能力

* `map`
* `filter`
* `fold`
* `find`
* `append`
* `concat`
* `sortBy`
* `binarySearch`

#### 字符串能力

* Unicode code point 遍历
* UTF-8 字节访问
* 子串
* 前缀和后缀检查
* 数字解析
* `StringBuilder`

#### 编译器集合

第一阶段不必立即设计完整泛型 `Map<K, V>`。

优先提供：

```text
StringMap<V>
IntMap<V>
StringSet
IntSet
BitSet
Arena<T>
```

这些集合必须保证：

* 确定性迭代顺序
* 不依赖随机 Hash Seed
* 跨平台输出一致
* 可限制最大容量
* 错误可诊断

### 7.2 Bootstrap Stdlib 冻结规则

Bootstrap Stdlib 和普通标准库分离：

```text
nexa:bootstrap/*
nexa:core
nexa:array
nexa:string
```

编译器只能依赖 `nexa:bootstrap/*`。

普通标准库可以快速演进，Bootstrap Stdlib 必须：

* API 小
* 行为确定
* 长期兼容
* 无 Host 副作用
* 无包管理依赖
* 无 UI 依赖

---

## 8. 纯编译器核心

自举编译器核心不直接访问文件系统。

接口应设计为：

```nexa
type SourceFile = {
  id: SourceId;
  path: String;
  content: String;
};

type CompilerOptions = {
  languageVersion: String;
  target: Target;
  optimization: OptimizationLevel;
};

type CompilerInput = {
  files: Array<SourceFile>;
  options: CompilerOptions;
};

type CompilerOutput = {
  diagnostics: Array<Diagnostic>;
  nir: Option<InternalNirModule>;
};

function compile(input: CompilerInput): CompilerOutput;
```

外层 CLI 负责：

```text
读取文件
解析 manifest
构建 SourceFile 数组
调用 compile
输出 diagnostics
验证并写入内部 NIR
```

这样可以获得：

* 无文件系统依赖的编译器核心
* 完全内存化测试
* 浏览器 Playground 复用
* Mock 输入
* 确定性构建
* 更容易进行 Differential Testing
* 更容易在未来运行于 Wasm

---

## 9. 内部自举产物边界

自举依赖可验证、可规范化的内部 NIR Stage 产物，但不依赖稳定公共字节码。

```text
Futao compiler core
  -> internal NIR
  -> independent Rust NIR verifier
  -> Rust LLVM/Wasm backend
  -> runnable compiler
```

内部 NIR schema 与 compiler release 一同版本化，可以在 `0.0.x` 之间不兼容演进。
旧工具链遇到未知 schema 必须 fail closed。它可以有用于审查的 canonical text dump 和用于
Stage 构建的 binary encoding，但二者都属于 private compiler artifact，不能作为 package、
plugin 或第三方 loader 的稳定输入。

### 9.1 Artifact 分类

```text
internal NIR Stage artifact
  stability: toolchain-internal
  consumer: pinned Rust verifier/backend
  public extension: none

stable component artifact
  stability: public versioned contract
  format: .nexc
  consumer: component loader
  contains MIR/NIR: false
```

`.nca` 和“公共消费者解释 NIR/MIR”的方案不采纳。Source package、private compiler
cache、runnable artifact 与 stable `.nexc` component 继续遵循 ADR-009 的四类产物边界。

### 9.2 版本与 provenance

Stage manifest 至少绑定：

```text
manifest schema version
compiler and language versions
Bootstrap Stdlib version and digest
internal NIR schema version
Stage 0 repository, full commit and source digest
locked build inputs and Rust toolchain
verifier/backend identity
target-independent feature/profile inputs
normalized Stage output digest
```

当前 `0.0.2` manifest 固定 `nexac 0.0.1` source commit、source archive SHA-256、
`Cargo.lock` SHA-256、Rust 1.80 和 locked release recipe。由于 `v0.0.1` 尚无 tag 或
GitHub Release，distribution 明确记录为 `source-only`，不得伪造 binary provenance。

### 9.3 确定性与规范化

内部产物和规范化输入禁止包含：

* 当前时间、签名时间或 transparency receipt
* 随机 UUID、进程 ID、用户名或主机名
* workspace 绝对路径、临时目录或未 remap 的 source path
* locale、timezone、未声明环境变量或网络结果
* 非确定 HashMap 遍历、object/table 顺序或压缩 metadata

Stage 比较覆盖内部 NIR 的全部逻辑 section、schema、ABI hash、Bootstrap Stdlib hash 和
feature/profile 输入。签名 envelope 与发布传输 metadata 位于逻辑 digest 外，不能通过
扩大 normalization 忽略具有语义影响的差异。

---

## 10. Stage 编译流程

设：

```text
S  = Futao 编译器源码
C0 = Rust Stage 0 编译器
B0 = Rust NIR verifier、backend 与 runtime/Host 底座
```

### 10.1 Stage 1

使用 Rust 编译器编译 Futao 编译器源码：

```text
C1 = C0(S)
```

`C1` 的 compiler core 输出内部 NIR，经 `B0` 验证和后端处理后形成可运行编译器。

### 10.2 Stage 2

使用 `C1` 再次编译相同源码：

```text
C2 = C1(S)
```

### 10.3 Stage 3 验证

再次使用 `C2` 编译：

```text
C3 = C2(S)
```

要求：

```text
normalize(C2) == normalize(C3)
```

最低要求是 C2 与 C3 完全一致。

C1 可能因 Stage 0 的实现细节存在可解释差异；`0.1.0` 的硬门槛是 C2/C3 一致。

---

## 11. 一致性验证

不能只比较程序运行结果。

需要分层比较两个编译器。

### 11.1 Lexer 一致性

同一源码的 Token 必须在以下字段一致：

```text
kind
text range
source span
literal value
error code
```

### 11.2 Parser 一致性

比较：

* Syntax Tree 结构
* 节点类型
* 子节点顺序
* Source Span
* 错误恢复位置
* Diagnostic Code

诊断文案可以暂时不同，但：

```text
code
severity
primary span
related spans
```

必须一致。

### 11.3 Type Checker 一致性

比较：

* 符号解析结果
* 推断类型
* 泛型实例
* 模式穷尽结果
* 类型错误 Code
* 类型错误位置

### 11.4 HIR/MIR 一致性

定义 Canonical HIR 和 Canonical MIR。

在比较前移除：

* 内存地址
* 临时随机 ID
* 构建时间
* 无意义的内部编号差异

然后比较规范化 Hash。

### 11.5 运行行为一致性

同一程序由两个编译器生成的产物，在相同 Runtime 上执行，必须得到：

* 相同返回值
* 相同标准输出
* 相同 Host 调用序列
* 相同错误 Code
* 相同资源限制行为

---

## 12. Differential Testing

自举期间同时保留：

```text
Rust Reference Compiler
Futao Self-hosted Compiler
```

所有测试都执行两遍：

```text
input
  ├── Rust Compiler
  └── Futao Compiler
```

再比较：

```text
Diagnostics
Canonical HIR
Canonical MIR
Canonical NIR
Runtime Behaviour
```

### 12.1 测试来源

* Conformance Cases
* 标准库源码
* 编译器源码
* 随机语法程序
* 随机类型程序
* Fuzz Corpus
* 历史 Bug Regression
* 手写复杂项目
* 变异测试生成的程序

### 12.2 差异处理

任何差异都必须归类：

```text
Rust Compiler Bug
Futao Compiler Bug
Specification Ambiguity
Canonicalization Bug
Allowed Diagnostic Text Difference
```

不得以“两个实现结果差不多”为理由忽略。

---

## 13. 自举阶段路线

### Phase B0：冻结当前语义基线

#### 目标

为自举建立稳定语言版本。

#### 工作项

* 冻结 Language 1.0 语义
* 冻结 Token 和 Diagnostic Code
* 明确 AST、HIR、MIR 不变量
* 建立 Canonical HIR/MIR
* 增加 MIR Validator
* 建立完整 Conformance Matrix
* 固定 Rust Stage 0 source commit、source archive 与 locked input hash
* 在二进制实际发布后补充平台资产 digest；发布前保持 `source-only`

#### 退出条件

* 无已知 P0/P1 语言语义问题
* 所有语义规则有正反用例
* Rust Stage 0 可以从固定源码稳定构建
* Conformance 在三大平台通过

---

### Phase B1：稳定编译器库边界

#### 目标

将现有 Rust 编译器整理成可被第二实现对照的参考架构。

#### 工作项

* 拆分 Compiler Core 和 CLI
* 编译器核心改为内存输入输出
* 文件系统逻辑移出 Compiler Core
* 定义 `CompilerInput`
* 定义 `CompilerOutput`
* 定义 Canonical Diagnostics
* 定义内部 NIR Stage schema 与 canonical dump
* 定义独立 Rust NIR Verifier

#### 退出条件

Rust 编译器可以完全基于内存中的文件集合编译项目。

---

### Phase B2：Bootstrap Stdlib

#### 目标

提供实现编译器所需的最小库。

#### 工作项

* `Option`
* `Result`
* `StringBuilder`
* `StringMap`
* `IntMap`
* `BitSet`
* `Arena`
* 确定性排序
* Span 和 Diagnostic 数据结构
* Bootstrap Stdlib Conformance

#### 退出条件

可以在 Nexa 中实现非平凡文本解析器，而不新增编译器特殊 Builtin。

---

### Phase B3：Nexa Lexer

#### 目标

用 Nexa 重写 Lexer。

#### 工作项

* Source Cursor
* UTF-8 处理
* Token
* Trivia
* Literal
* Error Recovery
* Lexer Differential Test
* Lexer Fuzz

#### 退出条件

完整 Corpus 中，Rust Lexer 与 Nexa Lexer 的 Canonical Token Stream 一致。

---

### Phase B4：Nexa Parser

#### 目标

用 Nexa 重写 Parser。

#### 工作项

* Expression Parser
* Statement Parser
* Type Parser
* Declaration Parser
* Module Parser
* Error Recovery
* Syntax Tree
* Parser Differential Test
* Parser Fuzz

#### 退出条件

所有合法和非法 Conformance Case 的 Canonical Syntax Tree 与 Diagnostic Code 一致。

---

### Phase B5：名称和模块解析

#### 目标

完成多文件语义解析。

#### 工作项

* Module Graph
* Import Resolution
* Symbol Table
* Scope
* Duplicate Symbol
* Circular Import
* Export Resolution
* Resolver Differential Test

#### 退出条件

编译器能够解析自身完整模块图。

---

### Phase B6：Nexa Type Checker

#### 目标

用 Nexa 实现完整静态语义。

#### 工作项

* 基础类型检查
* 函数调用
* 泛型实例化
* Record
* Tagged Union
* Pattern Matching
* Exhaustiveness
* Closure Capture
* Return Analysis
* Type Diagnostic

#### 退出条件

Rust 与 Nexa Type Checker 在完整 Corpus 上产生相同类型结果和错误 Code。

---

### Phase B7：HIR 与 MIR Lowering

#### 目标

完成完整的自托管编译器中端。

#### 工作项

* Typed HIR
* HIR Validation
* CFG Construction
* MIR Lowering
* Closure Conversion
* Control Flow
* MIR Validation
* Canonical MIR Serialization

#### 退出条件

同一输入的 Canonical MIR 与 Rust 编译器一致，或符合经过批准的新规范。

---

### Phase B8：首次自编译

#### 目标

Futao 编译器成功编译自身。

流程：

```text
C1 = Rust Stage 0 编译 Futao Compiler
C2 = C1 编译 Futao Compiler
```

#### 退出条件

* C1 可以编译完整 Futao 编译器源码
* C2 可以运行全部编译器测试
* C2 可以编译普通 Futao 项目
* C2 输出通过独立 Rust NIR Verifier

---

### Phase B9：可复现自举

#### 目标

建立稳定 Stage 构建链。

流程：

```text
C1 = C0(S)
C2 = C1(S)
C3 = C2(S)
```

#### 退出条件

```text
normalize(C2) == normalize(C3)
```

并满足：

* 多平台目标无关产物一致
* 重复构建结果一致
* Stage 信息被写入 Manifest
* CI 自动执行三阶段自举

---

### Phase B10：切换默认编译器

#### 目标

将 Futao 编译器设置为默认实现。

采用双实现模式：

```text
futao build
  → Futao Self-hosted Compiler

futao build --compiler=rust
  → Rust Reference Compiler
```

#### 切换条件

* 自举链连续两个版本稳定
* 没有未解决的 P0/P1 差异
* 性能处于可接受范围
* 自托管编译器可编译标准库和自身
* Rust 编译器可作为回滚路径
* 发布流程能够从固定 Stage 0 重建

---

## 14. 性能目标

自举初期以正确性为主，但必须设置性能边界。

### 初始目标

自托管编译器编译自身：

* 时间不超过 Rust 编译器的 10 倍
* 峰值内存不超过 Rust 编译器的 8 倍
* 不触发 compiler core 的默认资源上限
* 不发生无限递归
* 不产生不可控制的临时字符串

### 默认切换目标

正式替换 Rust 默认编译器前：

* 时间不超过 Rust 编译器的 3 倍
* 峰值内存不超过 Rust 编译器的 3 倍
* 增量前提下普通项目反馈时间可接受
* 诊断首屏不因全项目编译被严重阻塞

如果解释器运行的自托管编译器无法达到目标，可以增加：

```text
Bytecode Interpreter
MIR Optimizer
Wasm Backend
```

但不得通过修改语言语义掩盖性能问题。

---

## 15. 可信自举

编译器自举存在经典的 Trusting Trust 风险：

> 一个被污染的旧编译器，可能在编译新编译器时继续注入恶意逻辑，即使源码中已经看不到该逻辑。

因此必须建立可信链。

### 15.1 Stage 0 固定

每次发布记录：

```text
stage0 compiler version
stage0 source commit
stage0 source archive SHA-256
stage0 locked input SHA-256
Rust toolchain version
bootstrap stdlib hash
language version
internal NIR schema version
stage0 binary SHA-256（仅在实际发布后）
```

### 15.2 多平台重建

至少在：

* Linux
* macOS
* Windows

分别进行 Stage 构建。

生成的 normalized target-neutral NIR 应当一致。

### 15.3 Diverse Double Compilation

达到 Level 4 前，至少执行一种独立验证：

* 使用 Rust 编译器和 Futao 编译器分别构建
* 使用 Native 与 Wasm backend 分别处理同一已验证 NIR
* 使用两个独立 NIR serializer 比较逻辑模型
* 使用独立的最小编译器实现编译 Bootstrap Profile

不要求立即完成完整 DDC，但架构必须保留该能力。

---

## 16. 发布结构

自托管编译器 Release 最终包含：

```text
futao-stage0-source-provenance
futao-compiler-<platform>
futao-bootstrap-stdlib
futao-runtime-<platform>
bootstrap-manifest.json
checksums.txt
```

`bootstrap-manifest.json`：

```json
{
  "schemaVersion": 1,
  "toolchainVersion": "0.0.2",
  "languageVersion": "1.0",
  "stage0": {
    "compilerVersion": "0.0.1",
    "source": {
      "repository": "https://github.com/baicie/nexa",
      "commit": "9293a7b59ff3b6625ca09091f7ad50234638981f",
      "archiveDigest": "sha256:<digest>",
      "cargoLockDigest": "sha256:<digest>"
    },
    "distribution": "source-only"
  },
  "bootstrapStdlib": {
    "status": "not-defined",
    "version": null,
    "digest": null
  },
  "bootstrapOutput": {
    "kind": "internal-nir",
    "stability": "toolchain-internal",
    "publicExtension": null
  }
}
```

`0.0.2` 的完整 checked-in schema 见
[`bootstrap/stage0/bootstrap-manifest.json`](https://github.com/baicie/nexa/blob/mvp/bootstrap/stage0/bootstrap-manifest.json)。
Stage 1/2/3 出现后，manifest 扩展各 Stage 的 normalized NIR digest、verifier/backend
identity 和 reproducibility result；不会把签名时间混入 Stage 比较。

---

## 17. 仓库结构

仓库只在真实 phase boundary 出现时增加目录或 crate，不创建空占位。当前与近期结构为：

```text
crates/                 Rust Stage 0 phases and CLI
bootstrap/
  stage0/               pinned source provenance and build contract
  tests/rejected/       contract-fail fixtures
conformance/            Language 1.0 observable behavior
compiler/futao/         added only when the first .ft compiler slice exists
stdlib/bootstrap/       added only with the accepted Bootstrap Profile
```

在自举稳定前，不建议将自托管编译器拆到独立仓库。

编译器、Bootstrap Stdlib、Runtime 和内部 NIR schema 必须在同一 CI 中验证。

---

## 18. CI 流程

`0.0.2` 起，每个自举相关 PR 至少执行当前已存在的门槛：

```text
Rust unit tests
Rust conformance
Rust compiler unit tests
Bootstrap manifest accepted/rejected tests
Stage 0 source/archive/lock digest verification
Fuzz regression corpus
Performance regression
```

后续能力落地后按阶段追加，不以空任务伪装已实现：

```text
Lexer differential
Parser differential
Type checker differential
HIR differential
MIR differential
NIR verification
Stage 1 build
Stage 2 build
Stage 3 build
Stage 2/3 comparison
Bootstrap compiler compiles stdlib
Bootstrap compiler compiles examples
```

发布 CI 额外执行：

```text
Linux reproducibility
macOS reproducibility
Windows reproducibility
Checksum generation
Bootstrap manifest generation
Release artifact verification
```

---

## 19. 纯 AI 开发约束

Nexa 自举不能继续采用超大 PR 一次合并的方式。

### 19.1 PR 边界

每个 PR 只完成一个明确语义阶段，例如：

```text
Token model
Identifier lexer
Numeric lexer
Expression parser
Import resolver
Generic substitution
Match exhaustiveness
MIR block lowering
```

建议限制：

* 普通 PR 不超过 20 个有效代码文件
* 核心行为修改不超过约 1,000 行
* 机械生成和行为修改必须分开
* 每个 PR 必须包含失败用例
* 每个 PR 必须包含 Rust/Nexa 差异报告

### 19.2 异构审查

至少采用两个不同模型角色：

```text
Model A
  → 实现

Model B
  → 对照语言规范审查

Model C
  → 生成反例、Fuzz Seed 和边界测试
```

最终验收不能只依赖实现模型自述“无 P0/P1”。

### 19.3 禁止合并条件

* Nexa 实现通过，但与 Rust 实现存在未解释差异
* 只比较成功程序，没有比较错误程序
* 新增编译器专用 Builtin
* 使用未进入 Bootstrap Profile 的特性
* 产物包含非确定性字段
* Stage 2 和 Stage 3 不一致
* 性能出现数量级退化但未记录

---

## 20. 与标准库路线的关系

自举优先，但不代表完全停止标准库。

标准库被拆成两条路线：

```text
Bootstrap Stdlib
  → 为编译器自举服务
  → 立即建设
  → API 小且冻结

General Stdlib
  → 为普通应用服务
  → 自举中期开始
  → 可更快速演进
```

正确顺序：

```text
语言语义冻结
  ↓
Bootstrap Profile
  ↓
Bootstrap Stdlib
  ↓
自托管编译器
  ↓
完整通用标准库
```

不是：

```text
完整标准库
  ↓
UI 库
  ↓
最后再考虑自举
```

编译器自举会直接暴露标准库真正缺少的能力，因此自举本身也是标准库需求发现机制。

---

## 21. 与 UI 路线的关系

UI 不需要等待所有自举工作完全结束，但正式 UI Runtime 应等待以下条件：

* Nexa Parser 已经自托管
* Type Checker 已经自托管
* Canonical MIR 已稳定
* internal NIR schema、Verifier 与 canonical comparison 已稳定
* Stage 2 和 Stage 3 可一致
* Rust backend/runtime 与 Host 边界已确定

推荐依赖关系：

```text
ADR-000 自举
    ↓
ADR-001 语言和运行时总体架构
    ↓
ADR-002 所有权、Borrow 与 Drop
    ↓
ADR-003 NIR、LLVM 与 ABI 边界
    ↓
ADR-004 运行时内存布局
    ↓
ADR-005 Host ABI 与资源 Handle
    ↓
ADR-006 错误与跨 ABI 失败
    ↓
ADR-007 Async 与并发
    ↓
ADR-008 Wasm、JavaScript FFI 与 UI Host
    ↓
ADR-009 包、Lockfile、组件产物与签名
```

UI 原型可以在自举中后期探索，但不能反向改变 Bootstrap Profile。

---

## 22. 不采纳方案

### 22.1 继续永久使用 Rust 编译器

不采纳为长期路线。

原因：

* 无法证明 Nexa 足以构建复杂程序
* 语言生态永远依赖 Rust
* 编译器和标准库设计不会受到真实 Nexa 使用压力
* 无法形成语言社区的核心示范工程

Rust 编译器会长期保留，但定位将从默认实现变为参考和恢复实现。

### 22.2 一次性用 Nexa 重写整个编译器

不采纳。

原因：

* 差异范围过大
* 无法定位错误阶段
* PR 无法有效审查
* 自举失败时难以回退
* 纯 AI 容易生成内部一致但整体错误的大型实现

必须按 Lexer、Parser、Resolver、Type Checker、HIR、MIR 分阶段替换。

### 22.3 同时重写 Runtime、后端和编译器

不采纳。

原因：

* 没有稳定执行基准
* 编译器和 Runtime/backend 错误相互掩盖
* 自举链失去可信底座
* 调试成本过高

### 22.4 先开发 Nexa→Native Backend

不采纳为自举前置条件。

第一阶段让自托管 compiler core 生成内部 NIR，并复用 Rust verifier 与 LLVM/Wasm
backend，已经足够完成混合自举；不要求用 Futao 重写机器码后端。

### 22.5 为编译器增加大量专用语法

不采纳。

编译器应成为普通 Nexa 程序。

如果实现编译器必须不断增加特殊语法，说明语言或基础库设计存在问题。

---

## 23. 风险与缓解

### 23.1 自托管编译器性能过低

缓解措施：

* 引入确定性 `StringMap`、`Arena` 和 `StringBuilder`
* 避免大量不可变数组全量复制
* 对热点 Intrinsic 做性能优化
* 建立编译器 Profile
* 优化内部 NIR、Native 或 Wasm Backend

### 23.2 两个编译器语义长期漂移

缓解措施：

* 单一语言规范
* Canonical HIR/MIR
* Differential CI
* 共享 Conformance
* 差异必须有明确分类

### 23.3 Bootstrap Stdlib 变成第二套完整标准库

缓解措施：

* 严格限制模块数量
* 禁止网络、文件、时间和 UI
* 只加入编译器真实使用的 API
* 通用功能下沉到 General Stdlib

### 23.4 Rust 编译器成为无人维护的备用实现

缓解措施：

* 语言新特性仍需更新 Rust Reference Compiler
* 至少保持两个稳定版本的差异测试
* 之后再评估是否将 Rust 实现冻结为最小 Stage 0

### 23.5 新语法破坏旧编译器自举能力

缓解措施：

* Bootstrap Profile
* 两代编译器规则
* 编译器源码不立即采用新特性
* Release 中保留旧 Stage 0

---

## 24. 回滚策略

自托管编译器成为默认后，Rust 编译器至少保留两个稳定版本。

提供：

```bash
futao build --compiler=futao
futao build --compiler=rust
```

当出现以下情况时自动回退 Rust 编译器：

* 自托管编译器崩溃
* NIR Verifier 拒绝产物
* Bootstrap Hash 不匹配
* Stage 2/3 不一致
* 编译器产生内部错误
* 发布构建不可复现

不得在首次完成自举后立即删除 Rust 编译器。

---

## 25. 成功标准

Futao 自举完成必须同时满足：

### 功能

* Futao 编译器由 Futao 编写
* 能编译自身
* 能编译 Bootstrap Stdlib
* 能编译通用标准库
* 能编译真实示例项目

### 一致性

* Rust 与 Futao 编译器通过完整 Differential Test
* C2 与 C3 产物规范化后一致
* 不同平台生成的目标无关产物一致

### 可靠性

* 非法源码不会导致 compiler core、Verifier 或 Runtime 崩溃
* 内部 NIR 必须经过独立 Rust Verifier
* 资源上限有效
* 编译器错误可结构化诊断

### 工程

* 自举过程全自动
* Release 包含完整 Bootstrap Manifest
* Stage 0 可固定和重建
* 支持回滚 Rust 编译器

### 性能

* 自编译时间处于约定预算内
* 无数量级内存退化
* 大型源码不会因算法复杂度失控

---

## 26. 最终决策

Futao 后续总路线调整为：

```text
1. 冻结 Language Core
2. 定义 Bootstrap Profile
3. 建设 Bootstrap Stdlib
4. 稳定 Compiler Core 和内部 NIR schema
5. 用 Futao 重写 Lexer
6. 用 Futao 重写 Parser
7. 用 Futao 重写 Resolver 和 Type Checker
8. 用 Futao 重写 HIR/MIR/NIR Lowering
9. 完成 Stage 1/2/3 自举
10. 建立可复现构建
11. 切换自托管编译器为默认
12. 扩展通用标准库和 Host ABI
13. 按 ADR-004 至 ADR-009 交付完整平台能力
```

核心判断：

> Futao 的下一阶段不应首先追求更多语法、完整标准库或 UI，而应首先证明 Futao 能够实现并编译自己的编译器。

自举不要求立刻用 Futao 重写 Runtime、内存管理和机器码后端。

最合理的架构是：

```text
Futao 编写 compiler core
Rust 提供可信 Runtime 和后端
内部 NIR 与独立 Verifier 连接两者
Differential Testing 保证语义一致
Stage 1/2/3 保证自举可复现
```

这是 Futao 从“语言参考实现”进入“可独立演进语言平台”的关键分界线。
