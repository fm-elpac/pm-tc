//! pm-tc CLI
use std::{env, process::ExitCode};

use crate::conf::{ENV_PMTC_JSON, ENV_PMTC_REC};

pub mod i18n;

mod debug;
mod help;
mod task;

pub use task::CheckEnvDefault;

use help::{print_err, print_err_0};

/// CLI 允许的 最大递归深度 (内置上限). 超出该值一律截断为该值.
pub(crate) const EVAL_REC_MAX: u16 = 200;

/// 计算 CLI 使用的 递归深度:
/// 环境变量 `PMTC_REC` 覆盖 `base`, 结果不超过内置上限.
pub(crate) fn eval_rec(base: u16) -> u16 {
    let r = env::var(ENV_PMTC_REC)
        .ok()
        .and_then(|i| i.parse::<u16>().ok())
        .unwrap_or(base);
    limit_eval_rec(r)
}

/// 把递归深度限制在 内置上限 以内.
fn limit_eval_rec(r: u16) -> u16 {
    r.min(EVAL_REC_MAX)
}

/// 读取 `PMTC_JSON` 环境变量, 判断是否启用 JSON 输出.
///
/// - 未设置: 不启用.
/// - 值为 `1`: 启用.
/// - 其它值: 输出 警告 并 忽略 (不启用).
pub(crate) fn json_out() -> bool {
    match env::var(ENV_PMTC_JSON) {
        Ok(v) => {
            if v == "1" {
                true
            } else {
                log::warn!("`{}` ignored (expected `1`, got `{}`)", ENV_PMTC_JSON, v);
                false
            }
        }
        Err(_) => false,
    }
}

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

#[cfg(test)]
mod test {
    use super::{EVAL_REC_MAX, limit_eval_rec};

    #[test]
    fn limit_eval_rec_clamps() {
        assert_eq!(limit_eval_rec(0), 0);
        assert_eq!(limit_eval_rec(EVAL_REC_MAX - 1), EVAL_REC_MAX - 1);
        assert_eq!(limit_eval_rec(EVAL_REC_MAX), EVAL_REC_MAX);
        assert_eq!(limit_eval_rec(EVAL_REC_MAX + 1), EVAL_REC_MAX);
        assert_eq!(limit_eval_rec(u16::MAX), EVAL_REC_MAX);
    }
}
