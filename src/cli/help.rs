//! 输出命令行帮助信息
use std::{error::Error, process::ExitCode};

use crate::conf::SrcInfo;

use super::i18n;

/// 英文帮助 (`--help` / `--help-en`)
pub fn help_en() {
    println!(
        r#"pm-tc: template config tool

Usage:
  pm-tc [TASK ...]              run tasks
  pm-tc --plan [TASK ...]        print plan (no run)
  pm-tc --plan-debug [TASK ...]  print plan + debug info
  pm-tc --eval EXPR ...         eval template
  pm-tc --end EXPR ...          unescape string
  pm-tc --exec-eval EXPR ...    eval exec command
  pm-tc --env [TASK ...]        print used system env (JSON)
  pm-tc --help                  show this help (English)
  pm-tc --help-zh               show Chinese help
  pm-tc --version               show version"#
    );
}

/// 中文帮助 (`--帮助` / `--help-zh`)
pub fn help_zh() {
    println!(
        r#"pm-tc: 胖喵模板配置工具

用法:
  pm-tc 任务 ...              执行任务
  pm-tc --plan [任务 ...]      只输出执行计划 (不执行)
  pm-tc --plan-debug [任务 ...] 输出执行计划及内部调试信息
  pm-tc --eval 表达式 ...     求值模板
  pm-tc --end 表达式 ...      去转义字符串
  pm-tc --exec-eval 表达式 ... 求值 exec 命令
  pm-tc --env [任务 ...]      输出已使用的系统环境变量 (JSON)
  pm-tc --help                显示英文帮助
  pm-tc --帮助                显示中文帮助
  pm-tc --版本                显示版本"#
    );
}

/// 命令行参数错误信息
pub fn bad_cli_arg() {
    eprintln!("ERROR: Bad command arg, try --help");
}

/// 打印错误信息.
///
/// 优先使用 友好错误显示; 无法处理时 回退到 原始显示.
pub fn print_err(err: Box<dyn Error>, src: &SrcInfo) -> ExitCode {
    match i18n::t_err(&err, src) {
        Some(msg) => eprintln!("ERROR: {}", msg),
        None => eprintln!("ERROR: {}", err),
    }
    ExitCode::from(1)
}

/// 打印错误信息 (泛型包装).
pub fn print_err_0<E: Error + 'static>(err: E, src: &SrcInfo) -> ExitCode {
    print_err(Box::new(err), src)
}
