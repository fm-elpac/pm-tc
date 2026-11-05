//! pm-tc CLI
use std::process::ExitCode;

mod debug;
mod help;
mod task;

pub use task::CheckEnvDefault;

use help::{print_err, print_err_0};

/// 命令行执行入口
pub fn main(a: Vec<String>) -> Result<(), ExitCode> {
    // 命令行参数解析处理
    if a.len() > 0 {
        // 第 1 个参数: 命令
        let r: Vec<String> = (&a[1..]).into();
        match a[0].as_str() {
            "--help" | "--help-en" => {
                help::help_en();
                Ok(())
            }
            "--帮助" | "--help-zh" => {
                help::help_zh();
                Ok(())
            }

            "--eval" => debug::c_eval(r),
            "--end" => debug::c_end(r),
            "--exec-eval" => debug::c_exec_eval(r),
            "--env" => debug::c_env(r),

            "--plan" => task::c_plan(r),
            "--plan-debug" => task::c_plan_debug(r),
            // task (任务名 作为 首参, 需要传入 全部参数)
            _ => task::c_task(a),
        }
    } else {
        eprintln!("ERROR: Bad command, try --help");
        Err(ExitCode::from(1))
    }
}
