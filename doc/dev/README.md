# pm-tc 开发文档

胖喵模板配置工具 解析/检查 配置文件 (默认 `pm_tc.conf.toml`),
按依赖关系生成并执行任务计划.

## 模块结构

| 模块    | 说明                                      |
| ------- | ----------------------------------------- |
| `cli`   | 命令行入口与子命令                        |
| `conf`  | 配置文件(`*.toml`) 解析 / 加载 / 错误检查 |
| `te`    | 模板(字符串展开) 引擎                     |
| `task`  | 任务 / 依赖, 生成执行计划                 |
| `f`     | 底层基础操作(默认执行器)                  |
| `check` | task/f 依赖引用公共检查                   |
| `error` | 顶层错误类型                              |

## 核心设计

- **配置解析分两阶段**: 阶段 1 用 `toml_edit` DOM 反序列化为带源位置的
  中间结构(`p1`); 阶段 2 做业务校验并转为 `PmtcConfFile`(`p`). 所有叶子值用
  `HasP<T>` 包装以保留源位置, 用于报错.
- **尽量早检查**: 除动态执行外, 错误尽量在配置 / 计划阶段检查,
  而不是推迟到执行阶段.
- **模板引擎**(`te`): 支持 `[NAME]`, `[:NAME]`, `[n:NAME]` 三种变量 展开, 转义
  `\\` `\[` `\]`; 递归深度受 `eval_rec` 限制.
- **变量操作**(`[env]`): 字符串值表示定义/覆盖变量 (`EnvVar::Set`), 整数 `0`
  表示删除同名变量 (`EnvVar::Rm`), 按出现顺序执行.
- **执行计划**(`task::plan`): 先求值全部 `[env]` 得到只读基础环境, 再按 task
  依赖递归展开为动作列表(`PmtcRunList`), 执行阶段只做 文件 / 命令等操作;
  `file_merge` 在计划阶段合并为单个 `file`.
- **运行假设**: `task::plan()` 与 `task::run()` 执行期间, 假定外部环境保持不变:
  没有其它程序或用户操作与 pm-tc 冲突或并发执行. 调用者应保证这一点; pm-tc
  不处理由此导致的故障或 BUG.

## 项目约定

- 源码注释 / 文档注释 一律使用 中文 (可以含空格), 标识符 / 配置关键字为英文.
- 各 crate 根模块使用 `#![deny(unsafe_code)]`.
- 错误处理: 每个模块定义自己的错误枚举(`ConfError`, `TeError`, `TaskError`,
  `FError`), 顶层 `PmtcError` 用 `From` 汇总; 错误显示 仅为尽力而为,
  便于开发调试.
- 依赖 `pm-bin` 提供 CLI 脚手架(`pm_init!`, `init_env_logger`, `cli_arg`,
  `build_gen`).
- feature: `cli`(默认, 含 `task` `f`), `task`, `f`.
- 源码中遗留 `TODO` 表示尚未完成, 属正常.

## 代码 (文档) 格式化

```sh
cargo fmt  # rust 代码
deno fmt  # 其余文件 (比如 markdown)
```

## 构建与测试

```sh
cargo build
cargo test
cargo doc
cargo build --release
```
