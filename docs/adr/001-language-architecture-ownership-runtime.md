# ADR-001：Nexa 2.0 语言架构、所有权与运行时模型

* **状态**：Accepted
* **日期**：2026-08-01
* **适用范围**：Nexa 2.0 及后续 Edition
* **关联决策**：ADR-000 Nexa 编译器自举路线
* **不影响范围**：已发布并冻结的 Nexa Language 1.0 兼容基线

---

## 1. 背景

Nexa Language 1.0 已经建立：

* TypeScript-shaped 语法
* 静态类型检查
* Lossless CST
* Typed HIR
* CFG MIR
* 多文件模块
* 泛型
* Record
* Tagged Union
* 模式匹配
* 闭包
* 参考解释器
* 确定性执行语义

但当前 1.0 主要是语言参考核心，尚未完整解决：

* Native AOT
* WebAssembly
* 所有权与借用
* 无 GC 的长期运行内存安全
* Allocator
* Arena
* 可变集合
* Class 和 Interface
* Async/Concurrency
* FFI
* UI Runtime
* 完整工具链

Nexa 后续需要从“参考语言核心”发展为：

> 面向前端和全栈开发者、具有 TypeScript-like 开发体验，同时具备无 tracing GC 内存安全、可预测性能和跨平台能力的通用语言。

由于部分新决策会改变当前 1.0 的类型和数据模型，本 ADR 不作为 1.x 的兼容新增，而作为 Nexa 2.0 或新 Edition 的架构基线。

---

## 2. 决策摘要

Nexa 采用以下总体设计：

```text
TypeScript-like Surface
+
Independent Static Semantics
+
Single Ownership
+
Restricted Borrowing
+
Deterministic Drop
+
No Tracing GC
+
LLVM Native AOT
+
Wasm and Host ABI
```

核心决策：

1. 语法高度参考 TypeScript，但不接受 JavaScript 运行时语义。
2. 默认不使用 tracing GC。
3. 采用单一所有权、确定性 Drop 和受限借用。
4. 函数签名使用 `take` 表示消费所有权，调用处采用隐式 Move 语法糖。
5. 普通复合参数默认只读借用，`mut` 表示唯一可变借用。
6. 第一阶段不允许普通借用逃逸函数。
7. `Shared<T>` 和 `Weak<T>` 提供显式共享，不作为普通值的默认实现。
8. 普通数据使用值语义；小值可内联，大值可装箱。
9. `Array<T>` 不可变，`Vec<T>` 用于唯一所有权下的可变构建。
10. 普通 Record 使用结构类型；Class、Enum、Resource 和 Opaque Type 使用名义类型。
11. Class 不默认具有共享引用身份，也不支持继承。
12. 可恢复错误使用 `Result<T, E>`；`?` 传播，`try/catch` 处理。
13. Panic 默认终止程序，不进行异常栈展开。
14. 第一生产后端为 LLVM Native AOT。
15. WebAssembly 是第二生产目标。
16. 初期不建设独立公共字节码，也不建设 JIT。
17. OXC 可以辅助词法和语法实现，但不能成为 Nexa 的核心 AST/HIR。
18. 默认无完整运行时反射，使用显式 `@derive` 和 `@reflect`。
19. 内核和 Freestanding 能力通过独立 Profile 提供，不牺牲普通应用体验。
20. 语言设计必须兼顾 AI 生成、机器诊断、确定性构建和安全沙箱。

---

## 3. 产品定位

### 3.1 目标用户

主要用户：

* 前端开发者
* 全栈应用开发者
* 跨平台应用开发者
* CLI 和开发工具开发者

长期用户：

* 服务端开发者
* 系统工具开发者
* 嵌入式开发者
* Freestanding 应用开发者
* 操作系统内核与驱动开发者

内核、驱动和硬实时不是第一阶段交付目标，但语言核心不能从架构上阻断这些方向。

---

### 3.2 主要场景

第一阶段：

```text
CLI
Web
桌面
跨平台 UI
服务端应用
开发工具
编译器自举
```

后续阶段：

```text
Wasm 沙箱
插件系统
嵌入式
Freestanding
系统工具
内核和驱动
```

---

### 3.3 核心卖点

Nexa 对外只保留三个核心卖点：

1. **TypeScript-like 的通用编程语言**
2. **无 tracing GC 的内存安全**
3. **比 Rust 更容易使用的所有权模型**

其他目标分为质量目标和生态目标。

质量目标：

* 跨平台
* 确定性执行
* 快速编译
* 可预测性能
* 并发安全
* 小运行时
* 小发布产物

生态目标：

* 自举
* UI 友好
* AI 友好
* 沙箱安全

这些目标不是同等优先。

优先级为：

```text
正确性和内存安全
  >
开发体验
  >
跨平台
  >
确定性和可预测性
  >
编译速度
  >
运行性能
  >
高级功能
```

---

## 4. 非目标

Nexa 2.0 不追求：

* 任意 TypeScript 源码兼容
* JavaScript 运行时兼容
* npm 语义兼容
* `any`
* JavaScript truthiness
* 隐式数字与字符串转换
* `null` 和 `undefined`
* 原型链
* 动态添加和删除对象属性
* `eval`
* Monkey Patch
* Declaration Merging
* Namespace
* 完整 TypeScript 类型级编程
* 默认完整运行时反射
* 默认 tracing GC
* 默认共享可变对象
* 类继承体系
* 默认 JIT
* 第一阶段支持硬实时和内核开发

---

## 5. TypeScript 语法关系

### 5.1 基本原则

Nexa 使用 TypeScript-like 语法，但定义自己的：

* 类型系统
* 所有权规则
* 值表示
* 模块系统
* 错误模型
* 异步模型
* FFI
* Runtime
* ABI
* 内存管理

语法相似不构成 TypeScript 源码兼容承诺。

---

### 5.2 保留的语法形态

计划保留：

```text
function
const
let
type
interface
class
enum
public
private
async
await
try
catch
箭头函数
对象字面量
数组字面量
泛型
import/export
```

但这些关键字在 Nexa 中具有独立语义。

---

### 5.3 `type`

`type` 用于：

* 类型别名
* 结构 Record
* Union 别名
* 函数类型

例如：

```nexa
type Point = {
  x: Int;
  y: Int;
};
```

`type` 声明的普通 Record 采用结构类型，不创建独立名义身份。

---

### 5.4 `interface`

`interface` 是编译期结构契约。

```nexa
interface Named {
  name: String;
}
```

一个类型不需要显式声明 `implements Named`，只要满足所要求的字段和方法即可。

Nexa 不采用完整 TypeScript 结构子类型系统。

具体规则：

* 普通 Record 结构相等时兼容
* Interface 允许实现类型包含额外字段
* 可变字段不参与不安全的协变
* 函数参数和返回值变型采用保守规则
* 不支持 Declaration Merging

---

### 5.5 `class`

Class 是：

```text
名义类型
+ 字段
+ 方法
+ 构造逻辑
+ public/private 可见性
+ 所有权和 Drop
```

示例：

```nexa
class User {
  public name: String;
  private token: String;

  function rename(mut self, name: String): Unit {
    self.name = name;
  }
}
```

Class 第一阶段不支持：

* 继承
* Prototype
* 虚方法继承树
* 运行时方法替换
* 默认隐式共享引用
* 默认地址身份比较

Class 实例遵守单一所有权。

---

### 5.6 `enum`

Nexa 的 `enum` 是名义 Tagged Union，而不是 TypeScript 数字枚举。

```nexa
enum LoadState<T> {
  Idle,
  Loading,
  Loaded(T),
  Failed(String),
}
```

要求支持穷尽模式匹配。

不提供默认数字值、反向映射和字符串枚举对象。

---

### 5.7 `async/await`

语法保留，但编译为静态状态机。

Async Task 必须拥有跨暂停点继续使用的数据。

普通 Borrow 不允许跨越 `await`。

---

### 5.8 `try/catch`

语法保留，但不表示 JavaScript 异常或栈展开。

它用于处理 `Result<T, E>`。

```nexa
try {
  const user = loadUser(id)?;
  render(user);
} catch (error) {
  renderError(error);
}
```

编译器将其降低为结构化的 `Result` 匹配和控制流。

`catch` 不捕获 Panic。

---

### 5.9 TypeScript 迁移

核心编译器不直接接受任意 `.ts` 文件。

后续提供独立工具：

```text
ts-to-nexa
```

该工具可以：

* 转换语法
* 标记无法自动迁移的动态语义
* 调用 Nexa 编译器校验结果
* 使用 AI 辅助重构所有权和错误模型

转换器不属于 Nexa 核心语义。

---

## 6. Parser 与 OXC

Nexa 可以复用 OXC 的部分工程能力，例如：

* Tokenization
* TS-like 表达式解析
* JSX/TSX 解析
* Operator Precedence
* Error Recovery
* Parser Benchmark 方法

但必须保持：

```text
OXC Parser Result
  ↓
Nexa CST Adapter
  ↓
Nexa-owned CST
  ↓
Nexa HIR
```

禁止：

* HIR 直接保存 OXC AST 节点
* MIR 依赖 OXC 类型
* Nexa 类型系统复用 TypeScript 类型结构
* OXC 解析行为自动成为 Nexa 规范
* OXC 的 JavaScript 语义泄漏到 Nexa

自举编译器必须最终能够脱离 OXC。

因此 OXC 只能作为 Stage 0 工程加速工具，而不能成为永久语言语义依赖。

---

## 7. 执行与后端

### 7.1 编译管线

目标编译管线：

```text
Nexa Source
  ↓
Lossless CST
  ↓
Resolved HIR
  ↓
Typed HIR
  ↓
CFG MIR
  ↓
Backend-neutral NIR
  ↓
LLVM IR / Wasm / Other Backend
```

NIR 是否独立于 MIR，可以在实现阶段确定，但后端不得依赖 CST 和源级名称查找。

---

### 7.2 参考执行

现有 CFG MIR Interpreter 继续保留，用于：

* 语言规范验证
* Conformance
* Differential Testing
* Fuzzing
* 编译器自举验证
* 后端行为对比

参考解释器不作为最终高性能生产运行时。

---

### 7.3 第一生产后端

第一生产后端为：

```text
LLVM Native AOT
```

支持目标：

* Linux
* macOS
* Windows

后续支持：

* ARM
* Embedded Targets
* Freestanding Targets

---

### 7.4 LLVM 隔离

LLVM 必须封装在独立后端层：

```text
nexa_codegen_llvm
```

Compiler Core 只能依赖抽象接口：

```rust
trait CodegenBackend {
    fn compile(
        &self,
        module: &NirModule,
        options: &CodegenOptions,
    ) -> Result<Artifact, CodegenError>;
}
```

Lexer、Parser、HIR、类型系统和 MIR 不得依赖 LLVM API。

自举编译器初期输出后端无关 NIR，由 Rust LLVM Backend 生成机器码。

---

### 7.5 编译速度策略

LLVM 与快速编译目标存在张力，因此命令分层：

```text
nexa check
  → Parser + HIR + Type Check + MIR Validation
  → 不运行 LLVM

nexa run
  → 开发期可使用 MIR Interpreter 或低优化 LLVM

nexa build
  → LLVM 低至中等级优化

nexa build --release
  → LLVM 高优化 + LTO + Dead Stripping
```

后续可以增加 Cranelift 开发后端，但 Cranelift 不属于第一阶段承诺。

---

### 7.6 WebAssembly

第二生产目标为 WebAssembly。

第一阶段可以通过 LLVM Wasm Target 生成。

长期可以建立独立 Wasm Backend，以获得：

* 更快编译
* 更小包体
* 更稳定的 Host ABI
* 更好的 Component Model 支持

---

### 7.7 字节码

初期不定义独立 Nexa Bytecode。

原因：

* 已有 MIR Interpreter 可承担参考执行
* LLVM 是第一生产后端
* 过早增加 VM 会形成第二套运行时
* 增加 Verifier、版本和调试器工作量

MIR 是内部编译器表示，不是稳定公共字节码。

未来在以下需求明确后重新评估：

* REPL
* 快速脚本执行
* 插件系统
* 可分发沙箱程序
* 热重载
* 移动端小型 VM

---

### 7.8 JIT

第一阶段不实现 JIT。

后续可以针对服务端、交互式工具或长期运行程序评估：

* LLVM ORC
* Cranelift JIT
* Bytecode Tiering

JIT 不能成为语言语义的一部分。

---

## 8. 所有权模型

### 8.1 基本原则

每个非 `Copy` 值在任意时刻有一个逻辑所有者。

所有者离开作用域时执行确定性 Drop。

安全代码禁止：

* Use-after-free
* Double-free
* 悬空安全引用
* 无同步数据竞争
* 非法别名可变访问

---

### 8.2 Copy 类型

默认 Copy 类型：

```text
Bool
Unit
固定宽度整数
Float
小型无 Payload Enum
部分编译器确认可 Copy 的值类型
```

Copy 不调用用户可观察的 Clone。

---

### 8.3 Move 类型

默认 Move 类型：

```text
String
Array<T>
Vec<T>
Record
Class
Enum Payload
Closure
Resource
Shared<T>
用户定义非 Copy 类型
```

赋值、返回和所有权参数传递可以触发 Move。

Nexa 不进行隐式深度 Clone。

显式复制使用：

```nexa
const other = value.clone();
```

---

## 9. 参数所有权模式

函数参数有三种模式。

### 9.1 默认参数：只读 Borrow

```nexa
function inspect(document: Document): Unit {
  print(document.title);
}
```

调用：

```nexa
inspect(document);
inspect(document);
```

函数不能：

* 保存该 Borrow
* 返回该 Borrow
* 将其放进逃逸闭包
* 跨越 `await`
* 将其转移给其他所有者

---

### 9.2 `mut`：唯一可变 Borrow

```nexa
function normalize(mut document: Document): Unit {
  document.title = document.title.trim();
}
```

调用期间：

* 不允许其他 Borrow 同时访问
* 不允许另一个 `mut` Borrow
* 函数返回后所有权仍属于调用者

---

### 9.3 `take`：取得所有权

```nexa
function enqueue(take task: Task): Unit {
  queue.push(task);
}
```

调用采用隐式 `take` 语法糖：

```nexa
const task = createTask();

enqueue(task);
```

调用后：

```nexa
inspect(task); // 编译错误：task 已移动
```

调用处不要求写：

```nexa
enqueue(take task);
```

编译器根据已经完成解析的函数签名确定该参数会消费所有权。

---

### 9.4 隐式 Move 的限制

为了保持语义清晰：

1. 函数签名必须明确声明 `take`。
2. 调用处可以省略 `take`。
3. 编译器和 IDE 必须显示 Ownership Hint。
4. Diagnostic 必须说明值被哪个调用移动。
5. 不允许仅通过 `borrow`、`mut`、`take` 区别定义重载。
6. 动态分派接口必须在接口签名中明确所有权模式。
7. 编译器不能根据调用后的使用情况改变 Borrow 或 Move 决策。
8. 非 `Copy` 赋值默认 Move，不隐式 Clone。
9. 临时值可以直接传入 `take` 参数。
10. 同一表达式只能 Move 一次。

示例错误：

```text
E4101: value `task` was moved into `enqueue`
  moved here: enqueue(task)
  later used here: inspect(task)
```

---

## 10. 借用系统

### 10.1 第一阶段

第一阶段只实现：

* Copy
* Move
* Drop
* `take`
* 作用域级所有权检查

随后实现：

* 不逃逸只读 Borrow
* 不逃逸唯一 `mut` Borrow

第一阶段不公开显式生命周期语法。

---

### 10.2 不允许的 Borrow 逃逸

普通 Borrow 不允许：

* 从函数返回
* 存入普通 Record 或 Class
* 存入全局变量
* 被逃逸 Closure 捕获
* 跨越 `await`
* 跨线程传递
* 存入异步 Task
* 存入 `Shared<T>`

---

### 10.3 Slice 和 View

标准库可以提供受控的：

```text
Slice<T>
MutSlice<T>
StringView
ByteView
```

第一版要求其生命周期不超过当前调用或词法作用域。

后续根据真实性能需求，再决定是否支持受限逃逸 Borrow。

Nexa 2.0 不直接复制 Rust 完整生命周期参数系统。

---

## 11. Drop 与资源清理

Owned 值离开作用域时自动 Drop。

Drop 顺序：

1. 局部变量按确定的逆声明顺序
2. Record/Class 字段按规范规定的逆字段顺序
3. 已 Move 的值不再 Drop
4. Panic Abort 时不保证完成普通栈展开 Drop

Drop 不允许返回错误。

需要处理关闭错误时使用显式方法：

```nexa
const file = File.open(path)?;
defer file.closeIgnoringError();

const result = file.close();
```

或：

```nexa
function close(take self): Result<Unit, IoError>;
```

显式 `close` 消费资源所有权。

---

## 12. `defer`

Nexa 支持 `defer`。

```nexa
const lock = mutex.lock();
defer lock.release();
```

规则：

* 当前作用域退出时执行
* 按注册逆序执行
* 正常返回和 `Result` 提前传播时执行
* Panic Abort 时不承诺执行
* `defer` 不得捕获已经被 Move 的值
* `defer` 中的普通 Borrow 不能超过作用域

RAII 用于默认安全清理，`defer` 用于显式作用域操作。

---

## 13. Shared 与 Weak

### 13.1 显式共享

共享所有权必须显式使用：

```nexa
const config: Shared<Config> =
  Shared.new(loadConfig());
```

普通值不会因为复制或传参自动变为 Shared。

`Shared<T>` 预计使用引用计数实现，但具体实现不是语言 API。

---

### 13.2 Weak

提供：

```nexa
const weak: Weak<Config> = config.weak();
```

`Weak<T>` 不保持对象存活。

使用前必须升级：

```nexa
const config: Config? = weak.upgrade();
```

---

### 13.3 共享可变状态

`Shared<T>` 默认只提供只读访问。

共享修改必须显式使用：

```text
Shared<Cell<T>>
Shared<Mutex<T>>
Actor<T>
Atomic<T>
```

Shared Mutable 不属于首次自举前置能力。

编译器自举优先使用：

* 唯一拥有的 CompilerSession
* Arena
* Builder
* Interner
* ID 引用
* `mut` Borrow

---

## 14. Allocator 和 Arena

### 14.1 Application Profile

普通应用存在默认安全 Allocator。

```nexa
const values = Vec<Int>();
```

用户不需要在所有 API 上传递 Allocator。

关键底层 API可以提供显式版本：

```nexa
const values = Vec.withAllocator<Int>(allocator);
```

---

### 14.2 Freestanding Profile

Freestanding 环境没有默认堆。

程序必须显式提供：

* 内存区域
* Allocator
* OOM 策略
* Host 能力

---

### 14.3 Allocator 类型

标准能力包括：

```text
SystemAllocator
ArenaAllocator
FixedBufferAllocator
PoolAllocator
DebugAllocator
FailingAllocator
```

Allocator 实现可能包含 Unsafe，但安全调用方不能获得非法引用。

---

### 14.4 Arena

Arena 是一级标准能力，主要用于：

* 编译器
* Parser
* HIR/MIR
* 短生命周期 UI Tree
* Request Scope
* 批处理
* 临时反序列化

Arena 中的普通引用不得逃逸 Arena 生命周期。

安全 API 使用 Arena ID、Handle 或受限 Borrow，避免悬空引用。

---

## 15. OOM 策略

Application Profile：

```text
默认 OOM → 终止程序
```

原因是让普通集合和字符串 API 保持可用。

受控环境：

```text
tryAllocate
Allocator.allocate
```

可以返回：

```nexa
Result<T, AllocationError>
```

Freestanding Profile：

必须显式选择：

* Abort
* Result
* 固定缓冲区耗尽错误
* Host 回调

不同构建模式不能静默改变 OOM 语义。

---

## 16. Unsafe

Nexa 支持：

```text
unsafe block
unsafe function
unsafe module
```

Unsafe 用于：

* 裸指针
* FFI
* 自定义 Allocator
* 内存映射
* SIMD
* 原子操作底层
* 系统调用
* 内核接口
* Runtime 实现

普通代码默认 Safe。

包 Manifest 必须记录：

```toml
unsafe = true
```

工具链必须支持：

* 统计 Unsafe Block
* CI 禁止 Unsafe
* 依赖审计
* 显示 Unsafe 传递链

Unsafe 不会关闭整个语言的类型检查，只允许调用方承担明确列出的安全不变量。

---

## 17. 数据与值语义

### 17.1 语言语义

以下类型默认使用值语义：

* String
* Array
* Vec
* Record
* Tuple
* Enum
* Class
* Closure

值语义不表示每次赋值都深度复制。

非 Copy 值赋值默认 Move。

---

### 17.2 物理布局

编译器可以选择：

```text
小值 → 栈或寄存器内联
大值 → 堆上装箱
不可变值 → COW 或结构共享
Move → 移动 Handle
```

装箱阈值不是语言语义。

程序不能根据值大小观察：

* 是否堆分配
* 是否共享缓冲区
* 是否发生 COW
* 对象地址

---

## 18. Record、Class 与对象身份

### 18.1 普通 Record

匿名 Record 和 `type` Record 使用结构类型。

```nexa
type Point = {
  x: Int;
  y: Int;
};
```

结构兼容采用确定规则，不复制 TypeScript 全部宽度和深度子类型行为。

---

### 18.2 Class

Class 使用名义类型。

Class 默认没有用户可观察的共享对象身份。

Class 值仍然：

* 可 Move
* 可 Borrow
* 可 Drop
* 唯一所有权下可修改

比较默认基于显式实现的值比较，不提供隐式地址相等。

---

### 18.3 Resource

真正具有外部身份和线性生命周期的对象使用：

```nexa
resource File {
  function read(mut self, buffer: MutSlice<Byte>): Result<Int, IoError>;
  function close(take self): Result<Unit, IoError>;
}
```

Resource：

* 名义类型
* Move-only
* 不可隐式 Clone
* 具有外部身份
* 确定性关闭
* 可以通过显式 Handle 或 Shared 包装共享

适用：

* File
* Socket
* DOM Node
* Window
* GPU Buffer
* Database Connection
* Process
* Timer

---

## 19. Array 和 Vec

### 19.1 Array

```text
Array<T>
```

是不可变集合。

```nexa
const values: Array<Int> = [1, 2, 3];
const next = values.append(4);
```

`values` 不变。

Runtime 可以使用：

* 共享缓冲区
* COW
* Chunked Storage
* Persistent Vector

但共享不可观察。

---

### 19.2 Vec

```text
Vec<T>
```

是唯一所有权的可变动态数组。

```nexa
let values = Vec<Int>();

values.push(1);
values.push(2);
values.push(3);
```

`Vec<T>` 的修改要求：

* 当前调用路径拥有唯一访问权
* 不存在活跃 Borrow
* 不存在 Shared Alias

转换：

```nexa
const array: Array<Int> = values.freeze();
```

`freeze()` 消费 Vec。

```nexa
const values: Vec<Int> = array.toVec();
```

`toVec()` 创建可变所有权集合。

---

### 19.3 Slice

提供：

```text
Slice<T>
MutSlice<T>
```

用于零拷贝临时访问。

Slice 不拥有元素。

第一阶段 Slice 不能逃逸创建其 Borrow 的作用域。

---

## 20. 可变性

### 20.1 `const`

```nexa
const user = User(...);
```

`const`：

* 不可重新绑定
* 不允许通过该绑定执行可变操作
* 可以只读 Borrow
* 可以 Move，除非后续决定引入 `const` 所有权保护

---

### 20.2 `let`

```nexa
let user = User(...);
```

`let`：

* 可以重新绑定
* 可以在唯一所有权条件下执行可变操作
* 不意味着值可以在存在 Alias 时任意修改

---

### 20.3 深度可变性

Nexa 采用：

> 唯一所有权下可变，共享后默认只读。

普通共享可变状态必须使用显式同步或 Cell 类型。

---

## 21. 空值

语言核心不存在：

```text
null
undefined
```

使用：

```text
T? = Option<T>
```

示例：

```nexa
const user: User? = findUser(id);
```

不是：

```nexa
const user?: User = findUser(id);
```

因为 `?` 修饰类型，而不是变量名。

---

## 22. 类型推断

允许：

* 局部变量类型推断
* 局部泛型参数推断
* 闭包在明确 Expected Type 下进行有限推断
* 私有函数返回值的有限推断

要求：

* Public 函数参数显式
* Public 函数返回类型显式
* FFI 签名完全显式
* Resource API 完全显式
* Async Public API 完全显式

不采用 TypeScript 式全局复杂推断。

---

## 23. 泛型

语言层泛型语义不依赖具体后端。

第一 LLVM 实现可以优先使用单态化。

为控制包体积，长期提供：

```text
speed 模式 → 选择性单态化
size 模式 → 共享代码或 Dictionary Passing
```

允许显式：

```text
@specialize
@noSpecialize
```

但这些属于性能提示，不改变类型语义。

编译器必须限制无限泛型实例化。

---

## 24. 数字

默认类型：

```text
Int   = signed 64-bit
Float = IEEE-754 binary64
```

底层 Profile 提供：

```text
I8 I16 I32 I64
U8 U16 U32 U64
F32 F64
Usize Isize
```

普通整数运算始终检查溢出。

提供显式：

```text
checkedAdd
wrappingAdd
saturatingAdd
```

Debug 和 Release 的整数语义保持一致。

---

## 25. 错误模型

### 25.1 可恢复错误

使用：

```nexa
Result<T, E>
```

错误必须出现在类型系统中。

---

### 25.2 `?`

`?` 用于向当前函数调用者传播错误。

```nexa
function load(): Result<Data, LoadError> {
  const text = readFile(path)?;
  return parse(text);
}
```

---

### 25.3 `try/catch`

`try/catch` 用于当前作用域恢复或转换 `Result`。

```nexa
try {
  const data = load()?;
  render(data);
} catch {
  case LoadError.NotFound() => renderEmpty();
  case LoadError.Invalid(message) => renderError(message);
}
```

不进行 Runtime 栈展开。

---

### 25.4 Panic

Panic 表示：

* 内部不变量破坏
* 明确不可恢复错误
* 默认 OOM
* 安全检查失败

默认行为：

```text
panic → abort
```

不允许普通 `catch` 捕获 Panic。

后续可以为测试或 Host Runtime 提供隔离进程级 Panic 报告，但不改变语言默认语义。

---

## 26. 闭包

非逃逸闭包可以临时 Borrow。

逃逸闭包必须拥有其捕获值。

编译器可以自动推断 Closure 是否逃逸，但捕获结果必须确定。

逃逸示例：

```nexa
function createHandler(take state: State): () => Unit {
  return (): Unit => {
    render(state);
  };
}
```

这里 Closure 拥有 `state`。

共享捕获必须显式使用 `Shared<T>`。

普通 Borrow 不允许被逃逸 Closure 捕获。

---

## 27. Async 和并发

### 27.1 Async

Async 编译为状态机。

Task 在创建后拥有跨 `await` 使用的数据。

普通 Borrow 不能跨 `await`。

允许跨 `await` 的数据：

* Owned Value
* Copy Value
* Shared Value
* 静态数据
* Runtime 明确认可的 Handle

---

### 27.2 并发路线

按以下顺序实现：

```text
1. 单线程事件循环
2. Structured Concurrency
3. Channel
4. Worker / Actor
5. 共享内存线程
```

不在第一阶段直接实现任意共享内存线程。

---

### 27.3 跨线程安全

Owned 值可以通过 Move 发送到其他线程。

Shared 值必须满足线程安全约束。

未来提供类似：

```text
Send
Sync
```

的自动能力判断，但不要求使用 Rust Trait 语法暴露。

共享修改必须使用：

* Mutex
* RwLock
* Atomic
* Actor
* Channel

安全代码禁止数据竞争。

---

## 28. FFI 与 ABI

优先顺序：

```text
1. C ABI
2. Wasm Component / Host ABI
3. JavaScript Generated Binding
4. 其他语言绑定
```

不承诺 Rust ABI 稳定。

JavaScript FFI 优先采用声明生成：

```nexa
@jsImport("window.localStorage")
interface LocalStorage {
  function getItem(key: String): String?;
}
```

编译器或工具生成：

* 类型转换
* 生命周期桥接
* 错误处理
* Host Handle

任意动态 JavaScript 对象只允许存在于 Unsafe Adapter。

---

## 29. 模块和包

模块支持：

```text
相对路径模块
Manifest 包名模块
Workspace 模块
```

不复制 Node.js Module Resolution。

依赖来源顺序：

```text
1. Local Path
2. Git Commit
3. Registry
```

应用必须生成 Lockfile。

Lockfile记录：

* 版本
* Commit
* Hash
* Feature
* Target
* Unsafe 使用
* Capability

---

## 30. 安装脚本

默认禁止任意安装脚本。

优先使用声明式构建描述。

确需脚本时：

* 必须声明 Capability
* 必须在沙箱中运行
* 默认无网络
* 默认只写构建目录
* 输入和输出参与缓存 Hash
* 用户可以禁止所有构建脚本

---

## 31. 反射

默认不提供完整运行时反射。

默认 Release 不保留：

* 所有类型名
* 所有字段名
* 所有泛型信息
* 所有 Annotation
* 任意动态字段访问表

显式使用：

```nexa
@derive(Json, Schema)
type User = {
  name: String;
};
```

或：

```nexa
@reflect
class User {
  public name: String;
}
```

`@derive` 优先生成静态代码。

`@reflect` 才保留运行时元数据。

Debug Info 与运行时 Reflection 分离。

---

## 32. 宏和元编程

自举前不实现通用过程宏。

第一阶段提供：

* `@derive`
* 声明式生成
* 外部代码生成工具
* 编译器内建的有限 Attribute

后续宏系统必须满足：

* Hygiene
* 确定性
* 无默认网络
* 文件访问受 Capability 控制
* 生成结果可检查
* IDE 能看到展开结果
* 宏不能绕过 Unsafe 审计

---

## 33. 确定性

核心语言始终保证：

* 左到右求值
* 每个表达式恰好求值一次
* 稳定整数语义
* 稳定 Match 行为
* 稳定 Drop 顺序
* 稳定 Diagnostic 排序
* 稳定模块解析
* 稳定构建输入

Hash 集合默认不得将随机迭代顺序暴露为语言行为。

提供 Deterministic Profile：

* 无隐式系统时间
* 无隐式随机数
* 无隐式环境变量
* 无隐式文件系统
* 无隐式网络
* Host 能力显式注入
* 资源预算显式配置

Application Profile 可以使用非确定 Host 能力，但调用必须显式。

---

## 34. 沙箱和 Capability

Host 能力包括：

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
```

包和应用通过 Manifest 声明 Capability。

沙箱 Runtime 可以拒绝未授权调用。

Unsafe 不自动获得 Host Capability。

---

## 35. 工具链

正式可用版本必须提供：

```text
nexa check
nexa build
nexa run
nexa test
nexa fmt
nexa doc
nexa add
```

### Formatter

* 官方唯一 Formatter
* 少量配置
* TS-like 风格
* 输出确定
* AI 和用户使用相同格式

### LSP

属于正式可用版本的必要能力。

支持：

* 诊断
* 补全
* 跳转
* 引用
* Rename
* Ownership Hint
* Move Location
* Borrow Conflict
* Type Hover

### Test Runner

支持：

* Unit Test
* Integration Test
* Snapshot
* Property Test
* Benchmark
* Mock Host
* Capability Test
* Unsafe Audit Test

### Debugger

路线：

```text
MIR Trace
→ Native Debug Info
→ LLDB Integration
→ DAP
→ Wasm Debug
```

---

## 36. AI 友好

AI 友好不是单独语法功能，而是工程约束。

必须具备：

* 少歧义语法
* 官方 Formatter
* 明确 Ownership 模式
* 结构化 Diagnostic
* 稳定错误码
* 机器可读 Compiler Output
* 可执行 Conformance
* 确定性构建
* 自动 Fix
* API 文档 Schema
* 依赖和 Capability Manifest
* Unsafe 可审计
* Move 和 Borrow 错误包含具体来源

隐式 `take` 不得降低 AI 可理解性。

因此编译器输出必须明确：

```json
{
  "code": "E4101",
  "value": "task",
  "movedAt": {
    "callee": "enqueue",
    "parameter": "task"
  },
  "usedAt": {
    "callee": "inspect"
  }
}
```

---

## 37. 与自举路线的关系

ADR-000 继续定义自举阶段。

自举编译器可以采用 Nexa 2.0 的保守子集：

* Copy
* Move
* Drop
* `take`
* 非逃逸 Borrow
* `mut` Borrow
* Vec
* Arena
* StringBuilder
* Result
* Match
* 泛型
* Module

自举不依赖：

* Shared Mutable
* Async
* Threads
* UI
* Reflection
* Class Inheritance
* JIT

LLVM Backend 初期保留 Rust 实现。

自托管编译器输出后端无关 NIR，由 Rust LLVM Backend 生成目标代码。

---

## 38. 兼容性和版本策略

Language 1.0 保持冻结。

本 ADR 中可能破坏 1.0 的内容包括：

* Record 结构类型
* Class 和 Interface
* 所有权语义
* Array/Vec 区分
* `T?`
* `take`
* `mut` 参数
* Drop
* Async
* Try/Catch Result 语义

这些能力不得以普通 1.x Minor Release 静默引入。

采用：

```text
Nexa Language 2.0
或
新的 Language Edition
```

版本分别管理：

```text
Language Version
Compiler Version
Stdlib Version
Runtime Version
Artifact Version
Edition
```

---

## 39. 实施阶段

### Phase L0：规范与兼容边界

* 固定 Nexa 2.0 语法草案
* 定义与 Language 1.0 的迁移边界
* 定义 Ownership Diagnostic
* 定义 Copy/Move/Drop 不变量
* 更新 Parser 和 CST 规划
* 建立 ADR 和 Conformance 目录

### Phase L1：Move 和 Drop

* Copy 类型
* Move 类型
* Use-after-move 检查
* Scope Drop
* `take` 参数
* 调用处隐式 Move
* 显式 Clone
* Resource Move-only

### Phase L2：受限 Borrow

* 默认只读 Borrow
* `mut` 唯一 Borrow
* Borrow Conflict
* 不允许逃逸
* 不允许跨 await
* Slice/MutSlice

### Phase L3：Allocator 与集合

* Default Allocator
* Arena
* Vec
* Array Freeze
* StringBuilder
* FixedBufferAllocator
* OOM 策略

### Phase L4：类型与 TS-like Surface

* Structural Record
* Interface
* Class
* Enum Syntax
* Opaque Type
* `T?`
* public/private
* 无继承限制

### Phase L5：LLVM Native

* Backend-neutral NIR
* LLVM IR Lowering
* Object Generation
* Linker Driver
* Debug Info
* Native Runtime
* Linux/macOS/Windows

### Phase L6：错误和资源

* Result
* `?`
* Try/Catch Lowering
* Panic Abort
* Defer
* Resource
* C ABI

### Phase L7：Wasm 和 JS Host

* LLVM Wasm
* Wasm Host ABI
* JS Binding Generator
* DOM Handle
* Browser Package
* Capability Model

### Phase L8：Async 和并发

* Async State Machine
* Event Loop
* Structured Concurrency
* Channel
* Actor/Worker
* Shared/Weak
* Mutex/Atomic

### Phase L9：工具和生态

* Formatter
* LSP
* Test Runner
* Package Manager
* Lockfile
* Documentation
* Debugger

---

## 40. 验收标准

### 所有权

* 安全代码无法 Use-after-free
* 安全代码无法 Double-free
* 非 Copy 值不发生隐式 Clone
* `take` 调用处隐式 Move 行为确定
* Move Diagnostic 能定位消费调用
* Drop 顺序稳定
* Borrow 不能逃逸

### 内存

* 不依赖 tracing GC
* Application 有默认 Allocator
* Freestanding 可完全控制 Allocator
* Vec 可线性构建大型集合
* Array 可安全共享
* Resource 确定性释放
* OOM 行为明确

### TypeScript-like 体验

* 常用声明与控制流接近 TS
* Type、Interface、Class、Enum 语法熟悉
* 所有权错误可由 IDE 明确展示
* 普通只读函数调用无需用户写 Borrow 标记
* 消费函数调用无需显式写 `take`

### 后端

* LLVM Native 可编译真实跨平台应用
* Reference Interpreter 与 LLVM 行为一致
* Wasm 可运行同一核心程序
* 后端不参与源级名称和类型解析

### 工具链

* Formatter 输出唯一
* LSP 显示 Move/Borrow 信息
* Compiler 输出结构化错误
* 构建可复现
* Unsafe 和 Capability 可审计

---

## 41. 最终决策

Nexa 2.0 的目标不是复制 TypeScript、Rust 或 Zig。

最终组合为：

```text
TypeScript 的语法亲和力
+
Rust 的所有权与内存安全原则
+
比 Rust 更受限、更易理解的 Borrow
+
Zig 的 Allocator 和 Arena 思想
+
LLVM 的 Native AOT 能力
+
Wasm 的跨平台与沙箱能力
+
Elm 式可预测 UI 状态方向
```

所有权调用语义最终确定为：

```nexa
function inspect(value: Task): Unit;
function update(mut value: Task): Unit;
function enqueue(take value: Task): Unit;

inspect(task); // Borrow
update(task);  // Unique Mutable Borrow
enqueue(task); // Implicit Move
```

调用处不写 `take`，它是由函数签名驱动的隐式语法糖。

但 Move 绝不能依赖编译器猜测：

```text
函数签名决定 Ownership
调用解析结果决定 Move
IDE 显示 Ownership Hint
Diagnostic 显示 Move Source
语言不执行隐式 Clone
```

Nexa 的最终内存方向确定为：

```text
No Tracing GC
Single Ownership
Deterministic Drop
Restricted Borrowing
Explicit Shared/Weak
Allocator and Arena
Safe by Default
Unsafe at Auditable Boundaries
```
