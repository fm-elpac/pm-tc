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
fn home_missing_keeps_tilde() {
    let env = PmtcEnv::new();

    // 缺失 HOME 时, get_home 用 `~` 占位, 使 `~` 路径 保持原样.
    let home = get_home(&env);
    assert_eq!(home, "~");

    assert_eq!(path_home("~/a".to_string(), &home), "~/a");
    assert_eq!(path_home("~".to_string(), &home), "~");
    assert_eq!(path_home("~/".to_string(), &home), "~/");
    assert_eq!(path_home("/x".to_string(), &home), "/x");
    assert_eq!(path_home("~user/x".to_string(), &home), "~user/x");
}

#[test]
fn exec_eval_home_missing() {
    let env = PmtcEnv::new();

    // 缺失 HOME 时, `~` 路径 不展开.
    assert_eq!(exec_eval("~/a", &env, 16).unwrap(), vec!["~/a".to_string()]);
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

    let (out, _) = conf_env(&env, vec![l1, l2], 16).unwrap();
    assert_eq!(out.get("A").unwrap().as_str(), "2");
    assert_eq!(out.get("B").unwrap().as_str(), "1x");
    assert_eq!(out.get("C").unwrap().as_str(), "2");
}

#[test]
fn conf_env_rm() {
    let mut env = PmtcEnv::new();
    env.set("A", "1");
    env.set("B", "2");

    let mut l = PmtcEnvList::new();
    l.add_rm("A", SrcP::new(0, 0));
    l.add("C", "[B]", SrcP::new(0, 1));

    let (out, _) = conf_env(&env, vec![l], 16).unwrap();
    assert!(out.get("A").is_none());
    assert_eq!(out.get("B").as_deref(), Some("2"));
    assert_eq!(out.get("C").as_deref(), Some("2"));
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

// ===== 新功能 (eval_use / exec_eval_use) =====

#[test]
fn eval_use_marks_use_and_unknown() {
    let mut env = PmtcEnv::new();
    env.set("A", "1");
    env.set_label("B", "2", PMTC_ENV_LABEL_SYS);

    let mut u = HashSet::new();
    let r = eval_use("[A][B][MISS]", &mut env, 16, &mut u).unwrap();
    assert_eq!(r, "12");

    let used = env.use_get();
    assert!(used.contains(&("A".to_string(), PMTC_ENV_LABEL_DEFAULT)));
    assert!(used.contains(&("B".to_string(), PMTC_ENV_LABEL_SYS)));
    assert_eq!(used.len(), 2);
    assert!(u.contains("MISS"));
    assert_eq!(u.len(), 1);
}

#[test]
fn exec_eval_use_marks_use_and_unknown() {
    let mut env = PmtcEnv::new();
    env.set("HOME", "/h");
    env.set("X", "v");

    let mut u = HashSet::new();
    let a = exec_eval_use("echo [X] [MISS] ~/a", &mut env, 16, &mut u).unwrap();
    assert_eq!(
        a,
        vec!["echo".to_string(), "v".to_string(), "/h/a".to_string()]
    );

    let used = env.use_get();
    assert!(used.contains(&("X".to_string(), PMTC_ENV_LABEL_DEFAULT)));
    assert!(!used.contains(&("HOME".to_string(), PMTC_ENV_LABEL_DEFAULT)));
    assert!(u.contains("MISS"));
}

#[test]
fn conf_env_unknown() {
    let env = PmtcEnv::new();

    let mut l = PmtcEnvList::new();
    l.add("A", "[MISS]", SrcP::new(0, 0));

    let (out, u) = conf_env(&env, vec![l], 16).unwrap();
    assert_eq!(out.get("A").as_deref(), Some(""));
    assert!(u.contains("MISS"));
}

// ===== 旧功能 补充测试 =====

#[test]
fn eval_end_escape() {
    let mut env = PmtcEnv::new();
    env.set("A", "x");

    let v = eval(r"\[[A]\]", &env, 16).unwrap();
    assert_eq!(end(&v).unwrap(), "[x]");
}

#[test]
fn parse_t_parts() {
    let p = parse_t("a[b]c").unwrap();
    assert_eq!(p.len(), 3);
    match &p[0] {
        Part::S(s) => assert_eq!(s, "a"),
        _ => panic!("part 0 不是普通字符串"),
    }
    match &p[1] {
        Part::V(v) => assert_eq!(v.name, "b"),
        _ => panic!("part 1 不是变量"),
    }
    match &p[2] {
        Part::S(s) => assert_eq!(s, "c"),
        _ => panic!("part 2 不是普通字符串"),
    }
}

#[test]
fn check_t_ok() {
    assert_eq!(check_t("[a]").unwrap(), "[a]");
}

#[test]
fn p_env_use_clear_and_rm() {
    let mut env = PmtcEnv::new();
    env.set("A", "1");

    assert_eq!(env.get_use("A").as_deref(), Some("1"));
    assert_eq!(env.use_get().len(), 1);

    env.use_clear();
    assert!(env.use_get().is_empty());

    assert_eq!(env.rm("A").map(|i| i.0).as_deref(), Some("1"));
    assert!(env.get("A").is_none());
}
