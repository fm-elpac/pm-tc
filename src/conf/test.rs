use std::{collections::HashMap, error::Error};

use crate::te::EnvVar;

use super::*;

// example 文件 只用于 "最终验证" (见 load_examples).
const EXAMPLE_MAIN: &str = include_str!("../../example/pm_tc.conf.toml");
const EXAMPLE_INCLUDE: &str = include_str!("../../example/pm_ssh_tool.f.toml");

/// 内存读取器: 用于 `load` 测试, 避免 文件系统 副作用.
struct MapReader(HashMap<String, String>);

impl ReadConfFile for MapReader {
    fn read(&mut self, path: &str) -> Result<String, Box<dyn Error>> {
        self.0.get(path).cloned().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, path.to_string()).into()
        })
    }
}

/// 把 主文件 作为 `main.toml` 加载, 其余文件按 (路径, 内容) 提供.
fn load_files(main: &str, files: &[(&str, &str)]) -> Result<PmtcConf, Box<dyn Error>> {
    let mut map = HashMap::from([("main.toml".to_string(), main.to_string())]);
    for (p, t) in files {
        map.insert((*p).to_string(), (*t).to_string());
    }
    let mut src = SrcInfo::new();
    load("main.toml", &mut MapReader(map), &mut src)
}

/// 取 `[[f]]` 的 动作.
fn fa<'a>(c: &'a PmtcConfFile, name: &str) -> &'a PmtcFA {
    c.f.get(name).unwrap().as_ref().a.as_ref().unwrap().as_ref()
}

// ===== 正确情况 (自建 最简单 固定数据) =====

#[test]
fn parse_minimal() {
    let c = parse(
        "pm_tc = 1\n\
         [env]\n\
         a = \"[b]\"\n\
         b = \"x\"\n\
         \n\
         [[t.one]]\n\
         env_for = \"t.two\"\n\
         f = \"act\"\n\
         dep = [\"t.two\"]\n\
         \n\
         [[t.two]]\n\
         \n\
         [[f]]\n\
         name = \"act\"\n\
         a = \"exec\"\n\
         exec_eval = \"echo [a]\"\n",
        0,
    )
    .unwrap();

    assert_eq!(c.env.to_vec().len(), 2);
    assert!(c.task.contains_key("one"));
    assert!(c.task.contains_key("two"));
    assert!(c.f.contains_key("act"));
}

#[test]
fn parse_env_rm() {
    let c = parse(
        "pm_tc = 1\n\
         [env]\n\
         a = \"x\"\n\
         b = 0\n",
        0,
    )
    .unwrap();

    let env = c.env.to_vec();
    assert_eq!(env.len(), 2);
    assert!(matches!(&env[0].1, EnvVar::Set(v) if v == "x"));
    assert!(matches!(&env[1].1, EnvVar::Rm));
}

#[test]
fn parse_actions() {
    let c = parse(
        "pm_tc = 1\n\
         \n\
         [[f]]\nname = \"a_exec\"\na = \"exec\"\nexec_eval = \"cmd\"\n\
         \n\
         [[f]]\nname = \"a_exec_sh\"\na = \"exec_sh\"\nexec_eval = \"cmd\"\n\
         \n\
         [[f]]\nname = \"a_echo\"\na = \"echo\"\ntext_eval = \"hi\"\n\
         \n\
         [[f]]\nname = \"a_log\"\na = \"log\"\nlog_eval = [\"debug\", \"test [A]\", \"666\"]\n\
         \n\
         [[f]]\nname = \"a_cat\"\na = \"cat\"\nfile_eval = \"f\"\n\
         \n\
         [[f]]\nname = \"a_mkdir\"\na = \"mkdir\"\npath_eval = \"d\"\n\
         \n\
         [[f]]\nname = \"a_chmod\"\na = \"chmod\"\nmod = \"700\"\npath_eval = [\"d1\", \"d2\"]\n\
         \n\
         [[f]]\nname = \"a_file\"\na = \"file\"\nfile_eval = \"f\"\ntext_eval = \"t\"\n\
         \n\
         [[f]]\nname = \"a_file_append\"\na = \"file_append\"\nfile_eval = \"f\"\ntext_eval = \"t\"\n\
         \n\
         [[f]]\nname = \"a_file_merge\"\na = \"file_merge\"\nfile_eval = \"f\"\ntext_eval = \"t\"\n",
        0,
    )
    .unwrap();

    assert!(matches!(fa(&c, "a_exec"), PmtcFA::Exec(_)));
    assert!(matches!(fa(&c, "a_exec_sh"), PmtcFA::ExecSh(_)));
    assert!(matches!(fa(&c, "a_echo"), PmtcFA::Echo(_)));
    assert!(matches!(fa(&c, "a_log"), PmtcFA::Log(_)));
    assert!(matches!(fa(&c, "a_cat"), PmtcFA::Cat(_)));
    assert!(matches!(fa(&c, "a_mkdir"), PmtcFA::Mkdir(_)));
    assert!(matches!(fa(&c, "a_chmod"), PmtcFA::Chmod(_)));
    assert!(matches!(fa(&c, "a_file"), PmtcFA::File(_)));
    assert!(matches!(fa(&c, "a_file_append"), PmtcFA::FileAppend(_)));
    assert!(matches!(fa(&c, "a_file_merge"), PmtcFA::FileMerge(_)));
}

#[test]
fn load_include() {
    let c = load_files(
        "pm_tc = 1\n[[include]]\nfile = \"inc.toml\"\n[[t.m]]\nf = \"k\"\n",
        &[(
            "inc.toml",
            "pm_tc = 1\n[[f]]\nname = \"k\"\na = \"echo\"\ntext_eval = \"hi\"\n",
        )],
    )
    .unwrap();

    assert_eq!(c.env.len(), 2);
    assert!(c.task.contains_key("m"));
    assert!(c.f.contains_key("k"));
}

#[test]
fn load_examples() {
    let mut r = MapReader(HashMap::from([
        ("example/pm_tc.conf.toml".into(), EXAMPLE_MAIN.into()),
        ("example/pm_ssh_tool.f.toml".into(), EXAMPLE_INCLUDE.into()),
    ]));
    let mut src = SrcInfo::new();
    let c = load("example/pm_tc.conf.toml", &mut r, &mut src).unwrap();

    assert_eq!(c.eval_rec, 16);
    assert!(c.task.contains_key("init"));
    assert!(c.f.contains_key("gen_key"));
}

// ===== 错误情况 (只判断 出错, 不看具体错误信息) =====

#[test]
fn err_file_id() {
    let src = SrcInfo::new();
    // file_id 超出 已加载 源文件 范围
    assert!(src.get(0).is_err());
}

#[test]
fn err_toml() {
    // 不是 合法 TOML
    assert!(parse("[", 0).is_err());
}

#[test]
fn err_conf_version_missing() {
    // 缺少 pm_tc 版本号
    assert!(parse("x = 1\n", 0).is_err());
}

#[test]
fn err_conf_version_unsupported() {
    // 版本号 != 1
    assert!(parse("pm_tc = 2\n", 0).is_err());
}

#[test]
fn err_te() {
    // 变量名 含空格, 触发 te 的 check_name
    assert!(parse("pm_tc = 1\n[env]\n\"a b\" = \"x\"\n", 0).is_err());
}

#[test]
fn err_key() {
    // 未知的 toml key
    assert!(parse("pm_tc = 1\nfoo = 1\n", 0).is_err());
}

#[test]
fn err_value_type() {
    // eval_rec 要求 整数, 这里给 字符串
    assert!(parse("pm_tc = 1\neval_rec = \"x\"\n", 0).is_err());
}

#[test]
fn err_env_rm_value() {
    // [env] 中 整数 只能是 0 (删除变量), 其它整数 报错
    assert!(parse("pm_tc = 1\n[env]\nA = 1\n", 0).is_err());
}

#[test]
fn err_value_range() {
    // once 只允许 0 / 1
    assert!(parse("pm_tc = 1\n[[f]]\nname = \"a\"\nonce = 2\n", 0).is_err());
}

#[test]
fn err_f_name_missing() {
    // [[f]] 缺少 name
    assert!(parse("pm_tc = 1\n[[f]]\na = \"echo\"\ntext_eval = \"hi\"\n", 0).is_err());
}

#[test]
fn err_f_name_empty() {
    // [[f]] name 为空字符串
    assert!(
        parse(
            "pm_tc = 1\n[[f]]\nname = \"\"\na = \"echo\"\ntext_eval = \"hi\"\n",
            0
        )
        .is_err()
    );
}

#[test]
fn err_include_dup() {
    // 同一文件里 include 相同路径 两次
    assert!(
        parse(
            "pm_tc = 1\n[[include]]\nfile = \"a.toml\"\n[[include]]\nfile = \"a.toml\"\n",
            0
        )
        .is_err()
    );
}

#[test]
fn err_include_cycle() {
    // main -> b -> main, 循环 include
    assert!(
        load_files(
            "pm_tc = 1\n[[include]]\nfile = \"b.toml\"\n",
            &[("b.toml", "pm_tc = 1\n[[include]]\nfile = \"main.toml\"\n")],
        )
        .is_err()
    );
}

#[test]
fn err_dep_cycle() {
    // t.a -> t.b -> t.a 循环依赖
    assert!(
        load_files(
            "pm_tc = 1\n[[t.a]]\ndep = [\"t.b\"]\n[[t.b]]\ndep = [\"t.a\"]\n",
            &[],
        )
        .is_err()
    );
}

#[test]
fn err_fa_missing() {
    // a = "exec" 缺少 exec_eval
    assert!(parse("pm_tc = 1\n[[f]]\nname = \"a\"\na = \"exec\"\n", 0).is_err());
}

#[test]
fn err_fa_empty() {
    // exec_eval 不能为空
    assert!(
        parse(
            "pm_tc = 1\n[[f]]\nname = \"a\"\na = \"exec\"\nexec_eval = \"\"\n",
            0
        )
        .is_err()
    );
}

#[test]
fn err_fa_log_missing() {
    // a = "log" 缺少 log_eval
    assert!(parse("pm_tc = 1\n[[f]]\nname = \"a\"\na = \"log\"\n", 0).is_err());
}

#[test]
fn err_fa_log_empty() {
    // log_eval 不能为空
    assert!(
        parse(
            "pm_tc = 1\n[[f]]\nname = \"a\"\na = \"log\"\nlog_eval = []\n",
            0
        )
        .is_err()
    );
}

#[test]
fn err_fa_log_extra() {
    // a = "log" 不允许使用 text_eval
    assert!(
        parse(
            "pm_tc = 1\n[[f]]\nname = \"a\"\na = \"log\"\nlog_eval = [\"debug\", \"x\"]\ntext_eval = \"y\"\n",
            0
        )
        .is_err()
    );
}

#[test]
fn err_dep_t() {
    // env_for 引用的 task 不存在
    assert!(load_files("pm_tc = 1\n[[t.a]]\nenv_for = \"t.missing\"\n", &[]).is_err());
}

#[test]
fn err_dep_f() {
    // dep 引用的 f 不存在
    assert!(
        load_files(
            "pm_tc = 1\n[[f]]\nname = \"a\"\ndep = [\"f.missing\"]\n",
            &[],
        )
        .is_err()
    );
}

#[test]
fn err_f_missing() {
    // task 引用的 f 不存在
    assert!(load_files("pm_tc = 1\n[[t.a]]\nf = \"missing\"\n", &[]).is_err());
}
