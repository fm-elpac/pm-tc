# pm-tc User Guide

pm-tc (PM Template Config Tool) parses `pm_tc.conf.toml` and, based on
dependencies, builds and executes tasks (e.g. generating SSH keys / config).

## Installation

Requires the Rust toolchain (edition 2024).

```sh
cargo build --release
# binary at target/release/pm-tc
```

## Command Line

| Command                         | Description                            |
| ------------------------------- | -------------------------------------- |
| `pm-tc TASK ...`                | Run tasks                              |
| `pm-tc --plan [TASK ...]`       | Print the plan only (no execution)     |
| `pm-tc --plan-debug [TASK ...]` | Print the plan and internal debug info |
| `pm-tc --eval EXPR`             | Evaluate a template                    |
| `pm-tc --end EXPR`              | Unescape a string                      |
| `pm-tc --exec-eval EXPR`        | Evaluate an exec command               |
| `pm-tc --env [TASK ...]`        | Print used system env vars (JSON)      |
| `pm-tc --help` / `--help-zh`    | Help (English / Chinese)               |

By default it reads `pm_tc.conf.toml` from the current directory; use
`PMTC_CONF` to specify another file.

## Config File

The format version is fixed at `pm_tc = 1`.

Simple example:

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

- `eval_rec`: max template recursion depth (default 16, CLI max 200).
- `[[include]]`: include other config files.
- `[env]`: define/remove variables, evaluated in order; a string value sets
  (overrides) the variable, and the integer `0` removes it.
- `[[t.NAME]]`: define a task; multiple instances of the same name are allowed.
  - `env_for`: reference another task's variables (without running it).
  - `f`: name of the `[[f]]` to call.
  - `dep`: tasks to run first.
  - `arg`: number of command-line arguments, referenced as `@1`..`@n`.
  - `env:XXX`: variables local to this task.
- `[[f]]`: define a function (action); names must be unique.
  - `once`: run at most once in the whole plan.
  - `skip_file_eval`: skip if the file already exists.
  - `skip_include_eval`: skip if the target file already contains the text.
  - `dep`: dependent functions.
  - `a`: action type (see below).

Path convention: paths in the config file (for example `include` `file`,
`path_eval`, `file_eval`, `skip_file_eval`) always use UNIX form (`/`
separated), even when running on Windows.

## Action `a`

| Action        | Parameters               | Description                                             |
| ------------- | ------------------------ | ------------------------------------------------------- |
| `exec`        | `exec_eval`              | Run an external command                                 |
| `exec_sh`     | `exec_eval`              | Run a command via `sh -c`                               |
| `echo`        | `text_eval`              | Print text                                              |
| `log`         | `log_eval`               | Emit a log line (`error`/`warn`/`info`/`debug`/`trace`) |
| `cat`         | `file_eval`              | Print file contents                                     |
| `mkdir`       | `path_eval`              | Create a directory (`mkdir -p`)                         |
| `chmod`       | `mod`, `path_eval`       | Change permissions (`mod` is octal)                     |
| `file`        | `file_eval`, `text_eval` | Replace file contents                                   |
| `file_append` | `file_eval`, `text_eval` | Append to a file                                        |
| `file_merge`  | `file_eval`, `text_eval` | Merge writes (combined during planning)                 |

## Template Syntax

- `[NAME]`: expand the variable (missing variables become empty).
- `[:NAME]`: do not expand this time.
- `[n:NAME]` (`n >= 2`): expand n times.
- Escapes: `\\` `\[` `\]`.
- Variable names must not contain `\`, newline, `[`, `]`, `:`, or space, and
  must be unchanged under NFKC.

Variables are defined in the config file's `[env]`, and system environment
variables can also be used.

Built-in variables:

- `date` (current date `yyyyMMdd`)

## Command Argument Splitting (`exec` / `exec_eval`)

The `exec` action uses `exec_eval` to evaluate its command and split it into
arguments. pm-tc does not use shell quoting / escaping rules; it splits on
spaces directly. This is not a missing capability: the template engine already
provides the equivalent, so there is no need to introduce quoting / escaping.
(`exec_sh` also accepts the `exec_eval` field, but it passes the whole text to
`sh -c` without splitting.)

`exec_eval` evaluates as follows:

1. Evaluate the whole string once with `eval()` (template expansion).
2. Split on spaces (ASCII whitespace) into a list of arguments.
3. Evaluate each argument again with `eval()`, then unescape (`end()`), and
   expand `~` paths.

Therefore, arguments containing spaces can be passed correctly by deferring
their expansion until after splitting:

```toml
[env]
A = "test 666"
B = "222 hello"

[[f]]
name = "test1"
a = "exec"
exec_eval = "touch [:A] [:B]"
```

Evaluation steps:

1. First `eval()`: `"touch [:A] [:B]"` -> `"touch [A] [B]"`. (`[:A]` means "do
   not expand this time", so it first becomes `[A]`.)
2. Split on spaces: `["touch", "[A]", "[B]"]`.
3. Second per-argument `eval()`: `["touch", "test 666", "222 hello"]`.

The final `touch` arguments are `test 666` and `222 hello`; spaces inside
arguments are preserved with no ambiguity.

## Environment Variables

| Variable    | Description                                                                         |
| ----------- | ----------------------------------------------------------------------------------- |
| `PMTC_CONF` | Config file path (default `pm_tc.conf.toml`)                                        |
| `PMTC_ENV`  | Saved-variable file (default `pm_tc.env`)                                           |
| `PMTC_REC`  | Max template recursion depth (overrides `eval_rec`; default 16, CLI max 200)        |
| `PMTC_JSON` | Set to `1` to emit JSONL for `--plan`/`--plan-debug`/`--eval`/`--end`/`--exec-eval` |

## Runtime Assumptions

While pm-tc runs (building a plan or executing tasks), it assumes the external
environment stays unchanged: no other program or user action conflicts with, or
runs concurrently with, pm-tc. You must guarantee this; pm-tc does not handle
failures or bugs caused by such interference.

## Examples

See `example/pm_tc.conf.toml` and `example/pm_ssh_tool.f.toml` (generating SSH
keys and a remote forward service).
