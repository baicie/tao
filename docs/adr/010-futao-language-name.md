# ADR-010：采用 Futao 作为语言正式名称

* **状态**：Accepted
* **日期**：2026-08-01
* **目标版本**：Futao Language 2.0 与后续工具链
* **关联 ADR**：ADR-001 至 ADR-009
* **兼容性范围**：不修改 Language 1.0 语义；已发布的 `nexac 0.0.1` 标识在迁移完成前保持可追溯

---

## 1. 背景

项目需要一个能够长期承载纪念意义、同时适合作为语言和工具链标识的正式名称。隐晦缩写会随着
语言定位变化而失去解释，单独使用 Tao 又会弱化对付涛本人的指向，并与既有技术名称混杂。

名称迁移还必须区分品牌决策和已经交付的产物。当前 Rust workspace、Cargo package、CLI 和
Language 1.0 文档仍使用 Nexa / `nexac` / `.nexa`。直接在设计文档中假定迁移已经完成，会让安装、
构建和兼容性说明失真。

---

## 2. 决策

语言正式名称采用 **Futao Programming Language**，简称 **Futao**。名称直接来自 Fu Tao（付涛），
不再为旧名称或字母构造新的展开含义。

规范标识统一为：

| 用途 | 标识 |
|------|------|
| 正式品牌 | `Futao` |
| 命令、包和仓库名 | `futao` |
| 源文件后缀 | `.ft` |
| 编译器命令 | `futao` |
| Language Server | `futao-ls` |
| 格式化命令 | `futao fmt` |
| 包管理命令 | `futao pkg` |
| Package manifest | `futao.toml` |
| Lockfile | `futao.lock` |
| Source package | `.ftpkg` |
| Stable component | `.ftc` |

正式文本不使用 `FuTao`。新建示例、规范片段和 2.0 package fixture 使用 Futao / `.ft` 命名。

---

## 3. 兼容与迁移边界

本 ADR 接受名称，不宣称工具链迁移已经交付。

* 历史 ADR、Language 1.0 规范、release tag 和构建证明中的 Nexa / `nexac` 保留原文，避免改写历史。
* 当前 `nexac 0.0.1`、Cargo crate 名和 `.nexa` conformance corpus 在独立迁移 PR 完成前继续工作。
* 新的 2.0 合同示例使用 `.ft`；其 README 必须明确当前可执行能力和未来命令之间的差异。
* CLI、crate、CI、release archive 和 repository URL 的迁移必须原子化校验，不能只改显示文本。
* `.nexa` 是否作为兼容输入长期保留，由迁移实现和版本策略另行决定；本 ADR 不隐式承诺永久双后缀。
* `futao.toml` 与 `futao.lock` 继承 ADR-009 的 manifest、lockfile、签名和可复现性合同。
* ADR-009 中历史性的 `.nexpkg` / `.nexc` 标识迁移为 `.ftpkg` / `.ftc`；容器语义、验证顺序和
  签名覆盖范围不变。

迁移完成的最低证明包括：全仓测试、Language 1.0 conformance、安装验证、release archive 验证、
`.ft` module resolution 以及旧产物的明确兼容诊断。

---

## 4. 纪念表达

仓库使用根目录 `DEDICATION.md` 保存简短双语纪念文字。README 只保留一句入口和链接，不将疾病、
病房或医学符号作为项目主视觉，也不把纪念元素强行加入每个语言概念。

视觉方向可以围绕“涛”的波形和延续感，但 logo、域名和商标仍需独立设计与检索，不由本 ADR
宣称可用性。

---

## 5. 后果

正面结果：

* 名称直接、稳定地纪念付涛，不依赖会过时的产品定位解释。
* `.ft`、`futao` 和 Futao 形成一致且简洁的品牌层级。
* 历史产物与未来品牌之间存在可审计的迁移边界。

成本与限制：

* Rust crate、CLI、CI、文档、fixture 和发布基础设施需要后续独立迁移。
* 搜索可用性、域名和商标状态仍需正式清查。
* 迁移期间文档必须同时准确说明历史 `nexac` 和未来 `futao`，不能混用命令。

---

## 6. 最终决定

```text
Language: Futao
Source:   .ft
CLI:      futao
Manifest: futao.toml
Lockfile: futao.lock
Package:  .ftpkg
Component: .ftc
```

Futao 以付涛之名命名。品牌决策立即生效，工具链标识按可验证的独立迁移逐步交付。
