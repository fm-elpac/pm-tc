use super::*;
use crate::conf::SrcP;

// ===== 正确情况 =====

#[test]
fn eval_basic() {
    let mut env = PmtcEnv::new();
    env.set("NAME", "x");

    assert_eq!(eval("a[NAME]b", &env, 16).unwrap(), "axb");
}

#[test]
fn eval_skip() {
    let env = PmtcEnv::new();

    // `[:NAME]` 本次不展开
    assert_eq!(eval("[:NAME]", &env, 16).unwrap(), "[NAME]");
}

#[test]
fn eval_multi() {
    let mut env = PmtcEnv::new();
    env.set("a", "[b].x");
    env.set("b", "B");

    // `[a]` 只展开 1 层
    assert_eq!(eval("[a]", &env, 16).unwrap(), "[b].x");
    // `[2:a]` 展开 2 层
    assert_eq!(eval("[2:a]", &env, 16).unwrap(), "B.x");
}

#[test]
fn eval_unknown_empty() {
    let env = PmtcEnv::new();

    // 不存在的变量替换为 ""
    assert_eq!(eval("[MISSING]", &env, 16).unwrap(), "");
}

#[test]
fn end_escape() {
    assert_eq!(end(r"\[Unit\]").unwrap(), "[Unit]");
    assert_eq!(end(r"a\\b").unwrap(), r"a\b");
}

#[test]
fn exec_eval_home_and_split() {
    let mut env = PmtcEnv::new();
    env.set("HOME", "/home/u");
    env.set("X", "v");

    assert_eq!(
        exec_eval("~/a [X] b", &env, 16).unwrap(),
        vec!["/home/u/a".to_string(), "v".to_string(), "b".to_string()]
    );
}

#[test]
fn conf_env_in_order() {
    let mut env = PmtcEnv::new();
    env.set("A", "1");

    let mut l1 = PmtcEnvList::new();
    l1.add("B", "[A]x", SrcP::new(0, 0));
    l1.add("A", "2", SrcP::new(0, 1));

    let mut l2 = PmtcEnvList::new();
    l2.add("C", "[A]", SrcP::new(0, 2));

    let out = conf_env(&env, vec![l1, l2], 16).unwrap();
    assert_eq!(out.get("A").unwrap().as_str(), "2");
    assert_eq!(out.get("B").unwrap().as_str(), "1x");
    assert_eq!(out.get("C").unwrap().as_str(), "2");
}

// ===== 错误情况 (只判断 出错, 不看具体错误信息) =====

#[test]
fn err_escape_bad_char() {
    // `\q`: `q` 不是允许的 转义字符 (只允许 \\、\[、\])
    assert!(end(r"\q").is_err());
}

#[test]
fn err_escape_end() {
    // 末尾的 `\` 后面没有字符
    assert!(end(r"a\").is_err());
}

#[test]
fn err_name_char() {
    // 变量名 不能包含 特殊字符 (这里包含空格)
    assert!(check_name("a b", 0).is_err());
}

#[test]
fn err_name_nfkc() {
    // `①` NFKC 之后变成 `1`, 名称 发生变化
    assert!(check_name("①", 0).is_err());
}

#[test]
fn err_name_empty() {
    // 变量名 不能为空
    assert!(check_name("", 0).is_err());
}

#[test]
fn err_ve() {
    // `[` 没有 匹配的 `]`
    assert!(parse_t("a[b").is_err());
}

#[test]
fn err_vf() {
    // `[1:NAME]`: 数字 n 必须 >= 2
    assert!(parse_t("[1:NAME]").is_err());
}

#[test]
fn err_eval_rec() {
    let env = PmtcEnv::new();
    // `[2:x]` 需要 递归展开, 但 r = 0 不允许 递归
    assert!(eval("[2:x]", &env, 0).is_err());
}
