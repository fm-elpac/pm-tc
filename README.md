# pm-tc

<https://github.com/fm-elpac/pm-tc>

![CI](https://github.com/fm-elpac/pm-tc/actions/workflows/ci.yml/badge.svg)

胖喵模板配置工具.

PM Template Config Tool.

> 本项目使用 AI (kimi, deepseek) 辅助制作.
>
> This project is made with the assistance of AI (kimi, deepseek).

pm-tc 解析并检查 `pm_tc.conf.toml`, 按依赖关系生成并执行任务计划.

pm-tc parses and checks `pm_tc.conf.toml`, then builds and executes a task plan
according to dependencies.

## 文档 Documentation

开发文档见 `doc/dev/`(中文) / `doc/dev_en/`(英文翻译); 用户文档见
`doc/user/`(中文) / `doc/user_en/`(英文翻译). 以中文文档为准.

Development docs are in `doc/dev/` (Chinese) / `doc/dev_en/` (English
translation); user docs are in `doc/user/` (Chinese) / `doc/user_en/` (English
translation). The Chinese docs are authoritative.

## 快速开始 Quick Start

编译后运行, 查看命令行帮助.

Build and run to show the CLI help.

```sh
cargo build --release
./target/release/pm-tc --help
```

## LICENSE

`MIT`
