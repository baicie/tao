# ADR-002：所有权、借用、Move 与确定性 Drop 语义

* **状态**：Accepted
* **日期**：2026-08-01
* **目标版本**：Nexa Language 2.0
* **关联 ADR**：ADR-001 Nexa 2.0 语言架构、所有权与运行时模型
* **替代范围**：不修改 Nexa Language 1.0 已冻结语义
* **实现优先级**：P0

---

## 1. 背景

Nexa 的目标是同时满足：

* TypeScript-like 的日常开发体验
* 默认内存安全
* 不使用 tracing GC
* 确定性资源释放
* 小运行时和小发布产物
* Native、Wasm 与 Freestanding 支持
* 比 Rust 更容易学习的所有权模型
* 可供 AI 和工具稳定分析的明确语义

单纯采用 Zig 式手动 `alloc/free` 无法保证安全代码不存在：

* Use-after-free
* Double-free
* 悬空引用
* 非法可变别名
* 数据竞争

完整复制 Rust 的生命周期与借用表达能力，又会显著提高前端开发者的学习成本。

因此 Nexa 采用：

> 单一所有权、隐式 Move、受限且不可逃逸的 Borrow、显式共享所有权，以及确定性 Drop。

Nexa 2.0 不暴露通用生命周期参数，不允许普通安全引用构成任意长期引用图。

---

## 2. 决策摘要

Nexa 的所有权模型遵循以下规则：

```text
1. 每个非 Copy 值在任意时刻只有一个逻辑所有者
2. 非 Copy 值的赋值、返回和消费参数传递默认执行 Move
3. Move 不执行深度复制
4. 普通函数参数默认是只读 Borrow
5. `mut` 参数是唯一可变 Borrow
6. `take` 参数取得所有权
7. 调用 `take` 参数时不要求在调用处写 `take`
8. 普通 Borrow 不得逃逸函数或词法作用域
9. Borrow 不得跨越 await
10. 所有者离开作用域时执行确定性 Drop
11. 可恢复失败使用 Result，不依赖异常栈展开
12. 共享所有权必须显式使用 Shared<T>
13. 共享可变状态必须使用 Cell、Mutex、Actor 等显式类型
14. 裸指针和手动内存操作只能存在于 unsafe
15. 安全代码保证内存安全，但不保证不存在逻辑内存泄漏
```

典型函数签名：

```nexa
function inspect(value: Document): Unit;
function normalize(mut value: Document): Unit;
function enqueue(take value: Document): Unit;
```

典型调用：

```nexa
let document = loadDocument();

inspect(document);   // 只读 Borrow
normalize(document); // 唯一可变 Borrow
enqueue(document);   // 隐式 Move

inspect(document);   // 编译错误：document 已移动
```

调用处的隐式 Move 是语法糖，但 Move 决策只能来自已经解析完成的函数签名，不能由编译器根据上下文猜测。

---

## 3. 术语

### 3.1 Owner

Owner 是负责一个值最终被 Drop 的变量、字段、集合元素、返回位置或 Runtime Root。

非 Copy 值同时只能有一个 Owner。

---

### 3.2 Place

Place 是能够保存值的位置，例如：

```text
局部变量
对象字段
Tuple 字段
数组或 Vec 元素
函数返回位置
静态变量
```

编译器以 Place 为单位执行初始化、Move、Borrow 和 Drop 分析。

---

### 3.3 Copy

Copy 表示可以按位或按语言规定复制，且复制不会产生：

* 双重释放
* 外部资源重复关闭
* 用户可观察的 Clone 行为
* 所有权冲突

---

### 3.4 Move

Move 将一个值的所有权从源 Place 转移到目标 Place。

Move 后源 Place 变为未初始化状态，直到被重新赋值。

Move 本身不调用用户代码，也不执行深度 Clone。

---

### 3.5 Borrow

Borrow 是对一个仍由其他 Place 拥有的值进行临时访问。

Nexa 有两种 Borrow：

```text
Shared Borrow：只读，可同时存在多个
Mutable Borrow：可变，同时最多存在一个
```

---

### 3.6 Drop

Drop 是值生命周期结束时执行的确定性清理过程，包括：

* 调用用户定义的析构逻辑
* 依次 Drop 字段或元素
* 释放堆内存
* 释放 Shared 强引用
* 关闭由 Drop 兜底管理的资源

---

## 4. 类型的所有权分类

### 4.1 Copy 类型

以下内建类型默认是 Copy：

```text
Bool
Unit
I8/I16/I32/I64
U8/U16/U32/U64
Isize/Usize
F32/F64
Char
无 Payload 的小型 Enum
函数定义引用 FunctionRef
```

用户类型只有显式声明或派生 Copy 后，才是 Copy：

```nexa
@derive(Copy)
type Point = {
  x: Int;
  y: Int;
};
```

编译器必须验证：

* 所有字段均为 Copy
* 类型没有用户自定义 Drop
* 类型不包含 Resource
* 类型不包含 Shared、Vec、String 等非 Copy 字段
* 复制不会产生外部副作用

不允许仅根据类型尺寸自动推断公开类型为 Copy。

类型今天是否 Copy，不能因优化器版本或装箱策略而改变。

---

### 4.2 Move 类型

除 Copy 类型外，普通类型默认为 Move 类型，包括：

```text
String
Array<T>
Vec<T>
Record
Class
带 Payload 的 Enum
Closure
Resource
Shared<T>
Weak<T>
Box<T>
用户定义非 Copy 类型
```

---

### 4.3 Clone

Clone 是显式的逻辑复制操作：

```nexa
const second = first.clone();
```

Clone：

* 可以分配内存
* 可以复制整个值
* 可以增加共享引用计数
* 可以失败时返回 Result
* 不属于赋值或传参的隐式行为

Nexa 禁止非 Copy 值的隐式 Clone。

对于 `Shared<T>`：

```nexa
const secondOwner = firstOwner.clone();
```

会创建另一个强引用。

普通赋值：

```nexa
const secondOwner = firstOwner;
```

仍然是 Move。

---

## 5. 绑定与可变性

### 5.1 `const`

```nexa
const value = createValue();
```

`const` 表示：

* 绑定不可重新赋值
* 不能通过该绑定获得 Mutable Borrow
* 可以获得 Shared Borrow
* 可以被 Move
* Move 后绑定变为不可用

示例：

```nexa
const task = createTask();
enqueue(task); // 允许，task 被 Move
```

`const` 不表示值永远不可被转移。

---

### 5.2 `let`

```nexa
let value = createValue();
```

`let` 表示：

* 绑定可以重新赋值
* 在满足唯一访问条件时可以获得 Mutable Borrow
* 可以被 Move
* Move 后可以重新初始化

```nexa
let task = createTask();

enqueue(task);
task = createTask(); // 重新初始化

inspect(task);       // 有效
```

---

### 5.3 重新赋值

对一个已经初始化的 Place 重新赋值前，必须先 Drop 原值：

```nexa
let value = createFirst();
value = createSecond();
```

语义顺序为：

```text
1. 先完整求值 createSecond()
2. 若求值失败，不修改 value
3. Drop value 中原来的值
4. 将新值 Move 到 value
```

该顺序保证右侧表达式可以安全读取旧值：

```nexa
value = transform(value);
```

如果 `transform` 默认 Borrow，则合法；如果其参数是 `take`，则属于消费并重新初始化。

---

## 6. Move 语义

### 6.1 赋值 Move

```nexa
const first = createDocument();
const second = first;
```

若 `Document` 非 Copy，则：

```text
first  → 未初始化
second → 新所有者
```

之后使用 `first` 是编译错误。

---

### 6.2 返回 Move

```nexa
function create(): Document {
  const document = Document(...);
  return document;
}
```

返回值获得 `document` 的所有权，函数退出时不再 Drop `document`。

---

### 6.3 字段初始化 Move

```nexa
const token = createToken();

const request = Request({
  token
});
```

若 `Token` 非 Copy，所有权移动到 `request.token`。

---

### 6.4 集合插入 Move

```nexa
let tasks = Vec<Task>();
const task = createTask();

tasks.push(task);
```

若 `push` 的签名为：

```nexa
function push(mut self, take value: T): Unit;
```

则 `task` 在调用后失效。

---

### 6.5 Move 不可回滚

一旦某个 `take` 调用真正开始执行，参数所有权已经转移。

如果被调用函数返回 `Result.Err`，所有权不会自动退回调用者：

```nexa
const result = queue.tryPush(task);
```

若 `tryPush` 需要在失败时返还值，应在类型中明确表达：

```nexa
function tryPush(
  mut self,
  take value: T
): Result<Unit, PushError<T>>;
```

失败值可携带原值：

```nexa
match (queue.tryPush(task)) {
  case Ok() => {}
  case Err(PushError.Full(value)) => {
    // value 重新成为当前作用域的 Owner
  }
}
```

---

## 7. 函数参数模式

### 7.1 默认参数：Shared Borrow

```nexa
function inspect(value: Document): Unit {
  print(value.title);
}
```

这里的 `value` 是只读 Borrow，而不是 Owner。

函数不能：

* Move `value`
* 返回指向 `value` 的普通 Borrow
* 将其保存到长期对象
* 将其放入 Shared
* 将其捕获到逃逸闭包
* 跨越 `await` 使用
* 调用需要 `take value` 的函数

允许：

* 读取字段
* 调用只读方法
* 创建更短生命周期的 Shared Borrow
* 对 Copy 字段执行 Copy

---

### 7.2 `mut` 参数：Mutable Borrow

```nexa
function normalize(mut value: Document): Unit {
  value.title = value.title.trim();
}
```

调用期间必须满足：

* 调用者拥有该值
* 当前不存在其他 Mutable Borrow
* 当前不存在仍有效的 Shared Borrow
* 当前值未被 Move
* 绑定允许可变访问

函数不能：

* Move 整个 `value`
* 将 Borrow 保存到长期对象
* 返回普通 Borrow
* 跨 `await`
* 将 Borrow 发送到其他线程

---

### 7.3 `take` 参数：所有权转移

```nexa
function enqueue(take task: Task): Unit {
  queue.push(task);
}
```

调用：

```nexa
enqueue(task);
```

等价于内部的所有权转移，但调用处不出现额外关键字。

`take` 必须出现在函数签名、Interface 签名和函数类型中。

---

### 7.4 所有权模式属于函数类型

以下函数类型不同：

```nexa
type Reader = (value: Data) => Unit;
type Mutator = (mut value: Data) => Unit;
type Consumer = (take value: Data) => Unit;
```

它们不能互相隐式转换。

特别是不允许：

* Consumer 伪装成 Reader
* Mutator 伪装成 Reader
* 仅通过所有权模式实现函数重载

---

### 7.5 方法中的 `self`

方法遵循相同规则：

```nexa
function length(self): Int;        // Shared Borrow
function clear(mut self): Unit;    // Mutable Borrow
function freeze(take self): Array; // Move
```

调用：

```nexa
builder.clear();
const result = builder.freeze();
```

`freeze()` 后 `builder` 失效。

---

## 8. 隐式 `take` 的约束

调用处省略 `take` 只是表面语法简化，编译器必须遵守：

1. 必须先完成名称解析和重载解析。
2. 必须从唯一确定的参数签名中获知 `take`。
3. 不能根据变量之后是否继续使用来猜测 Move。
4. 不能因优化级别改变 Move 结果。
5. 动态分派接口必须在接口中保存所有权模式。
6. FFI 描述必须明确参数是 Borrow 还是 Take。
7. IDE 应显示参数所有权提示。
8. 编译诊断必须指出 Move 的准确调用位置。
9. 非 Copy 参数不会因调用失败而隐式 Clone。
10. 表达式按从左到右顺序 Move。

示例：

```nexa
consume(first, second);
```

若两个参数均为 `take`，则先 Move `first`，再 Move `second`。

---

## 9. Borrow 规则

### 9.1 核心别名规则

任一时刻，对同一个 Place：

```text
允许：
多个 Shared Borrow

或者：
一个 Mutable Borrow

禁止：
Shared Borrow 与 Mutable Borrow 同时有效
多个 Mutable Borrow 同时有效
Borrow 有效时 Move 或 Drop Owner
```

---

### 9.2 Borrow 结束时间

Nexa 使用函数内部的最后使用分析，而不是简单要求 Borrow 持续到整个代码块末尾。

```nexa
const title = document.title();
print(title);

// 对 document 的 Borrow 在最后一次使用后结束
enqueue(document);
```

只要数据流能够确定 Borrow 已不再使用，后续 Move 可以合法。

Nexa 不向用户暴露显式生命周期参数。

---

### 9.3 字段级 Borrow

静态字段可以作为独立 Borrow Path：

```nexa
let user = User(...);

const name = user.name;
update(mut user.settings);
print(name);
```

只有在字段不重叠、类型没有特殊 Drop 限制时，编译器才允许字段级并行 Borrow。

---

### 9.4 集合索引 Borrow

动态索引默认视为对整个集合的 Borrow：

```nexa
let values = Vec<Int>();

const first = values[index];
update(mut values[otherIndex]); // 默认冲突
```

即使两个索引运行时可能不同，编译器也不进行一般整数不等式证明。

需要不重叠 Mutable Slice 时使用安全 API：

```nexa
const [left, right] = values.splitAtMut(index);
```

---

### 9.5 Borrow Owner 时的限制

当值存在 Shared Borrow 时，Owner 不能：

* Move
* Drop
* 重新赋值
* Mutable Borrow
* 修改可能影响被借用部分的结构

当值存在 Mutable Borrow 时，Owner 不能进行任何其他访问，除非访问编译器可证明不重叠的字段。

---

## 10. Borrow 逃逸

Nexa 2.0 的普通 Borrow 不允许逃逸。

禁止：

```nexa
function invalid(value: User): User {
  return value; // 若返回的是 Borrow，则非法
}
```

禁止将 Borrow：

* 返回给调用者
* 存入普通 Record
* 存入 Class 字段
* 存入 Array 或 Vec
* 存入静态变量
* 存入 `Shared<T>`
* 捕获到逃逸闭包
* 发送到其他线程
* 保存到异步 Task
* 跨越 `await`

需要长期保存时必须：

* Move 原值
* Clone 原值
* 使用 `Shared<T>`
* 使用拥有数据的 View 类型
* 在 Unsafe 边界构建明确的底层抽象

---

## 11. Slice 与 View

标准库提供：

```text
Slice<T>
MutSlice<T>
StringView
ByteView
```

这些类型是受限 Borrow View。

第一阶段规则：

* 只能存在于当前函数或词法作用域
* 不能存入普通堆对象
* 不能跨 `await`
* 不能跨线程
* Owner Drop 或 Move 前必须结束 View
* `MutSlice<T>` 保持唯一可变访问

未来若确有零拷贝 API 需求，可以在新 ADR 中增加受限生命周期关系，但不得静默扩展 2.0 语义。

---

## 12. 控制流中的所有权

### 12.1 条件分支

若值只在某一条可能路径被 Move，则在分支汇合后默认不可用：

```nexa
const task = createTask();

if (condition) {
  enqueue(task);
}

inspect(task); // 错误：task 可能已被移动
```

如果每条路径都重新初始化，则可继续使用：

```nexa
let task = createTask();

if (condition) {
  enqueue(task);
  task = createTask();
}

inspect(task);
```

编译器执行确定初始化分析。

---

### 12.2 所有路径均 Move

```nexa
const value = createValue();

if (condition) {
  consumeA(value);
} else {
  consumeB(value);
}
```

合法，因为每次执行只会选择一条路径，且汇合后不再使用 `value`。

---

### 12.3 循环

循环中 Move 一个循环外变量时，必须在所有回边和可能继续执行的路径上重新初始化：

```nexa
let task = createTask();

while (condition) {
  enqueue(task);
  task = createTask();
}
```

如果存在 `continue`、`break` 或条件路径，编译器必须验证每个路径上的初始化状态。

---

### 12.4 Match

对 Borrow 值进行 Match 时，Payload 默认也是 Borrow：

```nexa
function show(result: Result<User, Error>): Unit {
  match (result) {
    case Ok(user) => print(user.name);
    case Err(error) => print(error.message);
  }
}
```

对 Owned 值进行消费式操作时，具体函数或构造位置触发 Move。

初始版本不允许从一个仍需整体 Drop 的值中随意 Partial Move。

---

## 13. Partial Move

为降低实现复杂度，Nexa 2.0 初始版本采用保守规则：

* 不允许从实现自定义 Drop 的值中 Move 单个字段
* 不允许从普通 Class 中 Move 单个字段后继续使用剩余实例
* 允许通过消费整个值的解构获得字段
* Copy 字段可以单独 Copy
* Borrow 字段不影响其余可证明不重叠字段

消费式解构：

```nexa
const user = createUser();

const User({
  name,
  token
}) = user;
```

解构后 `user` 整体失效，`name` 和 `token` 分别成为新的 Owner。

---

## 14. 临时值

临时值通常在完整语句结束时 Drop：

```nexa
print(createMessage());
```

顺序：

```text
1. 创建 Message 临时值
2. Borrow 给 print
3. print 返回
4. Drop 临时 Message
```

如果临时值被 Move，则由新 Owner 负责 Drop：

```nexa
queue.push(createTask());
```

`createTask()` 的结果直接 Move 进入 Queue。

同一语句中的临时值按照创建顺序求值，按逆创建顺序 Drop。

---

## 15. Drop 语义

### 15.1 自动 Drop

每个已初始化且未 Move 的 Owned 值，在生命周期结束时自动 Drop。

生命周期结束包括：

* 到达作用域末尾
* `return`
* `break`
* `continue`
* `Result` 的 `?` 提前返回
* 普通控制流离开代码块
* 显式重新赋值
* 调用 `drop(value)`

---

### 15.2 用户自定义 Drop

用户可以为名义类型声明特殊 Drop 方法：

```nexa
class Buffer {
  private handle: NativeHandle;

  function drop(mut self): Unit {
    self.handle.releaseIgnoringError();
  }
}
```

限制：

* `drop` 是保留方法
* 返回类型必须是 `Unit`
* 不能是 `async`
* 不能被普通代码直接调用
* 不能 Move `self`
* 不能 Move 非 Copy 字段
* 不能令 Borrow 逃逸
* 不允许递归显式 Drop 自身
* 执行完成后编译器继续 Drop 字段

需要提前结束生命周期时使用内建：

```nexa
drop(buffer);
```

`drop(value)` 消费所有权。

---

### 15.3 Drop 顺序

Drop 顺序是语言语义的一部分。

#### 局部变量

按完成初始化的逆序 Drop：

```nexa
const first = createFirst();
const second = createSecond();
```

离开作用域时：

```text
Drop second
Drop first
```

#### Record 与 Class

顺序：

```text
1. 执行用户自定义 drop(mut self)
2. 按字段声明的逆序 Drop 字段
3. 释放对象自身存储
```

#### Tuple

按元素逆序 Drop。

#### Array 与 Vec

按元素索引逆序 Drop，再释放缓冲区。

#### Enum

只 Drop 当前 Variant 的 Payload，按 Payload 字段逆序执行。

#### Shared

减少强引用计数；最后一个强引用消失时 Drop 内部值。

---

### 15.4 已 Move 值不 Drop

```nexa
const value = createValue();
consume(value);
```

当前作用域退出时不会再次 Drop `value`。

---

## 16. 显式资源关闭

Drop 不允许返回错误。

对于文件、Socket、数据库连接等资源，提供显式消费方法：

```nexa
resource File {
  function close(take self): Result<Unit, IoError>;
}
```

使用：

```nexa
const file = File.open(path)?;
file.close()?;
```

`close()` 调用后 `file` 已 Move，无论返回成功还是失败，都不能再次使用。

若失败时需要返还资源，返回类型必须显式携带：

```nexa
Result<Unit, CloseError<File>>
```

Drop 仅负责未显式关闭时的兜底释放，不能作为处理关闭错误的主要方式。

---

## 17. `defer`

Nexa 支持：

```nexa
defer cleanup();
```

语义上等价于在当前作用域的所有正常退出路径插入清理代码。

执行顺序为注册逆序。

```nexa
defer first();
defer second();
```

离开作用域：

```text
second()
first()
```

`defer` 在以下情况执行：

* 正常到达作用域末尾
* `return`
* `break`
* `continue`
* `?` 提前返回

默认 `panic → abort` 时不保证执行 `defer`。

`defer` 中引用的变量被视为在作用域退出点仍有一次使用：

```nexa
const file = open();
defer file.closeIgnoringError();

consume(file); // 错误：defer 仍需要 file
```

如果 Defer 调用消费值，该值在作用域结束前不能被其他操作 Move。

---

## 18. Panic 与异常

Nexa 默认：

```text
panic → abort
```

因此 Panic 不进行栈展开，不保证执行：

* 局部变量 Drop
* `defer`
* 用户自定义析构
* 资源关闭

需要可靠清理的失败必须使用：

```text
Result
结构化并发取消
显式 close
进程或任务隔离
```

普通 `try/catch` 只处理 `Result`，不能捕获 Panic。

---

## 19. 闭包捕获

### 19.1 非逃逸闭包

非逃逸闭包可以临时 Borrow：

```nexa
items.forEach((item) => {
  print(config.format(item));
});
```

只要闭包不会被保存或返回，`config` 可以被 Borrow。

---

### 19.2 逃逸闭包

逃逸闭包必须拥有其长期捕获值。

```nexa
function createHandler(take state: State): () => Unit {
  return (): Unit => {
    render(state);
  };
}
```

这里 `state` Move 进入 Closure Environment。

普通 Borrow 不能进入逃逸闭包。

---

### 19.3 自动捕获规则

编译器可以推断闭包是否逃逸，但捕获结果必须确定：

```text
Copy 值      → Copy
只在非逃逸闭包读取 → Shared Borrow
非逃逸闭包修改     → Mutable Borrow
逃逸闭包长期使用   → Move
共享捕获           → 显式 Shared<T>
```

如果自动 Move 会导致外部变量失效，IDE 和诊断必须展示捕获位置。

---

## 20. Async

普通 Borrow 不允许跨越 `await`：

```nexa
async function invalid(value: Data): Result<Unit, Error> {
  await delay();
  print(value); // 非法：Borrow 跨 await
}
```

解决方式：

```nexa
async function valid(take value: Data): Result<Unit, Error> {
  await delay();
  print(value);
}
```

或者：

```nexa
async function valid(value: Shared<Data>): Result<Unit, Error> {
  await delay();
  print(value);
}
```

Async 状态机必须拥有跨暂停点保存的值。

允许跨 `await`：

* Copy 值
* Owned 值
* Shared 值
* `'static` Runtime Handle
* 编译器明确认可的异步资源

---

## 21. Shared 与 Weak

### 21.1 Shared

共享所有权必须显式创建：

```nexa
const first = Shared.new(config);
const second = first.clone();
```

`Shared<T>`：

* 本身是 Move 类型
* `.clone()` 创建新的强引用
* 最后一个强引用消失时 Drop `T`
* 默认只提供 Shared Borrow
* 不提供普通可变访问

---

### 21.2 Weak

```nexa
const weak = shared.weak();
```

Weak 不保持内部值存活。

升级返回：

```nexa
const current: Shared<Config>? = weak.upgrade();
```

---

### 21.3 强引用环

`Shared<T>` 可能形成强引用环。

强引用环不会造成内存不安全，但可能造成内存泄漏。

因此：

* 父子反向关系优先使用 Weak
* UI 订阅优先使用 Token 或 Handle
* Actor 不应通过强引用互相持有
* Debug Runtime 可以提供 Shared Cycle 检测
* 编译器不承诺静态消除所有引用环

---

## 22. 共享可变状态

`Shared<T>` 默认只读。

需要共享修改时必须显式使用：

```text
Shared<Cell<T>>
Shared<Mutex<T>>
Shared<RwLock<T>>
Actor<T>
Atomic<T>
```

单线程 `Cell<T>` 可以使用运行时 Borrow 检查或受控替换语义。

多线程共享必须使用线程安全容器。

普通 Record、Class、Array 不会因放入 Shared 而自动获得可变能力。

---

## 23. 线程安全

未来并发实现使用编译器能力判断：

```text
Send：值可以通过 Move 转移到其他线程
Sync：值可以安全地被多个线程 Shared Borrow
```

规则方向：

* 大多数纯 Owned 值自动满足 Send
* 包含非线程安全 Handle 的类型不满足 Send
* `Shared<T>` 跨线程要求 `T: Sync`
* `Shared<Cell<T>>` 默认不满足 Sync
* `Shared<Mutex<T>>` 可在约束满足时满足 Sync
* Borrow 不能跨线程发送
* Mutable Borrow 不能跨线程共享

具体能力语法另行 ADR 定义。

---

## 24. Unsafe 边界

Unsafe 可以执行：

* 裸指针读取和写入
* 手动分配与释放
* FFI
* 内存映射
* 自定义 Allocator
* 不受安全 Borrow 规则保护的底层操作

但 Unsafe 不关闭以下检查：

* 基础类型检查
* 名称解析
* 初始化检查
* 控制流检查
* 普通安全值的 Move 检查

Safe API 包装 Unsafe 时必须保证：

* 不返回悬空安全引用
* 不产生重复 Owner
* 不允许同一内存存在非法可变别名
* Drop 只执行一次
* 长度、对齐和边界合法
* 线程安全声明真实成立

---

## 25. FFI 所有权

FFI 声明必须明确参数模式：

```nexa
extern "C" unsafe function nativeRead(
  handle: NativeHandle,
  mut buffer: MutSlice<Byte>
): Result<Int, NativeError>;
```

如果外部函数取得所有权：

```nexa
extern "C" unsafe function nativeDestroy(
  take handle: NativeHandle
): Unit;
```

禁止默认假设外部调用：

* 不保存指针
* 不释放内存
* 不跨线程
* 不在返回后回调

这些行为必须由 FFI Schema 或 Unsafe Wrapper 明确声明。

---

## 26. MIR 表示要求

Typed HIR 必须保存参数所有权模式。

MIR 建议至少支持：

```text
StorageLive(place)
StorageDead(place)

Operand.Copy(place)
Operand.Move(place)
Operand.BorrowShared(place)
Operand.BorrowMut(place)

Assign(target, operand)
Drop(place)
Call(target, arguments)
Return
```

调用参数降低示例：

```nexa
consume(first, second);
```

若签名为：

```nexa
function consume(take first: A, second: B): Unit;
```

降低为：

```text
arg0 = Move(first)
arg1 = BorrowShared(second)
Call consume(arg0, arg1)
```

MIR Verifier 必须验证：

* 未初始化 Place 不可读取
* 已 Move Place 不可读取或 Drop
* Borrow 有效时不能非法 Move
* Mutable Borrow 唯一
* 所有控制流出口状态一致
* 每个已初始化 Owned Place 最终恰好 Drop 一次或 Move 一次

后端不得重新推断所有权。

---

## 27. 诊断要求

所有权错误必须具有稳定错误码、主标签和相关位置。

建议错误码：

| 错误码     | 含义                                |
| ------- | --------------------------------- |
| `E4101` | 使用已 Move 的值                       |
| `E4102` | 可能在某条控制流路径上已 Move                 |
| `E4103` | Move 一个正在被 Borrow 的值              |
| `E4104` | 从 Borrow 参数中 Move 值               |
| `E4105` | 非 Copy 值发生隐式复制请求                  |
| `E4110` | Mutable Borrow 与 Shared Borrow 冲突 |
| `E4111` | 存在多个 Mutable Borrow               |
| `E4112` | Borrow 逃逸函数                       |
| `E4113` | Borrow 被逃逸闭包捕获                    |
| `E4114` | Borrow 跨越 await                   |
| `E4115` | Borrow 跨线程传递                      |
| `E4120` | 对 const 绑定请求 Mutable Borrow       |
| `E4121` | 从禁止 Partial Move 的值中移动字段          |
| `E4130` | Drop 方法签名不合法                      |
| `E4131` | 直接调用保留 Drop 方法                    |
| `E4132` | Drop 中 Move 受保护字段                 |
| `E4140` | defer 使用已经 Move 的值                |
| `E4150` | 资源被重复关闭或消费                        |
| `E4160` | FFI 所有权模式缺失                       |

示例：

```text
E4101: value `task` was used after being moved

  12 | enqueue(task);
     |         ---- value moved into parameter `task`
  13 |
  14 | inspect(task);
     |         ^^^^ value used here after move

note: `enqueue` declares `take task: Task`
help: clone the value before the call if duplication is intended
```

结构化诊断必须提供：

```json
{
  "code": "E4101",
  "value": "task",
  "move": {
    "kind": "call-argument",
    "callee": "enqueue",
    "parameter": "task"
  },
  "laterUse": {
    "kind": "call-argument",
    "callee": "inspect"
  }
}
```

---

## 28. AI 与 IDE 要求

隐式 Move 不得成为隐藏语义。

IDE 应支持显示：

```text
inspect(document)    parameter: borrow
normalize(document)  parameter: mut
enqueue(document)    parameter: take
```

建议显示 Inlay Hint：

```nexa
enqueue(/* take */ task);
```

但不修改源码。

AI 工具应能通过 Compiler API 查询：

* 参数所有权模式
* 值当前初始化状态
* Move 来源
* Borrow 生效范围
* Drop 位置
* 自动修复候选

自动修复可以建议：

```text
插入 clone()
调整函数参数为 Borrow
将值重新初始化
缩短 Borrow 范围
改用 Shared<T>
```

不得自动插入不必要 Clone 而不提示成本。

---

## 29. 分阶段实现

### Phase O0：所有权 IR 基础

* Place
* 初始化状态
* Copy/Move Operand
* Scope Storage
* Drop Terminator
* 控制流数据流分析

### Phase O1：Copy、Move 与 Drop

* Copy 类型验证
* 非 Copy 赋值 Move
* Return Move
* `take` 参数
* 调用处隐式 Move
* Use-after-move 诊断
* 确定性 Drop
* 显式 `drop(value)`

### Phase O2：Shared Borrow

* 默认参数 Borrow
* 最后使用分析
* Move/Borrow 冲突
* 字段级 Borrow Path
* Slice

### Phase O3：Mutable Borrow

* `mut` 参数
* 唯一 Mutable Borrow
* 字段不重叠分析
* MutSlice
* Vec 原地修改

### Phase O4：控制流完整性

* 分支汇合
* 循环回边
* Match
* 提前返回
* `?`
* `defer`

### Phase O5：Closure 与 Async

* 非逃逸闭包 Borrow
* 逃逸闭包 Move Capture
* Borrow 跨 await 拒绝
* Async 状态机 Owned Capture

### Phase O6：Shared、Weak 与并发能力

* Shared
* Weak
* Cell
* Mutex
* Send/Sync 推导
* 跨线程 Move

### Phase O7：Unsafe 与 FFI

* Raw Pointer
* Allocator Contracts
* FFI Ownership Schema
* Unsafe Audit
* Native Resource Wrapper

---

## 30. 验收标准

### 30.1 安全性

Safe Nexa 程序不能产生：

* Use-after-free
* Double-free
* 对同一内存的非法可变别名
* Borrow 生命周期超过 Owner
* Borrow 跨越不安全异步边界
* Resource 重复关闭
* 数据竞争

---

### 30.2 确定性

以下行为必须稳定：

* 参数求值顺序
* Move 顺序
* Drop 顺序
* `defer` 顺序
* 控制流汇合规则
* 错误码和主诊断位置
* Debug 与 Release 的所有权语义

---

### 30.3 易用性

普通只读调用不需要用户书写 Borrow 标记：

```nexa
inspect(value);
```

消费调用不需要调用方书写 `take`：

```nexa
consume(value);
```

可变调用不需要调用方书写额外语法：

```nexa
update(value);
```

所有权模式由函数签名、IDE Hint 和诊断明确展示。

---

### 30.4 性能

* Move 不执行隐式深度复制
* Borrow 不增加普通引用计数
* Copy 仅适用于可安全廉价复制的类型
* Vec 支持原地修改
* Shared 只在显式使用时产生引用计数成本
* Drop 不依赖 tracing GC
* 后端可以内联、消除无效 Drop 和优化 Move

---

## 31. 被拒绝的方案

### 31.1 Zig 式默认手动内存

拒绝原因：

* 无法满足默认内存安全
* 前端开发者容易出现生命周期错误
* AI 生成代码难以可靠证明释放正确
* FFI 与应用代码的 Unsafe 边界不清晰

Allocator 思想保留，但手动释放限制在 Unsafe 和底层库。

---

### 31.2 完整 Rust 生命周期系统

暂不采用：

* 显式生命周期参数
* 任意 Borrow 字段
* 自引用安全结构
* Borrow 跨异步状态机
* Pin 等复杂能力

原因是会显著提高语言与编译器复杂度。

---

### 31.3 默认 ARC

拒绝原因：

* 每次值复制都可能产生引用计数操作
* 成本对用户不可见
* 强引用环
* 不利于系统和 Freestanding
* 削弱单一所有权模型

Shared/Weak 作为显式能力保留。

---

### 31.4 调用处强制书写 `take`

未采用：

```nexa
consume(take value);
```

最终采用：

```nexa
consume(value);
```

原因：

* 更接近 TypeScript 的调用体验
* 函数签名已经明确消费语义
* IDE 可以显示 Inlay Hint
* 结构化诊断可以准确解释 Move

但不允许编译器隐式推断函数是否消费参数。

---

## 32. 最终决定

Nexa 所有权模型最终冻结为：

```text
Copy 类型：
  赋值与传参复制

普通非 Copy 类型：
  赋值、返回和消费位置 Move

普通函数参数：
  默认 Shared Borrow

mut 参数：
  唯一 Mutable Borrow

take 参数：
  取得所有权，调用处隐式 Move

长期共享：
  Shared<T>/Weak<T>

资源：
  Move-only Resource + 显式 close + Drop 兜底

清理：
  确定性 Drop + defer

借用：
  不逃逸、不跨 await、不跨线程

底层操作：
  unsafe
```

核心调用体验：

```nexa
function inspect(value: Task): Unit;
function update(mut value: Task): Unit;
function enqueue(take value: Task): Unit;

let task = createTask();

inspect(task);
update(task);
enqueue(task);

// E4101：task 已移动
inspect(task);
```

该模型以函数签名驱动所有权，以调用处隐式 Move 保持 TS-like 体验，以受限 Borrow 降低生命周期复杂度，以确定性 Drop 和显式 Shared 消除对 tracing GC 的依赖。
