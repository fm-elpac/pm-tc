# pm-tc 用户文档

胖喵模板配置工具 解析 `pm_tc.conf.toml`, 按依赖关系生成并执行 任务 (例如生成 SSH
key / 配置).

## 安装

需要 Rust 工具链 (edition 2024).

```sh
cargo build --release
# 二进制位于 target/release/pm-tc
```

## 命令行

| 命令                            | 说明                           |
| ------------------------------- | ------------------------------ |
| `pm-tc 任务 ...`                | 执行任务                       |
| `pm-tc --plan [任务 ...]`       | 只输出执行计划(不执行)         |
| `pm-tc --plan-debug [任务 ...]` | 输出计划及内部调试信息         |
| `pm-tc --eval 表达式`           | 求值模板                       |
| `pm-tc --end 表达式`            | 去转义字符串                   |
| `pm-tc --exec-eval 表达式`      | 求值 exec 命令                 |
| `pm-tc --env [任务 ...]`        | 输出已使用的系统环境变量(JSON) |
| `pm-tc --help` / `--帮助`       | 帮助(英文 / 中文)              |

默认读取当前目录的 `pm_tc.conf.toml`, 可用环境变量 `PMTC_CONF` 指定.

## 配置文件

格式版本固定为 `pm_tc = 1`.

简单示例:

```toml
pm_tc = 1

[[include]]
file = "pm_ssh_tool.f.toml"

[env]
KEY_FILE = "~/.ssh/id_ed25519-[KEY_NAME]"

[[t.gen-key]]
env_for = "t._env"
f = "gen_key"

[[f]]
name = "gen_key"
a = "exec"
exec_eval = "[2:ssh_keygen]"
```

- `eval_rec`: 模板最大递归深度 (默认 16, CLI 上限 200).
- `[[include]]`: 引入其它配置文件.
- `[env]`: 定义/删除变量, 按出现顺序求值; 字符串值为覆盖赋值, 整数 `0`
  为删除同名变量.
- `[[t.NAME]]`: 定义任务, 可同名多个实例.
  - `env_for`: 引用其它任务的变量 (不执行该任务).
  - `f`: 调用的 `[[f]]` 名称.
  - `dep`: 依赖的任务(先执行).
  - `arg`: 使用命令行参数的个数, 通过 `@1`..`@n` 引用.
  - `env:XXX`: 本任务的变量.
- `[[f]]`: 定义功能 (动作), 名称不可重复.
  - `once`: 整个计划只执行一次.
  - `skip_file_eval`: 若文件已存在则跳过.
  - `skip_include_eval`: 若目标文件内容包含指定文本则跳过.
  - `dep`: 依赖的 f.
  - `a`: 动作类型 (见下).

路径约定: 配置文件 中的 路径 (例如 `include` 的 `file`, `path_eval`,
`file_eval`, `skip_file_eval` 等) 一律使用 UNIX 形式 (`/` 分隔), 即使运行在
Windows 上.

## 动作 `a`

| 动作          | 参数                     | 说明                                             |
| ------------- | ------------------------ | ------------------------------------------------ |
| `exec`        | `exec_eval`              | 执行外部命令                                     |
| `exec_sh`     | `exec_eval`              | 用 `sh -c` 执行命令                              |
| `echo`        | `text_eval`              | 输出文本                                         |
| `log`         | `log_eval`               | 输出日志 (`error`/`warn`/`info`/`debug`/`trace`) |
| `cat`         | `file_eval`              | 输出文件内容                                     |
| `mkdir`       | `path_eval`              | 创建目录 (`mkdir -p`)                            |
| `chmod`       | `mod`, `path_eval`       | 修改权限 (`mod` 为八进制)                        |
| `file`        | `file_eval`, `text_eval` | 替换文件内容                                     |
| `file_append` | `file_eval`, `text_eval` | 追加写入文件                                     |
| `file_merge`  | `file_eval`, `text_eval` | 合并写入 (计划阶段合并)                          |

## 模板语法

- `[NAME]`: 展开变量(不存在则替换为空).
- `[:NAME]`: 本次不展开.
- `[n:NAME]`(`n >= 2`): 展开 n 次.
- 转义: `\\` `\[` `\]`.
- 变量名不得包含 `\`, 换行, `[`, `]`, `:`, 空格, 且须 NFKC 不变.

变量 在配置文件的 `[env]` 中定义, 也可使用系统环境变量.

内置变量:

- `date` (当前日期 `yyyyMMdd`)

## 命令参数切分 (`exec` / `exec_eval`)

`exec` 动作使用 `exec_eval` 求值命令并切分参数. pm-tc 不使用 shell 的 引号 /
转义规则, 而是按空格直接切分. 这并非能力缺失: 模板引擎本身已经 提供了等价能力,
无需再引入引号 / 转义那一套. (`exec_sh` 同样接受 `exec_eval` 字段,
但它把整串文本交给 `sh -c`, 不做切分.)

`exec_eval` 的求值过程:

1. 对整个字符串执行一次 `eval()` 模板展开.
2. 按空格 (ASCII 空白) 切分, 得到参数列表.
3. 对每个参数再执行一次 `eval()`, 然后去转义 (`end()`), 并展开 `~` 路径.

因此, 只要把含空格的变量延迟到切分之后才展开, 就能正确传递带空格的参数:

```toml
[env]
A = "test 666"
B = "222 hello"

[[f]]
name = "test1"
a = "exec"
exec_eval = "touch [:A] [:B]"
```

求值过程:

1. 第一次 `eval()`: `"touch [:A] [:B]"` -> `"touch [A] [B]"`. (`[:A]`
   表示本次不展开, 所以先变成 `[A]`.)
2. 按空格切分: `["touch", "[A]", "[B]"]`.
3. 第二次逐个 `eval()`: `["touch", "test 666", "222 hello"]`.

最终执行 `touch` 的参数为 `test 666` 与 `222 hello`, 参数中的空格被正确 保留,
且无歧义.

## 环境变量

| 变量        | 说明                                                                     |
| ----------- | ------------------------------------------------------------------------ |
| `PMTC_CONF` | 配置文件路径 (默认 `pm_tc.conf.toml`)                                    |
| `PMTC_ENV`  | 保存变量文件 (默认 `pm_tc.env`)                                          |
| `PMTC_REC`  | 模板最大递归深度 (覆盖 `eval_rec`; 默认 16, CLI 最大 200)                |
| `PMTC_JSON` | `1` 时 `--plan`/`--plan-debug`/`--eval`/`--end`/`--exec-eval` 输出 JSONL |

## 运行假设

运行 pm-tc 生成计划或执行任务时, 假定外部环境保持不变: 没有其它程序或用户操作与
pm-tc 冲突或并发执行. 用户应保证这一点; pm-tc 不处理由此导致的故障或 BUG.

## 示例

见 `example/pm_tc.conf.toml` 与 `example/pm_ssh_tool.f.toml` (生成 SSH key
与远程转发服务).
