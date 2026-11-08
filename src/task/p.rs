use std::{
    collections::{HashMap, HashSet},
    error::Error,
    path::{Path, PathBuf},
};

use crate::{
    check::{MissingRef, check_dep_cycle, dep_edges, norm_path},
    conf::{HasP, HasPD, PmtcConf, PmtcF, PmtcFA, PmtcTask, SrcP},
    te::{PmtcEnv, TeError, conf_env, end, eval_use, exec_eval_use, get_home, path_home},
};

use super::{CheckEnv, PlanDebug, PmtcRunList, PmtcRunOp, TaskDebug, TaskError};

/// 计划执行.
///
/// - `task`: 命令行参数表. 依次是 task 名称 (`[[t.NAME]]`); 遇到 `arg` 类型的 task 时,
///   其后面的 `n` 个参数 不再是 task 名称, 而是该 task 的参数 (`@1`..`@n`).
///   同名 task 有多个实例时, 依次取多次.
/// - `env`: 是 conf 的 `[env]` 执行之前的 变量 (环境变量).
pub fn plan<C: CheckEnv>(
    task: &Vec<String>,
    conf: &PmtcConf,
    env: &PmtcEnv,
    c: &mut C,
) -> Result<(PmtcRunList, PlanDebug), Box<dyn Error>> {
    // 先把 所有 `[env]` 执行一遍, 结果作为 只读基础环境.
    // 每执行一个 task, 再 clone 一份, 加入本 task 的变量.
    let (env0, u) = conf_env(env, conf.env.clone(), conf.eval_rec).map_err(|e| {
        Box::new(TaskError::Te(HasPD::new(e.src(), e.as_ref().clone()))) as Box<dyn Error>
    })?;

    // 依赖/引用 检查 (存在性 + 无环).
    validate(conf)?;

    let invokes = parse_invokes(task, conf)?;

    // 用于调试
    let pd = PlanDebug {
        env0,
        u,
        task: Vec::new(),
    };

    let mut p = Planner {
        conf,
        pd,
        c,
        once: HashSet::new(),
        raw: Vec::new(),
    };

    for (name, idx, args) in invokes {
        p.expand_instance(&name, idx, &args)?;
    }

    let (r, pd) = p.finish()?;
    Ok((r, pd))
}

/// 解析 命令行参数表.
///
/// 返回 `(task 名称, 实例下标, 该实例的参数)` 列表, 按执行顺序.
fn parse_invokes(
    task: &Vec<String>,
    conf: &PmtcConf,
) -> Result<Vec<(String, usize, Vec<String>)>, Box<dyn Error>> {
    let mut out = Vec::new();
    let mut i = 0;

    while i < task.len() {
        let name = task[i].clone();
        let instances = conf.task.get(name.as_str()).ok_or_else(|| {
            Box::new(TaskError::DepT((
                HasPD::new(SrcP::new(0, 0), format!("task `{}` does not exist", name)),
                HashMap::new(),
            ))) as Box<dyn Error>
        })?;

        let total: usize = instances.iter().map(|ht| *ht.as_ref().arg.as_ref()).sum();

        // 1 个名称 + total 个参数
        if i + 1 + total > task.len() {
            return Err(Box::new(TaskError::Arg((
                HasPD::new(
                    SrcP::new(0, 0),
                    format!("task `{}` needs {} arg(s)", name, total),
                ),
                HashMap::new(),
            ))));
        }

        let mut pos = i + 1;
        for (idx, ht) in instances.iter().enumerate() {
            let n = *ht.as_ref().arg.as_ref();
            let args = task[pos..pos + n].to_vec();
            pos += n;
            out.push((name.clone(), idx, args));
        }

        i = pos;
    }

    Ok(out)
}

/// 计划执行的 内部动作.
///
/// 文件写入 单独保存 (带 源位置), 便于后续做 冲突检查 / `file_merge` 合并.
enum RawOp {
    /// 非文件 动作
    Run(PmtcRunOp),
    /// `file` (file_replace)
    Replace {
        src: SrcP,
        path: String,
        text: String,
    },
    /// `file_append`
    Append {
        src: SrcP,
        path: String,
        text: String,
    },
    /// `file_merge`
    Merge {
        src: SrcP,
        path: String,
        text: String,
    },
}

struct Planner<'a, C: CheckEnv> {
    conf: &'a PmtcConf,
    /// 用于调试: 基础环境 + 每个任务的最终变量
    pd: PlanDebug,
    c: &'a mut C,
    /// 已经处理过的 `once` f (按名称)
    once: HashSet<String>,
    raw: Vec<RawOp>,
}

impl<C: CheckEnv> Planner<'_, C> {
    fn rec(&self) -> u16 {
        self.conf.eval_rec
    }

    /// 依次执行 同名 task 的 所有实例 (用于 依赖, 无 命令行参数).
    fn expand_name(&mut self, name: &str) -> Result<(), Box<dyn Error>> {
        let tasks = self.conf.task.get(name).ok_or_else(|| {
            Box::new(TaskError::DepT((
                HasPD::new(SrcP::new(0, 0), format!("task `{}` does not exist", name)),
                HashMap::new(),
            ))) as Box<dyn Error>
        })?;

        let n = tasks.len();
        for idx in 0..n {
            self.expand_instance(name, idx, &[])?;
        }
        Ok(())
    }

    /// 执行 单个 task 实例: 先 依赖, 后 自身.
    fn expand_instance(
        &mut self,
        name: &str,
        idx: usize,
        args: &[String],
    ) -> Result<(), Box<dyn Error>> {
        let deps: Vec<String> = self.conf.task[name][idx]
            .as_ref()
            .dep
            .iter()
            .map(|h| h.as_ref().clone())
            .collect();

        for d in &deps {
            self.expand_name(d)?;
        }

        self.emit_task(name, idx, args)
    }

    /// 执行 单个 task 实例 的 自身动作 (不含 依赖).
    fn emit_task(&mut self, name: &str, idx: usize, args: &[String]) -> Result<(), Box<dyn Error>> {
        let t = self.conf.task[name][idx].as_ref().clone();

        match &t.env_for {
            Some(h) => {
                // env_for: 取 指向 task 的 变量结果 (不执行对应 task),
                // 再加上 本 task 的 `@n` 参数 和 `env:XXX` 变量.
                // 对应 task 有多个实例时, 按顺序执行多次.
                let target = h.as_ref().clone();
                let n = self.conf.task[target.as_str()].len();
                for j in 0..n {
                    let (mut env, mut env_use, mut u) = self.vars_of(&target, j, &[])?;
                    inject_args(&mut env, args);
                    self.eval_own(&t, &mut env, &mut u)?;
                    self.emit_f(&t, &mut env, &mut u)?;
                    env_use.extend(env.use_get());
                    self.pd.task.push(TaskDebug {
                        name: name.to_string(),
                        env: env.to_map_sort(),
                        env_use,
                        u,
                    });
                }
            }
            None => {
                let (mut env, mut env_use, mut u) = self.vars_of(name, idx, args)?;
                self.emit_f(&t, &mut env, &mut u)?;
                env_use.extend(env.use_get());
                self.pd.task.push(TaskDebug {
                    name: name.to_string(),
                    env: env.to_map_sort(),
                    env_use,
                    u,
                });
            }
        }

        Ok(())
    }

    /// 计算 某个 task 实例的 变量结果 (不执行其 依赖/f).
    ///
    /// = 基础环境 + (env_for 链 结果) + `@n` 参数 + 自身 `env:XXX` (eval).
    ///
    /// 返回 `(env, env_use, u)`:
    /// - `env`: 变量结果. `use` 标记 已清空 (供 调用者 继续标记 f 动作).
    /// - `env_use`: 计算变量 过程中 使用的变量 (`use`), 含 env_for 链.
    /// - `u`: 计算变量 过程中 出现的 未知变量, 含 env_for 链.
    fn vars_of(
        &self,
        name: &str,
        idx: usize,
        args: &[String],
    ) -> Result<(PmtcEnv, HashSet<(String, u8)>, HashSet<String>), TaskError> {
        let t = self.conf.task[name][idx].as_ref().clone();
        let mut env = self.pd.env0.clone();
        // 只统计 本 task 的 use 标记 (不包含 初始化 `[env]` 阶段).
        env.use_clear();

        let mut env_use: HashSet<(String, u8)> = HashSet::new();
        let mut u: HashSet<String> = HashSet::new();

        if let Some(h) = &t.env_for {
            let target = h.as_ref().clone();
            let n = self.conf.task[target.as_str()].len();
            for j in 0..n {
                let (tv, tv_use, tv_u) = self.vars_of(&target, j, &[])?;
                env.add(tv);
                env_use.extend(tv_use);
                u.extend(tv_u);
            }
        }

        inject_args(&mut env, args);
        self.eval_own(&t, &mut env, &mut u)?;

        env_use.extend(env.use_get());
        // 清空 use 标记, 由 调用者 (f 动作 求值) 重新标记.
        env.use_clear();
        Ok((env, env_use, u))
    }

    /// eval 本 task 的 `env:NAME` 变量, 并 写入 env.
    ///
    /// 所有 `env:NAME` 都 先 基于 同一个 (尚未加入 本 task 变量 的) env 求值,
    /// 再统一写入, 避免 HashMap 无序 导致 求值顺序 不确定.
    fn eval_own(
        &self,
        t: &PmtcTask,
        env: &mut PmtcEnv,
        u: &mut HashSet<String>,
    ) -> Result<(), TaskError> {
        let mut vals: Vec<(String, String)> = Vec::new();
        for (k, v) in &t.env {
            let val = eval_use(v.as_ref(), env, self.rec(), u).map_err(|e| te_err(v, e))?;
            vals.push((k.clone(), val));
        }
        for (k, val) in vals {
            env.set(&k, &val);
        }
        Ok(())
    }

    /// 执行 本 task 对应的 `[[f]]`.
    fn emit_f(
        &mut self,
        t: &PmtcTask,
        env: &mut PmtcEnv,
        u: &mut HashSet<String>,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(hf) = &t.f {
            self.emit_f_by_name(hf.as_ref(), env, u, hf.src())?;
        }
        Ok(())
    }

    /// 按名称执行一个 `[[f]]` (先 执行 其 f 依赖, 再 执行 自身动作).
    fn emit_f_by_name(
        &mut self,
        name: &str,
        env: &mut PmtcEnv,
        u: &mut HashSet<String>,
        src: SrcP,
    ) -> Result<(), Box<dyn Error>> {
        let pf = self.conf.f.get(name).cloned().ok_or_else(|| {
            Box::new(TaskError::F((
                HasPD::new(src, format!("missing f `{}`", name)),
                HashMap::new(),
            ))) as Box<dyn Error>
        })?;
        let f = pf.as_ref();

        // `once`: 整个 plan 中 只执行一次.
        if f.once {
            if !self.once.insert(name.to_string()) {
                return Ok(());
            }
        }

        // skip 判断 先于 依赖, 保证 被 skip 的 f 不会 执行其依赖.
        if self.skip_f(f, env, u)? {
            return Ok(());
        }

        // f 依赖 先于 自身执行 (使用 相同的 task 环境).
        for h in &f.dep {
            self.emit_f_by_name(h.as_ref(), env, u, h.src())?;
        }

        self.emit_f_action(f, env, u)
    }

    /// skip 条件: 在 plan 阶段 读取 环境现状 判断.
    ///
    /// 如果 本 f 被 skip, 则 本 f 以及 其 依赖 都不执行.
    fn skip_f(
        &mut self,
        f: &PmtcF,
        env: &mut PmtcEnv,
        u: &mut HashSet<String>,
    ) -> Result<bool, Box<dyn Error>> {
        if self.skip_file(f, env, u)? {
            return Ok(true);
        }
        let Some(ha) = &f.a else { return Ok(false) };
        let a = ha.as_ref();
        if self.skip_include(f, a, env, u)? {
            return Ok(true);
        }
        Ok(false)
    }

    /// 执行 `[[f]]` 的 自身动作 (不含 依赖/once/skip).
    fn emit_f_action(
        &mut self,
        f: &PmtcF,
        env: &mut PmtcEnv,
        u: &mut HashSet<String>,
    ) -> Result<(), Box<dyn Error>> {
        let Some(ha) = &f.a else { return Ok(()) };
        let a = ha.as_ref();

        let r = self.rec();
        match a {
            PmtcFA::Exec(h) => {
                let argv = eval_exec(h, env, r, u)?;
                self.raw.push(RawOp::Run(PmtcRunOp::Exec(argv)));
            }
            PmtcFA::ExecSh(h) => {
                let cmd = eval_text(h, env, r, u)?;
                self.raw.push(RawOp::Run(PmtcRunOp::ExecSh(cmd)));
            }
            PmtcFA::Echo(h) => {
                let text = eval_text(h, env, r, u)?;
                self.raw.push(RawOp::Run(PmtcRunOp::Echo(text)));
            }
            PmtcFA::Log(l) => {
                let mut argv = Vec::with_capacity(l.len());
                for h in l {
                    argv.push(eval_text(h, env, r, u)?);
                }
                self.raw.push(RawOp::Run(PmtcRunOp::Log(argv)));
            }
            PmtcFA::Cat(h) => {
                let path = eval_path(h, env, r, u)?;
                self.raw.push(RawOp::Run(PmtcRunOp::Cat(path)));
            }
            PmtcFA::Mkdir(h) => {
                let path = eval_path(h, env, r, u)?;
                self.raw.push(RawOp::Run(PmtcRunOp::Mkdir(path)));
            }
            PmtcFA::Chmod(c) => {
                let mode = *c.mode.as_ref();
                for h in &c.path_eval {
                    let path = eval_path(h, env, r, u)?;
                    self.raw.push(RawOp::Run(PmtcRunOp::Chmod((path, mode))));
                }
            }
            PmtcFA::File(w) => {
                let path = eval_path(&w.file_eval, env, r, u)?;
                let text = eval_text(&w.text_eval, env, r, u)?;
                self.raw.push(RawOp::Replace {
                    src: w.file_eval.src(),
                    path,
                    text,
                });
            }
            PmtcFA::FileAppend(w) => {
                let path = eval_path(&w.file_eval, env, r, u)?;
                let text = eval_text(&w.text_eval, env, r, u)?;
                self.raw.push(RawOp::Append {
                    src: w.file_eval.src(),
                    path,
                    text,
                });
            }
            PmtcFA::FileMerge(w) => {
                let path = eval_path(&w.file_eval, env, r, u)?;
                let text = eval_text(&w.text_eval, env, r, u)?;
                self.raw.push(RawOp::Merge {
                    src: w.file_eval.src(),
                    path,
                    text,
                });
            }
        }

        Ok(())
    }

    fn skip_file(
        &mut self,
        f: &PmtcF,
        env: &mut PmtcEnv,
        u: &mut HashSet<String>,
    ) -> Result<bool, Box<dyn Error>> {
        if let Some(h) = &f.skip_file_eval {
            let path = eval_path(h, env, self.rec(), u)?;
            if self.c.file_exist(&path)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn skip_include(
        &mut self,
        f: &PmtcF,
        a: &PmtcFA,
        env: &mut PmtcEnv,
        u: &mut HashSet<String>,
    ) -> Result<bool, Box<dyn Error>> {
        let Some(h) = &f.skip_include_eval else {
            return Ok(false);
        };

        // `skip_include_eval` 检查 写入目标文件 (file_eval) 的内容.
        let file = match a {
            PmtcFA::File(w) | PmtcFA::FileAppend(w) | PmtcFA::FileMerge(w) => &w.file_eval,
            _ => return Ok(false),
        };

        let needle = eval_text(h, env, self.rec(), u)?;
        let path = eval_path(file, env, self.rec(), u)?;

        // 文件不存在 (或读取失败) 视为 不跳过.
        match self.c.file_read(&path) {
            Ok(content) => Ok(content.contains(&needle)),
            Err(_) => Ok(false),
        }
    }

    /// 处理 文件写入 冲突, 并 把 `file_merge` 合并为 单个 `file_replace`.
    fn finish(self) -> Result<(PmtcRunList, PlanDebug), Box<dyn Error>> {
        let pd = self.pd;

        #[derive(Default)]
        struct PathInfo {
            /// 写入类型 位图: 1=replace, 2=append, 4=merge
            kinds: u8,
            replace: usize,
            merges: Vec<String>,
            src: Option<SrcP>,
        }

        const K_REPLACE: u8 = 1;
        const K_APPEND: u8 = 2;
        const K_MERGE: u8 = 4;

        let mut info: HashMap<PathBuf, PathInfo> = HashMap::new();
        let mut first_merge: HashMap<PathBuf, usize> = HashMap::new();

        for (i, op) in self.raw.iter().enumerate() {
            match op {
                RawOp::Run(_) => {}
                RawOp::Replace { src, path, .. } => {
                    let e = info.entry(norm_path(Path::new(path))).or_default();
                    e.kinds |= K_REPLACE;
                    e.replace += 1;
                    if e.src.is_none() {
                        e.src = Some(*src);
                    }
                }
                RawOp::Append { src, path, .. } => {
                    let e = info.entry(norm_path(Path::new(path))).or_default();
                    e.kinds |= K_APPEND;
                    if e.src.is_none() {
                        e.src = Some(*src);
                    }
                }
                RawOp::Merge { src, path, text } => {
                    let k = norm_path(Path::new(path));
                    let e = info.entry(k.clone()).or_default();
                    e.kinds |= K_MERGE;
                    e.merges.push(text.clone());
                    if e.src.is_none() {
                        e.src = Some(*src);
                    }
                    first_merge.entry(k).or_insert(i);
                }
            }
        }

        for (k, e) in &info {
            if e.replace > 1 {
                return Err(Box::new(TaskError::FileWrite((
                    HasPD::new(
                        e.src.unwrap_or_default(),
                        format!("multiple `file_replace` to `{}`", k.display()),
                    ),
                    HashMap::new(),
                ))));
            }
            if e.kinds.count_ones() > 1 {
                return Err(Box::new(TaskError::FileWrite((
                    HasPD::new(
                        e.src.unwrap_or_default(),
                        format!("conflicting file writes to `{}`", k.display()),
                    ),
                    HashMap::new(),
                ))));
            }
        }

        let mut out = PmtcRunList::new();
        for (i, op) in self.raw.into_iter().enumerate() {
            match op {
                RawOp::Run(r) => {
                    out.add(r);
                }
                RawOp::Replace { path, text, .. } => {
                    out.add(PmtcRunOp::FileReplace((path, text)));
                }
                RawOp::Append { path, text, .. } => {
                    out.add(PmtcRunOp::FileAppend((path, text)));
                }
                RawOp::Merge { path, .. } => {
                    let k = norm_path(Path::new(&path));
                    // 只在 第一个 file_merge 的位置 输出 合并后的 file_replace.
                    if first_merge.get(&k) == Some(&i) {
                        let joined = info[&k].merges.join("");
                        out.add(PmtcRunOp::FileReplace((path, joined)));
                    }
                }
            }
        }

        Ok((out, pd))
    }
}

/// 注入 `@1`..`@n` 参数变量.
fn inject_args(env: &mut PmtcEnv, args: &[String]) {
    for (i, a) in args.iter().enumerate() {
        env.set(&format!("@{}", i + 1), a);
    }
}

/// TeError -> TaskError (附带 源位置)
fn te_err(h: &HasP<String>, e: TeError) -> TaskError {
    TaskError::Te(HasPD::new(h.src(), e))
}

/// 对 路径模板 求值: eval -> end -> `~` 展开 (只考虑 Linux 系统)
fn eval_path(
    h: &HasP<String>,
    env: &mut PmtcEnv,
    r: u16,
    u: &mut HashSet<String>,
) -> Result<String, TaskError> {
    let v = eval_use(h.as_ref(), env, r, u).map_err(|e| te_err(h, e))?;
    let v = end(&v).map_err(|e| te_err(h, e))?;
    // 展开 `~`
    let home = get_home(env);
    Ok(path_home(v, &home))
}

/// 对 文本模板 求值: eval -> end.
fn eval_text(
    h: &HasP<String>,
    env: &mut PmtcEnv,
    r: u16,
    u: &mut HashSet<String>,
) -> Result<String, TaskError> {
    let v = eval_use(h.as_ref(), env, r, u).map_err(|e| te_err(h, e))?;
    end(&v).map_err(|e| te_err(h, e))
}

/// `exec` 命令 求值 (split + `~` 展开).
fn eval_exec(
    h: &HasP<String>,
    env: &mut PmtcEnv,
    r: u16,
    u: &mut HashSet<String>,
) -> Result<Vec<String>, TaskError> {
    exec_eval_use(h.as_ref(), env, r, u).map_err(|e| te_err(h, e))
}

/// 检查 task/f 的 依赖/引用:
/// (1) 被引用/依赖 的 task/f 必须存在.
/// (2) 不允许 循环依赖 (`dep` / `env_for`).
fn validate(conf: &PmtcConf) -> Result<(), Box<dyn Error>> {
    let (task_edges, f_edges) = dep_edges(&conf.task, &conf.f).map_err(|m| {
        let e = match &m {
            MissingRef::TaskEnvFor { .. } | MissingRef::TaskDep { .. } => {
                TaskError::DepT((HasPD::new(m.src(), m.why()), m.params()))
            }
            MissingRef::TaskF { .. } | MissingRef::FDep { .. } => {
                TaskError::F((HasPD::new(m.src(), m.why()), m.params()))
            }
        };
        Box::new(e) as Box<dyn Error>
    })?;

    // `skip_include_eval` 必需有对应的 `file_eval` (file 系列动作).
    for (name, hf) in &conf.f {
        let f = hf.as_ref();
        if f.skip_include_eval.is_some() {
            let has_file = match &f.a {
                Some(ha) => matches!(
                    ha.as_ref(),
                    PmtcFA::File(_) | PmtcFA::FileAppend(_) | PmtcFA::FileMerge(_)
                ),
                None => false,
            };
            if !has_file {
                let src = f
                    .skip_include_eval
                    .as_ref()
                    .map(|h| h.src())
                    .unwrap_or_default();
                return Err(Box::new(TaskError::FDef((
                    HasPD::new(
                        src,
                        format!("f `{}` `skip_include_eval` requires `file_eval`", name),
                    ),
                    HashMap::new(),
                ))));
            }
        }
    }

    check_dep_cycle(&task_edges).map_err(|c| {
        Box::new(TaskError::DepCycle((
            HasPD::new(c.src, c.why()),
            HashMap::new(),
        ))) as Box<dyn Error>
    })?;
    check_dep_cycle(&f_edges).map_err(|c| {
        Box::new(TaskError::DepCycle((
            HasPD::new(c.src, c.why()),
            HashMap::new(),
        ))) as Box<dyn Error>
    })?;
    Ok(())
}
