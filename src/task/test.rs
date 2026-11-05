use std::{collections::HashMap, error::Error, io};

use crate::{
    conf::{HasP, PmtcConf, PmtcF, PmtcFA, PmtcFChmod, PmtcFFileWrite, PmtcTask, SrcInfo, SrcP},
    te::{PmtcEnv, PmtcEnvList},
};

use super::{CheckEnv, PlanDebug, PmtcRunList, PmtcRunOp, plan};

fn src() -> SrcP {
    SrcP::new(0, 0)
}

fn hs<T: std::fmt::Debug + Clone>(v: T) -> HasP<T> {
    HasP::new(src(), v)
}

fn task(env: &[(&str, &str)], env_for: Option<&str>, f: Option<&str>, dep: &[&str]) -> PmtcTask {
    PmtcTask {
        _name: String::new(),
        env: env
            .iter()
            .map(|(k, v)| (k.to_string(), hs(v.to_string())))
            .collect(),
        env_for: env_for.map(|s| hs(s.to_string())),
        f: f.map(|s| hs(s.to_string())),
        dep: dep.iter().map(|s| hs(s.to_string())).collect(),
        arg: hs(0usize),
    }
}

fn task_arg(
    env: &[(&str, &str)],
    env_for: Option<&str>,
    f: Option<&str>,
    dep: &[&str],
    arg: usize,
) -> PmtcTask {
    let mut t = task(env, env_for, f, dep);
    t.arg = hs(arg);
    t
}

fn f_task(a: Option<PmtcFA>) -> PmtcF {
    f_full(a, false, None, None, &[])
}

fn f_full(
    a: Option<PmtcFA>,
    once: bool,
    skip_file: Option<&str>,
    skip_include: Option<&str>,
    dep: &[&str],
) -> PmtcF {
    PmtcF {
        _name: String::new(),
        once,
        skip_file_eval: skip_file.map(|s| hs(s.to_string())),
        skip_include_eval: skip_include.map(|s| hs(s.to_string())),
        dep: dep.iter().map(|s| hs(s.to_string())).collect(),
        a: a.map(|x| hs(x)),
    }
}

fn env_list(pairs: &[(&str, &str)]) -> PmtcEnvList {
    let mut l = PmtcEnvList::new();
    for (k, v) in pairs {
        l.add(k, v, src());
    }
    l
}

fn conf(tasks: Vec<(&str, Vec<PmtcTask>)>, f: Vec<(&str, PmtcF)>) -> PmtcConf {
    conf_env(tasks, f, vec![])
}

fn conf_env(
    tasks: Vec<(&str, Vec<PmtcTask>)>,
    f: Vec<(&str, PmtcF)>,
    env: Vec<PmtcEnvList>,
) -> PmtcConf {
    PmtcConf {
        eval_rec: 16,
        env,
        task: tasks
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.into_iter().map(|x| hs(x)).collect()))
            .collect(),
        f: f.into_iter().map(|(k, v)| (k.to_string(), hs(v))).collect(),
        src: SrcInfo::new(),
    }
}

/// 内存环境: 用于 测试 plan 的 skip 判断, 避免 文件系统 副作用.
struct MemEnv {
    exist: HashMap<String, bool>,
    files: HashMap<String, String>,
}

impl MemEnv {
    fn new() -> Self {
        Self {
            exist: HashMap::new(),
            files: HashMap::new(),
        }
    }
}

impl CheckEnv for MemEnv {
    fn file_exist(&mut self, path: &str) -> Result<bool, Box<dyn Error>> {
        Ok(self.exist.get(path).copied().unwrap_or(false))
    }

    fn file_read(&mut self, path: &str) -> Result<String, Box<dyn Error>> {
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, path.to_string()).into())
    }
}

fn ops(r: (PmtcRunList, PlanDebug)) -> Vec<PmtcRunOp> {
    r.0.to_vec()
}

fn echo_text(op: &PmtcRunOp) -> &str {
    match op {
        PmtcRunOp::Echo(t) => t.as_str(),
        _ => panic!("not echo"),
    }
}

// ===== 正确情况 =====

#[test]
fn plan_simple_exec() {
    let c = conf(
        vec![("a", vec![task(&[], None, Some("f1"), &[])])],
        vec![("f1", f_task(Some(PmtcFA::Exec(hs("echo hi".to_string())))))],
    );
    let mut ce = MemEnv::new();
    let env = PmtcEnv::new();

    let o = ops(plan(&vec!["a".to_string()], &c, &env, &mut ce).unwrap());
    assert_eq!(o.len(), 1);
    assert!(
        matches!(&o[0], PmtcRunOp::Exec(a) if a == &vec!["echo".to_string(), "hi".to_string()])
    );
}

#[test]
fn plan_dep_order() {
    let c = conf(
        vec![
            ("a", vec![task(&[], None, Some("fa"), &["b"])]),
            ("b", vec![task(&[], None, Some("fb"), &[])]),
        ],
        vec![
            ("fa", f_task(Some(PmtcFA::Echo(hs("A".to_string()))))),
            ("fb", f_task(Some(PmtcFA::Echo(hs("B".to_string()))))),
        ],
    );
    let mut ce = MemEnv::new();
    let env = PmtcEnv::new();

    let o = ops(plan(&vec!["a".to_string()], &c, &env, &mut ce).unwrap());
    assert_eq!(o.len(), 2);
    assert_eq!(echo_text(&o[0]), "B");
    assert_eq!(echo_text(&o[1]), "A");
}

#[test]
fn plan_multi_instance() {
    let c = conf(
        vec![(
            "a",
            vec![
                task(&[("X", "1")], None, Some("f"), &[]),
                task(&[("X", "2")], None, Some("f"), &[]),
            ],
        )],
        vec![("f", f_task(Some(PmtcFA::Echo(hs("[X]".to_string())))))],
    );
    let mut ce = MemEnv::new();
    let env = PmtcEnv::new();

    let o = ops(plan(&vec!["a".to_string()], &c, &env, &mut ce).unwrap());
    assert_eq!(echo_text(&o[0]), "1");
    assert_eq!(echo_text(&o[1]), "2");
}

#[test]
fn plan_dep_all_instances() {
    let c = conf(
        vec![
            ("a", vec![task(&[], None, Some("fa"), &["b"])]),
            (
                "b",
                vec![
                    task(&[("X", "1")], None, Some("fb"), &[]),
                    task(&[("X", "2")], None, Some("fb"), &[]),
                ],
            ),
        ],
        vec![
            ("fa", f_task(Some(PmtcFA::Echo(hs("A".to_string()))))),
            ("fb", f_task(Some(PmtcFA::Echo(hs("[X]".to_string()))))),
        ],
    );
    let mut ce = MemEnv::new();
    let env = PmtcEnv::new();

    let o = ops(plan(&vec!["a".to_string()], &c, &env, &mut ce).unwrap());
    assert_eq!(o.len(), 3);
    assert_eq!(echo_text(&o[0]), "1");
    assert_eq!(echo_text(&o[1]), "2");
    assert_eq!(echo_text(&o[2]), "A");
}

#[test]
fn plan_env_isolated() {
    let c = conf(
        vec![
            ("x", vec![task(&[("A", "1")], None, Some("fx"), &[])]),
            ("y", vec![task(&[("A", "2")], None, Some("fy"), &[])]),
        ],
        vec![
            ("fx", f_task(Some(PmtcFA::Echo(hs("[A]".to_string()))))),
            ("fy", f_task(Some(PmtcFA::Echo(hs("[A]".to_string()))))),
        ],
    );
    let mut ce = MemEnv::new();
    let env = PmtcEnv::new();

    let o = ops(plan(&vec!["x".to_string(), "y".to_string()], &c, &env, &mut ce).unwrap());
    assert_eq!(echo_text(&o[0]), "1");
    assert_eq!(echo_text(&o[1]), "2");
}

#[test]
fn plan_env_value_evaled() {
    // task 的 `env:NAME` 值 现在 会被 eval.
    let c = conf(
        vec![("a", vec![task(&[("A", "[X]")], None, Some("f"), &[])])],
        vec![("f", f_task(Some(PmtcFA::Echo(hs("[A]".to_string())))))],
    );
    let mut ce = MemEnv::new();
    let mut env = PmtcEnv::new();
    env.set("X", "v");

    let o = ops(plan(&vec!["a".to_string()], &c, &env, &mut ce).unwrap());
    assert_eq!(echo_text(&o[0]), "v");
}

#[test]
fn plan_arg_basic() {
    // arg = 2: 注入 @1、@2.
    let c = conf(
        vec![("a", vec![task_arg(&[], None, Some("f"), &[], 2)])],
        vec![("f", f_task(Some(PmtcFA::Echo(hs("[@1]-[@2]".to_string())))))],
    );
    let mut ce = MemEnv::new();
    let env = PmtcEnv::new();

    let o = ops(plan(
        &vec!["a".to_string(), "x".to_string(), "y".to_string()],
        &c,
        &env,
        &mut ce,
    )
    .unwrap());
    assert_eq!(echo_text(&o[0]), "x-y");
}

#[test]
fn plan_arg_in_env_eval() {
    // @n 在 eval env:XXX 之前 注入.
    let c = conf(
        vec![(
            "a",
            vec![task_arg(&[("V", "[@1]-x")], None, Some("f"), &[], 1)],
        )],
        vec![("f", f_task(Some(PmtcFA::Echo(hs("[V]".to_string())))))],
    );
    let mut ce = MemEnv::new();
    let env = PmtcEnv::new();

    let o = ops(plan(&vec!["a".to_string(), "pk".to_string()], &c, &env, &mut ce).unwrap());
    assert_eq!(echo_text(&o[0]), "pk-x");
}

#[test]
fn plan_arg_multi_instance() {
    // 同名 arg task 多个实例: 依次取多次.
    let c = conf(
        vec![
            (
                "A",
                vec![
                    task_arg(&[], None, Some("f"), &[], 1),
                    task_arg(&[], None, Some("f"), &[], 1),
                ],
            ),
            ("B", vec![task(&[], None, Some("fb"), &[])]),
        ],
        vec![
            ("f", f_task(Some(PmtcFA::Echo(hs("[@1]".to_string()))))),
            ("fb", f_task(Some(PmtcFA::Echo(hs("B".to_string()))))),
        ],
    );
    let mut ce = MemEnv::new();
    let env = PmtcEnv::new();

    let o = ops(plan(
        &vec![
            "A".to_string(),
            "c".to_string(),
            "d".to_string(),
            "B".to_string(),
        ],
        &c,
        &env,
        &mut ce,
    )
    .unwrap());
    assert_eq!(o.len(), 3);
    assert_eq!(echo_text(&o[0]), "c");
    assert_eq!(echo_text(&o[1]), "d");
    assert_eq!(echo_text(&o[2]), "B");
}

#[test]
fn plan_env_for() {
    let c = conf(
        vec![
            (
                "gen",
                vec![task(&[("N", "[M]-x")], Some("_env"), Some("f"), &[])],
            ),
            (
                "_env",
                vec![
                    task(&[("M", "m1")], None, None, &[]),
                    task(&[("M", "m2")], None, None, &[]),
                ],
            ),
        ],
        vec![("f", f_task(Some(PmtcFA::Echo(hs("[2:N]".to_string())))))],
    );
    let mut ce = MemEnv::new();
    let env = PmtcEnv::new();

    let o = ops(plan(&vec!["gen".to_string()], &c, &env, &mut ce).unwrap());
    assert_eq!(o.len(), 2);
    assert_eq!(echo_text(&o[0]), "m1-x");
    assert_eq!(echo_text(&o[1]), "m2-x");
}

#[test]
fn plan_env_for_chain() {
    let c = conf(
        vec![
            ("a", vec![task(&[], Some("b"), Some("f"), &[])]),
            ("b", vec![task(&[], Some("c"), None, &[])]),
            ("c", vec![task(&[("C", "y")], None, None, &[])]),
        ],
        vec![("f", f_task(Some(PmtcFA::Echo(hs("[C]".to_string())))))],
    );
    let mut ce = MemEnv::new();
    let env = PmtcEnv::new();

    let o = ops(plan(&vec!["a".to_string()], &c, &env, &mut ce).unwrap());
    assert_eq!(echo_text(&o[0]), "y");
}

#[test]
fn plan_log_action() {
    let c = conf(
        vec![("a", vec![task(&[("A", "x")], None, Some("f"), &[])])],
        vec![(
            "f",
            f_task(Some(PmtcFA::Log(vec![
                hs("debug".to_string()),
                hs("test [A]".to_string()),
                hs("666".to_string()),
            ]))),
        )],
    );
    let mut ce = MemEnv::new();
    let env = PmtcEnv::new();

    let o = ops(plan(&vec!["a".to_string()], &c, &env, &mut ce).unwrap());
    assert!(matches!(&o[0], PmtcRunOp::Log(a) if a == &vec![
        "debug".to_string(),
        "test x".to_string(),
        "666".to_string(),
    ]));
}

#[test]
fn plan_debug_env() {
    let c = conf(
        vec![("a", vec![task(&[("A", "[X]")], None, Some("f"), &[])])],
        vec![("f", f_task(Some(PmtcFA::Echo(hs("[A]".to_string())))))],
    );
    let mut ce = MemEnv::new();
    let mut env = PmtcEnv::new();
    env.set("X", "v");

    let (run, pd) = plan(&vec!["a".to_string()], &c, &env, &mut ce).unwrap();
    assert_eq!(run.to_vec().len(), 1);
    assert_eq!(pd.env0.get("X").as_deref(), Some("v"));
    assert_eq!(pd.task.len(), 1);
    assert_eq!(pd.task[0].name, "a");
    assert_eq!(pd.task[0].env.get("A").map(|s| s.as_str()), Some("v"));
}

#[test]
fn plan_debug_env_for_order() {
    let c = conf(
        vec![
            (
                "gen",
                vec![task(&[("N", "[M]-x")], Some("_env"), Some("f"), &[])],
            ),
            (
                "_env",
                vec![
                    task(&[("M", "m1")], None, None, &[]),
                    task(&[("M", "m2")], None, None, &[]),
                ],
            ),
        ],
        vec![("f", f_task(Some(PmtcFA::Echo(hs("[N]".to_string())))))],
    );
    let mut ce = MemEnv::new();
    let env = PmtcEnv::new();

    let (_, pd) = plan(&vec!["gen".to_string()], &c, &env, &mut ce).unwrap();
    assert_eq!(pd.task.len(), 2);
    assert_eq!(pd.task[0].name, "gen");
    assert_eq!(pd.task[1].name, "gen");
    assert_eq!(pd.task[0].env.get("N").map(|s| s.as_str()), Some("m1-x"));
    assert_eq!(pd.task[1].env.get("N").map(|s| s.as_str()), Some("m2-x"));
}

#[test]
fn plan_conf_env() {
    let c = conf_env(
        vec![("a", vec![task(&[], None, Some("f"), &[])])],
        vec![("f", f_task(Some(PmtcFA::Echo(hs("[G]".to_string())))))],
        vec![env_list(&[("G", "base")])],
    );
    let mut ce = MemEnv::new();
    let env = PmtcEnv::new();

    let o = ops(plan(&vec!["a".to_string()], &c, &env, &mut ce).unwrap());
    assert_eq!(echo_text(&o[0]), "base");
}

#[test]
fn plan_file_merge() {
    let c = conf(
        vec![
            ("a", vec![task(&[], None, Some("m1"), &[])]),
            ("b", vec![task(&[], None, Some("m2"), &[])]),
        ],
        vec![
            (
                "m1",
                f_task(Some(PmtcFA::FileMerge(PmtcFFileWrite {
                    file_eval: hs("out".to_string()),
                    text_eval: hs("A".to_string()),
                }))),
            ),
            (
                "m2",
                f_task(Some(PmtcFA::FileMerge(PmtcFFileWrite {
                    file_eval: hs("out".to_string()),
                    text_eval: hs("B".to_string()),
                }))),
            ),
        ],
    );
    let mut ce = MemEnv::new();
    let env = PmtcEnv::new();

    let o = ops(plan(&vec!["a".to_string(), "b".to_string()], &c, &env, &mut ce).unwrap());
    assert_eq!(o.len(), 1);
    match &o[0] {
        PmtcRunOp::FileReplace((p, t)) => {
            assert_eq!(p, "out");
            assert_eq!(t, "AB");
        }
        _ => panic!("not file_replace"),
    }
}

#[test]
fn plan_file_append_multi_ok() {
    // 同一个文件 多次 file_append 是允许的 (同一种 写入操作).
    let c = conf(
        vec![
            ("a", vec![task(&[], None, Some("w1"), &[])]),
            ("b", vec![task(&[], None, Some("w2"), &[])]),
        ],
        vec![
            (
                "w1",
                f_task(Some(PmtcFA::FileAppend(PmtcFFileWrite {
                    file_eval: hs("out".to_string()),
                    text_eval: hs("A".to_string()),
                }))),
            ),
            (
                "w2",
                f_task(Some(PmtcFA::FileAppend(PmtcFFileWrite {
                    file_eval: hs("out".to_string()),
                    text_eval: hs("B".to_string()),
                }))),
            ),
        ],
    );
    let mut ce = MemEnv::new();
    let env = PmtcEnv::new();

    let o = ops(plan(&vec!["a".to_string(), "b".to_string()], &c, &env, &mut ce).unwrap());
    assert_eq!(o.len(), 2);
    assert!(matches!(&o[0], PmtcRunOp::FileAppend((p, t)) if p == "out" && t == "A"));
    assert!(matches!(&o[1], PmtcRunOp::FileAppend((p, t)) if p == "out" && t == "B"));
}

#[test]
fn plan_exec_sh() {
    let c = conf(
        vec![("a", vec![task(&[("X", "hi")], None, Some("f"), &[])])],
        vec![(
            "f",
            f_task(Some(PmtcFA::ExecSh(hs("echo [X]".to_string())))),
        )],
    );
    let mut ce = MemEnv::new();
    let env = PmtcEnv::new();

    let o = ops(plan(&vec!["a".to_string()], &c, &env, &mut ce).unwrap());
    assert!(matches!(&o[0], PmtcRunOp::ExecSh(s) if s == "echo hi"));
}

#[test]
fn plan_mkdir_home() {
    let c = conf(
        vec![("a", vec![task(&[], None, Some("f"), &[])])],
        vec![("f", f_task(Some(PmtcFA::Mkdir(hs("~/d".to_string())))))],
    );
    let mut ce = MemEnv::new();
    let mut env = PmtcEnv::new();
    env.set("HOME", "/h");

    let o = ops(plan(&vec!["a".to_string()], &c, &env, &mut ce).unwrap());
    assert!(matches!(&o[0], PmtcRunOp::Mkdir(p) if p == "/h/d"));
}

#[test]
fn plan_chmod_paths() {
    let c = conf(
        vec![("a", vec![task(&[], None, Some("f"), &[])])],
        vec![(
            "f",
            f_task(Some(PmtcFA::Chmod(PmtcFChmod {
                mode: hs(0o700u32),
                path_eval: vec![hs("a".to_string()), hs("b".to_string())],
            }))),
        )],
    );
    let mut ce = MemEnv::new();
    let env = PmtcEnv::new();

    let o = ops(plan(&vec!["a".to_string()], &c, &env, &mut ce).unwrap());
    assert_eq!(o.len(), 2);
    assert!(matches!(&o[0], PmtcRunOp::Chmod((p, m)) if p == "a" && *m == 0o700));
    assert!(matches!(&o[1], PmtcRunOp::Chmod((p, m)) if p == "b" && *m == 0o700));
}

#[test]
fn plan_skip_file() {
    let c = conf(
        vec![("a", vec![task(&[], None, Some("f"), &[])])],
        vec![(
            "f",
            f_full(
                Some(PmtcFA::Exec(hs("true".to_string()))),
                false,
                Some("exists"),
                None,
                &[],
            ),
        )],
    );
    let mut ce = MemEnv::new();
    ce.exist.insert("exists".to_string(), true);
    let env = PmtcEnv::new();

    let o = ops(plan(&vec!["a".to_string()], &c, &env, &mut ce).unwrap());
    assert_eq!(o.len(), 0);
}

#[test]
fn plan_skip_include() {
    let w = || {
        PmtcFA::FileAppend(PmtcFFileWrite {
            file_eval: hs("out".to_string()),
            text_eval: hs("x".to_string()),
        })
    };
    let c = conf(
        vec![("a", vec![task(&[], None, Some("f"), &[])])],
        vec![("f", f_full(Some(w()), false, None, Some("needle"), &[]))],
    );

    // 文件内容 包含 needle -> 跳过
    let mut ce = MemEnv::new();
    ce.files
        .insert("out".to_string(), "contains needle here".to_string());
    let env = PmtcEnv::new();
    let o = ops(plan(&vec!["a".to_string()], &c, &env, &mut ce).unwrap());
    assert_eq!(o.len(), 0);

    // 文件内容 不包含 needle -> 执行
    let mut ce = MemEnv::new();
    ce.files.insert("out".to_string(), "other".to_string());
    let o = ops(plan(&vec!["a".to_string()], &c, &env, &mut ce).unwrap());
    assert_eq!(o.len(), 1);
}

#[test]
fn plan_once_dedup() {
    let c = conf(
        vec![
            ("a", vec![task(&[], None, Some("g"), &[])]),
            ("b", vec![task(&[], None, Some("g"), &[])]),
        ],
        vec![(
            "g",
            f_full(
                Some(PmtcFA::Echo(hs("X".to_string()))),
                true,
                None,
                None,
                &[],
            ),
        )],
    );
    let mut ce = MemEnv::new();
    let env = PmtcEnv::new();

    let o = ops(plan(&vec!["a".to_string(), "b".to_string()], &c, &env, &mut ce).unwrap());
    assert_eq!(o.len(), 1);
    assert_eq!(echo_text(&o[0]), "X");
}

#[test]
fn plan_f_dep_order() {
    let c = conf(
        vec![("t", vec![task(&[], None, Some("a"), &[])])],
        vec![
            (
                "a",
                f_full(
                    Some(PmtcFA::Echo(hs("A".to_string()))),
                    false,
                    None,
                    None,
                    &["b"],
                ),
            ),
            ("b", f_task(Some(PmtcFA::Echo(hs("B".to_string()))))),
        ],
    );
    let mut ce = MemEnv::new();
    let env = PmtcEnv::new();

    let o = ops(plan(&vec!["t".to_string()], &c, &env, &mut ce).unwrap());
    assert_eq!(o.len(), 2);
    assert_eq!(echo_text(&o[0]), "B");
    assert_eq!(echo_text(&o[1]), "A");
}

#[test]
fn plan_f_dep_once() {
    let c = conf(
        vec![
            ("t1", vec![task(&[], None, Some("a"), &[])]),
            ("t2", vec![task(&[], None, Some("b"), &[])]),
        ],
        vec![
            (
                "g",
                f_full(
                    Some(PmtcFA::Echo(hs("G".to_string()))),
                    true,
                    None,
                    None,
                    &[],
                ),
            ),
            (
                "a",
                f_full(
                    Some(PmtcFA::Echo(hs("A".to_string()))),
                    false,
                    None,
                    None,
                    &["g"],
                ),
            ),
            (
                "b",
                f_full(
                    Some(PmtcFA::Echo(hs("B".to_string()))),
                    false,
                    None,
                    None,
                    &["g"],
                ),
            ),
        ],
    );
    let mut ce = MemEnv::new();
    let env = PmtcEnv::new();

    let o = ops(plan(&vec!["t1".to_string(), "t2".to_string()], &c, &env, &mut ce).unwrap());
    assert_eq!(o.len(), 3);
    assert_eq!(echo_text(&o[0]), "G");
    assert_eq!(echo_text(&o[1]), "A");
    assert_eq!(echo_text(&o[2]), "B");
}

// ===== 错误情况 (只判断 出错) =====

#[test]
fn err_file_merge_append_conflict() {
    let c = conf(
        vec![
            ("a", vec![task(&[], None, Some("m"), &[])]),
            ("b", vec![task(&[], None, Some("w"), &[])]),
        ],
        vec![
            (
                "m",
                f_task(Some(PmtcFA::FileMerge(PmtcFFileWrite {
                    file_eval: hs("out".to_string()),
                    text_eval: hs("A".to_string()),
                }))),
            ),
            (
                "w",
                f_task(Some(PmtcFA::FileAppend(PmtcFFileWrite {
                    file_eval: hs("out".to_string()),
                    text_eval: hs("B".to_string()),
                }))),
            ),
        ],
    );
    let mut ce = MemEnv::new();
    let env = PmtcEnv::new();
    assert!(plan(&vec!["a".to_string(), "b".to_string()], &c, &env, &mut ce).is_err());
}

#[test]
fn err_file_replace_multi() {
    let c = conf(
        vec![
            ("a", vec![task(&[], None, Some("w1"), &[])]),
            ("b", vec![task(&[], None, Some("w2"), &[])]),
        ],
        vec![
            (
                "w1",
                f_task(Some(PmtcFA::File(PmtcFFileWrite {
                    file_eval: hs("out".to_string()),
                    text_eval: hs("A".to_string()),
                }))),
            ),
            (
                "w2",
                f_task(Some(PmtcFA::File(PmtcFFileWrite {
                    file_eval: hs("out".to_string()),
                    text_eval: hs("B".to_string()),
                }))),
            ),
        ],
    );
    let mut ce = MemEnv::new();
    let env = PmtcEnv::new();
    assert!(plan(&vec!["a".to_string(), "b".to_string()], &c, &env, &mut ce).is_err());
}

#[test]
fn err_dep_cycle() {
    let c = conf(
        vec![
            ("a", vec![task(&[], None, None, &["b"])]),
            ("b", vec![task(&[], None, None, &["a"])]),
        ],
        vec![],
    );
    let mut ce = MemEnv::new();
    let env = PmtcEnv::new();
    assert!(plan(&vec!["a".to_string()], &c, &env, &mut ce).is_err());
}

#[test]
fn err_env_for_cycle() {
    let c = conf(
        vec![
            ("a", vec![task(&[], Some("b"), None, &[])]),
            ("b", vec![task(&[], Some("a"), None, &[])]),
        ],
        vec![],
    );
    let mut ce = MemEnv::new();
    let env = PmtcEnv::new();
    assert!(plan(&vec!["a".to_string()], &c, &env, &mut ce).is_err());
}

#[test]
fn err_dep_missing() {
    let c = conf(
        vec![("a", vec![task(&[], None, None, &["missing"])])],
        vec![],
    );
    let mut ce = MemEnv::new();
    let env = PmtcEnv::new();
    assert!(plan(&vec!["a".to_string()], &c, &env, &mut ce).is_err());
}

#[test]
fn err_f_missing() {
    let c = conf(
        vec![("a", vec![task(&[], None, Some("missing"), &[])])],
        vec![],
    );
    let mut ce = MemEnv::new();
    let env = PmtcEnv::new();
    assert!(plan(&vec!["a".to_string()], &c, &env, &mut ce).is_err());
}

#[test]
fn err_arg_missing() {
    let c = conf(
        vec![("a", vec![task_arg(&[], None, Some("f"), &[], 1)])],
        vec![("f", f_task(Some(PmtcFA::Echo(hs("[@1]".to_string())))))],
    );
    let mut ce = MemEnv::new();
    let env = PmtcEnv::new();
    assert!(plan(&vec!["a".to_string()], &c, &env, &mut ce).is_err());
}

#[test]
fn err_skip_include_no_file() {
    // skip_include_eval 必需有 file_eval (file 系列动作).
    let c = conf(
        vec![("a", vec![task(&[], None, Some("f"), &[])])],
        vec![(
            "f",
            f_full(
                Some(PmtcFA::Exec(hs("true".to_string()))),
                false,
                None,
                Some("x"),
                &[],
            ),
        )],
    );
    let mut ce = MemEnv::new();
    let env = PmtcEnv::new();
    assert!(plan(&vec!["a".to_string()], &c, &env, &mut ce).is_err());
}

#[test]
fn err_f_dep_missing() {
    let c = conf(
        vec![("t", vec![task(&[], None, Some("a"), &[])])],
        vec![("a", f_full(None, false, None, None, &["missing"]))],
    );
    let mut ce = MemEnv::new();
    let env = PmtcEnv::new();
    assert!(plan(&vec!["t".to_string()], &c, &env, &mut ce).is_err());
}

#[test]
fn err_f_dep_cycle() {
    let c = conf(
        vec![("t", vec![task(&[], None, Some("a"), &[])])],
        vec![
            ("a", f_full(None, false, None, None, &["b"])),
            ("b", f_full(None, false, None, None, &["a"])),
        ],
    );
    let mut ce = MemEnv::new();
    let env = PmtcEnv::new();
    assert!(plan(&vec!["t".to_string()], &c, &env, &mut ce).is_err());
}
