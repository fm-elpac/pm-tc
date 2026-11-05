# AGENTS.md

pm-tc (胖喵模板配置工具): 解析/检查 `pm_tc.conf.toml`,
按依赖关系生成并执行任务计划.

## 常用命令

```sh
cargo fmt
deno fmt
cargo build
cargo test
cargo doc
cargo build --release
```

## 代码约定

- 源码注释 与 文档注释 一律使用中文 (可以含空格). 标识符与配置关键字使用英文.
- 禁止 `unsafe` 代码 (`#![deny(unsafe_code)]`).
- 模块分工: `cli` 命令入口, `conf` 配置解析, `te` 模板引擎, `task` 任务计划, `f`
  基础操作, `check` 公共检查.
- 错误处理: 每个模块定义自己的错误枚举, 顶层 `PmtcError` 用 `From` 汇总;
  错误显示仅为尽力而为.
- 源码中的 `TODO` 表示未完成, 属正常.

## 文档约定

- 中文 文档为准, 英文翻译 由 AI 自动同步.
- 文档 内容应该精简. 中文 文档 (以及 代码注释) 也必须使用 英文标点 (ASCII).
- 开发文档: `doc/dev/` (中文), `doc/dev_en/` (英文).
- 用户文档: `doc/user/` (中文), `doc/user_en/` (英文).
- 项目入口 `README.md` 采用中英混写: 一段中文后紧跟一段对应英文翻译.
- 修改代码或配置格式时, 同步更新对应文档.
