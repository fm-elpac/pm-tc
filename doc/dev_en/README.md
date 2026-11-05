# pm-tc Developer Documentation

pm-tc (PM Template Config Tool) parses/checks the config file (default
`pm_tc.conf.toml`) and, based on dependencies, builds and executes a task plan.

## Module Layout

| Module  | Description                                          |
| ------- | ---------------------------------------------------- |
| `cli`   | CLI entry point and subcommands                      |
| `conf`  | Config file (`*.toml`) parse / load / error checking |
| `te`    | Template (string expansion) engine                   |
| `task`  | Tasks / dependencies, build the execution plan       |
| `f`     | Low-level base operations (default executor)         |
| `check` | Shared dependency/reference checks for task/f        |
| `error` | Top-level error type                                 |

## Core Design

- **Two-phase config parsing**: phase 1 deserializes the `toml_edit` DOM into an
  intermediate structure carrying source positions (`p1`); phase 2 performs
  semantic checks and converts to `PmtcConfFile` (`p`). All leaf values are
  wrapped in `HasP<T>` to keep source positions for error reporting.
- **Check as early as possible**: except for dynamic execution, errors are
  checked during the config/plan phase rather than deferred to execution.
- **Template engine** (`te`): supports `[NAME]`, `[:NAME]`, `[n:NAME]` variable
  expansion and escapes `\\` `\[` `\]`; recursion depth is limited by
  `eval_rec`.
- **Execution plan** (`task::plan`): first evaluates all `[env]` into a
  read-only base environment, then recursively expands tasks into an action list
  (`PmtcRunList`) following dependencies. `file_merge` actions are merged into a
  single `file` during planning.

## Project Conventions

- Source and doc comments are written in Chinese (with spaces); identifiers and
  config keywords are in English.
- All crate root modules use `#![deny(unsafe_code)]`.
- Error handling: each module defines its own error enum (`ConfError`,
  `TeError`, `TaskError`, `FError`); the top-level `PmtcError` aggregates them
  via `From`. Error display is best-effort, intended for development/debugging.
- `pm-bin` provides the CLI scaffolding (`pm_init!`, `init_env_logger`,
  `cli_arg`, `build_gen`).
- Features: `cli` (default, includes `task` `f`), `task`, `f`.
- Leftover `TODO` markers in the source indicate unfinished work and are normal.

## Code (Doc) Formatting

```sh
cargo fmt  # Rust code
deno fmt  # other files (e.g. markdown)
```

## Build & Test

```sh
cargo build
cargo test
cargo doc
cargo build --release
```
