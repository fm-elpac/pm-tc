//! 带位置信息的 TOML 反序列化实现 (只实现 de 部分).
//!
//! `toml_edit` 内置的 serde 实现是基于 `&str` 的流式解析, 无法提供 源位置 (`span`).
//! 这里的实现直接 遍历 `toml_edit` 的 DOM (`DocumentMut`), 通过 serde 的
//! `Deserializer` 把数据反序列化成 带 `SrcP` 的 中间结构 (`P1` 等).
//!
//! 设计要点:
//! - 所有叶子值使用 [`HasP`] 包装, 以保留 值的 源位置.
//! - `HasP` 的 span 通过 `deserialize_newtype_struct` 的 标记名 ("HasP") 从
//!   线程本地栈 (`SPAN_STACK`) 中取得.
//! - map 的 key 名称 / key 位置 通过 `MapAccess` 压入 线程本地栈 (`KEY_NAME`,
//!   `KEY_SPAN`), 供 错误信息 和 需要 key 位置 的字段 (比如 `[env]`) 使用.
//! - 未知 key 由 `#[serde(deny_unknown_fields)]` 触发, 并在 [`ConfError`] 的
//!   `serde::de::Error` 实现中 带上 key 位置.
use std::{
    cell::RefCell,
    collections::HashMap,
    fmt::{self, Debug, Display},
    marker::PhantomData,
};

use serde::{
    Deserialize,
    de::{self, DeserializeSeed, Deserializer, MapAccess, SeqAccess, Unexpected, Visitor},
};
use toml_edit::{Array, ArrayOfTables, DocumentMut, Item, Table, Value};

use super::{ConfError, HasP, SrcP};

// ===== 线程本地 位置/名称 栈 =====

thread_local! {
    /// 当前值 的 源位置 栈 (自 root 到 当前节点).
    static SPAN_STACK: RefCell<Vec<SrcP>> = RefCell::new(Vec::new());

    /// 当前 map 的 key 名称 栈.
    static KEY_NAME: RefCell<Vec<String>> = RefCell::new(Vec::new());

    /// 当前 map 的 key 位置 栈.
    static KEY_SPAN: RefCell<Vec<SrcP>> = RefCell::new(Vec::new());
}

/// 压入/弹出 值位置 的 作用域保护.
struct SpanGuard;

impl SpanGuard {
    fn push(src: SrcP) -> Self {
        SPAN_STACK.with(|v| v.borrow_mut().push(src));
        SpanGuard
    }
}

impl Drop for SpanGuard {
    fn drop(&mut self) {
        SPAN_STACK.with(|v| {
            v.borrow_mut().pop();
        });
    }
}

/// 压入/弹出 key (名称 + 位置) 的 作用域保护.
struct KeyGuard;

impl KeyGuard {
    fn push(name: String, src: SrcP) -> Self {
        KEY_NAME.with(|v| v.borrow_mut().push(name));
        KEY_SPAN.with(|v| v.borrow_mut().push(src));
        KeyGuard
    }
}

impl Drop for KeyGuard {
    fn drop(&mut self) {
        KEY_NAME.with(|v| {
            v.borrow_mut().pop();
        });
        KEY_SPAN.with(|v| {
            v.borrow_mut().pop();
        });
    }
}

/// 当前 值 的 源位置.
fn cur_span() -> SrcP {
    SPAN_STACK.with(|v| v.borrow().last().copied().unwrap_or_default())
}

/// 当前 key 名称.
fn cur_key() -> String {
    KEY_NAME.with(|v| v.borrow().last().cloned().unwrap_or_default())
}

/// 当前 key 位置.
fn cur_key_span() -> SrcP {
    KEY_SPAN.with(|v| v.borrow().last().copied().unwrap_or_default())
}

/// 如果存在 key 名称, 给错误信息加上 `key` 前缀.
fn keyed(msg: impl Display) -> String {
    let k = cur_key();
    if k.is_empty() {
        msg.to_string()
    } else {
        format!("`{}` {}", k, msg)
    }
}

// ===== ConfError 实现 serde::de::Error =====

impl de::Error for ConfError {
    fn custom<T: Display>(msg: T) -> Self {
        ConfError::value(cur_span(), "custom", keyed(msg), &[])
    }

    fn invalid_type(_unexp: Unexpected, exp: &dyn de::Expected) -> Self {
        ConfError::value(cur_span(), "type", keyed(format!("expected {}", exp)), &[])
    }

    fn invalid_value(unexp: Unexpected, exp: &dyn de::Expected) -> Self {
        ConfError::value(
            cur_span(),
            "invalid",
            keyed(format!("invalid value: {} (expected {})", unexp, exp)),
            &[],
        )
    }

    fn invalid_length(len: usize, exp: &dyn de::Expected) -> Self {
        ConfError::value(
            cur_span(),
            "length",
            keyed(format!("invalid length: {} (expected {})", len, exp)),
            &[],
        )
    }

    fn unknown_field(field: &str, _expected: &'static [&'static str]) -> Self {
        ConfError::key(
            cur_key_span(),
            "unknown",
            format!("unknown toml key `{}`", field),
            &[("key", field.to_string())],
        )
    }

    fn missing_field(field: &'static str) -> Self {
        ConfError::value(cur_span(), "missing", format!("missing `{}`", field), &[])
    }

    fn duplicate_field(field: &'static str) -> Self {
        ConfError::key(
            cur_key_span(),
            "duplicate",
            format!("duplicate toml key `{}`", field),
            &[("key", field.to_string())],
        )
    }
}

// ===== HasP 的 Deserialize (通过 标记 newtype 取得 span) =====

impl<'de, T> Deserialize<'de> for HasP<T>
where
    T: Deserialize<'de> + Debug + Clone,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct V<T>(PhantomData<T>);

        impl<'de, T> Visitor<'de> for V<T>
        where
            T: Deserialize<'de> + Debug + Clone,
        {
            type Value = HasP<T>;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("HasP value")
            }

            fn visit_newtype_struct<D2>(self, d: D2) -> Result<Self::Value, D2::Error>
            where
                D2: Deserializer<'de>,
            {
                let v = T::deserialize(d)?;
                Ok(HasP::new(cur_span(), v))
            }
        }

        deserializer.deserialize_newtype_struct("HasP", V(PhantomData))
    }
}

// ===== 源位置 辅助函数 =====

/// 获取 `Item` 的 源位置 (尽力而为).
pub(super) fn item_src(item: &Item, file_id: usize) -> SrcP {
    SrcP::new(file_id, item.span().map(|s| s.start as isize).unwrap_or(0))
}

/// 获取 `Table` 中 key 的 源位置 (尽力而为).
pub(super) fn key_src(t: &Table, k: &str, file_id: usize) -> SrcP {
    let i = t
        .key(k)
        .and_then(|key| key.span())
        .map(|s| s.start as isize)
        .unwrap_or(0);
    SrcP::new(file_id, i)
}

// ===== 带位置的 Deserializer =====

#[derive(Clone, Copy)]
enum Node<'de> {
    Value(&'de Value),
    Table(&'de Table),
    Aot(&'de ArrayOfTables),
    None,
}

#[derive(Clone, Copy)]
struct D<'de> {
    node: Node<'de>,
    src: SrcP,
    file_id: usize,
}

impl<'de> D<'de> {
    fn from_item(item: &'de Item, file_id: usize) -> Self {
        let src = item_src(item, file_id);
        let node = match item {
            Item::None => Node::None,
            Item::Value(v) => Node::Value(v),
            Item::Table(t) => Node::Table(t),
            Item::ArrayOfTables(a) => Node::Aot(a),
        };
        Self { node, src, file_id }
    }

    fn from_value(v: &'de Value, file_id: usize) -> Self {
        let src = SrcP::new(file_id, v.span().map(|s| s.start as isize).unwrap_or(0));
        Self {
            node: Node::Value(v),
            src,
            file_id,
        }
    }

    fn from_table(t: &'de Table, file_id: usize) -> Self {
        let i = t
            .position()
            .or_else(|| t.span().map(|s| s.start as isize))
            .unwrap_or(0);
        Self {
            node: Node::Table(t),
            src: SrcP::new(file_id, i),
            file_id,
        }
    }
}

/// map key 的 反序列化 (仅字符串).
struct KeyD<'de>(&'de str);

impl<'de> Deserializer<'de> for KeyD<'de> {
    type Error = ConfError;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        visitor.visit_borrowed_str(self.0)
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}

struct MapAccessImpl<'de> {
    table: &'de Table,
    iter: Box<dyn Iterator<Item = (&'de str, &'de Item)> + 'de>,
    file_id: usize,
    active: Option<KeyGuard>,
    pending: Option<&'de Item>,
}

impl<'de> MapAccessImpl<'de> {
    fn new(table: &'de Table, file_id: usize) -> Self {
        Self {
            table,
            iter: Box::new(table.iter()),
            file_id,
            active: None,
            pending: None,
        }
    }
}

impl<'de> MapAccess<'de> for MapAccessImpl<'de> {
    type Error = ConfError;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: DeserializeSeed<'de>,
    {
        match self.iter.next() {
            None => Ok(None),
            Some((k, item)) => {
                let ks = key_src(self.table, k, self.file_id);
                self.active = Some(KeyGuard::push(k.to_string(), ks));
                self.pending = Some(item);
                seed.deserialize(KeyD(k)).map(Some)
            }
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: DeserializeSeed<'de>,
    {
        let item = self
            .pending
            .take()
            .expect("next_value_seed called before next_key_seed");
        let d = D::from_item(item, self.file_id);
        let r = seed.deserialize(d);
        self.active = None;
        r
    }
}

#[derive(Clone, Copy)]
enum SeqItems<'de> {
    Aot(&'de ArrayOfTables),
    Arr(&'de Array),
}

struct SeqAccessImpl<'de> {
    items: SeqItems<'de>,
    idx: usize,
    file_id: usize,
}

impl<'de> SeqAccessImpl<'de> {
    fn new(items: SeqItems<'de>, file_id: usize) -> Self {
        Self {
            items,
            idx: 0,
            file_id,
        }
    }
}

impl<'de> SeqAccess<'de> for SeqAccessImpl<'de> {
    type Error = ConfError;

    fn next_element_seed<V>(&mut self, seed: V) -> Result<Option<V::Value>, Self::Error>
    where
        V: DeserializeSeed<'de>,
    {
        let d = match self.items {
            SeqItems::Aot(a) => a.get(self.idx).map(|t| D::from_table(t, self.file_id)),
            SeqItems::Arr(a) => a.get(self.idx).map(|v| D::from_value(v, self.file_id)),
        };
        match d {
            Some(d) => {
                self.idx += 1;
                seed.deserialize(d).map(Some)
            }
            None => Ok(None),
        }
    }
}

impl<'de> Deserializer<'de> for D<'de> {
    type Error = ConfError;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        let _g = SpanGuard::push(self.src);
        match self.node {
            Node::Value(v) => match v {
                Value::String(s) => visitor.visit_borrowed_str(s.value()),
                Value::Integer(i) => visitor.visit_i64(*i.value()),
                Value::Float(f) => visitor.visit_f64(*f.value()),
                Value::Boolean(b) => visitor.visit_bool(*b.value()),
                Value::Array(a) => {
                    visitor.visit_seq(SeqAccessImpl::new(SeqItems::Arr(a), self.file_id))
                }
                Value::Datetime(_) | Value::InlineTable(_) => Err(ConfError::value(
                    self.src,
                    "unsupported",
                    "unsupported type",
                    &[],
                )),
            },
            Node::Table(t) => visitor.visit_map(MapAccessImpl::new(t, self.file_id)),
            Node::Aot(a) => visitor.visit_seq(SeqAccessImpl::new(SeqItems::Aot(a), self.file_id)),
            Node::None => Err(ConfError::value(self.src, "value", "expected a value", &[])),
        }
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        let _g = SpanGuard::push(self.src);
        match self.node {
            Node::Value(Value::String(s)) => visitor.visit_borrowed_str(s.value()),
            _ => Err(ConfError::value(
                self.src,
                "str",
                keyed("expected a string"),
                &[],
            )),
        }
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        let _g = SpanGuard::push(self.src);
        match self.node {
            Node::Value(Value::String(s)) => visitor.visit_borrowed_str(s.value()),
            _ => Err(ConfError::value(
                self.src,
                "str",
                keyed("expected a string"),
                &[],
            )),
        }
    }

    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        let _g = SpanGuard::push(self.src);
        match self.node {
            Node::Value(Value::Integer(i)) => visitor.visit_i64(*i.value()),
            _ => Err(ConfError::value(
                self.src,
                "int",
                keyed("expected an integer"),
                &[],
            )),
        }
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_some(self)
    }

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        let _g = SpanGuard::push(self.src);
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        let _g = SpanGuard::push(self.src);
        match self.node {
            Node::Value(Value::Array(a)) => {
                visitor.visit_seq(SeqAccessImpl::new(SeqItems::Arr(a), self.file_id))
            }
            Node::Aot(a) => visitor.visit_seq(SeqAccessImpl::new(SeqItems::Aot(a), self.file_id)),
            _ => Err(ConfError::value(
                self.src,
                "array",
                keyed("expected an array"),
                &[],
            )),
        }
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        let _g = SpanGuard::push(self.src);
        match self.node {
            Node::Table(t) => visitor.visit_map(MapAccessImpl::new(t, self.file_id)),
            _ => Err(ConfError::value(
                self.src,
                "table",
                keyed("expected a table"),
                &[],
            )),
        }
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_map(visitor)
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_unit()
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i128 u8 u16 u32 u64 u128 f32 f64 char bytes byte_buf
        unit unit_struct tuple tuple_struct enum identifier
    }
}

// ===== 中间数据结构 =====

/// 解析 阶段 1 (结构) 结束时的 中间结构.
///
/// 字段类型已经过 serde 反序列化, 但 尚未做 语义检查 (`check_name`, `check_t`,
/// 取值范围, 动作分派 等).
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub(super) struct P1 {
    #[allow(dead_code)]
    pub(super) pm_tc: i64,
    pub(super) eval_rec: Option<HasP<i64>>,
    #[serde(default)]
    pub(super) include: Vec<Include1>,
    #[serde(default)]
    pub(super) env: Env1,
    #[serde(default)]
    pub(super) t: HashMap<String, Vec<HasP<Task1>>>,
    #[serde(default)]
    pub(super) f: Vec<HasP<F1>>,
}

/// `[[include]]` 中间结构.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub(super) struct Include1 {
    pub(super) file: Option<HasP<String>>,
}

/// `[env]` 中间结构 (有序, 保留 key/value 位置).
#[derive(Debug, Clone, Default)]
pub(super) struct Env1 {
    /// (name, key 位置, value)
    pub(super) list: Vec<(String, SrcP, HasP<String>)>,
}

impl<'de> Deserialize<'de> for Env1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct V;

        impl<'de> Visitor<'de> for V {
            type Value = Env1;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an [env] table")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut list = Vec::new();
                while let Some(k) = map.next_key::<String>()? {
                    let ks = cur_key_span();
                    let value = map.next_value::<HasP<String>>()?;
                    list.push((k, ks, value));
                }
                Ok(Env1 { list })
            }
        }

        deserializer.deserialize_map(V)
    }
}

/// `[[t.*]]` 中间结构.
///
/// 与其它中间结构不同, 这里 手动实现 `Deserialize`, 因为 `env:NAME` 是
/// 动态 key (前缀 `env:`), 需要 区分 已知 key 和 未知 key.
#[derive(Debug, Clone)]
pub(super) struct Task1 {
    pub(super) env_for: Option<HasP<String>>,
    pub(super) f: Option<HasP<String>>,
    pub(super) dep: Vec<HasP<String>>,
    pub(super) arg: Option<HasP<i64>>,
    /// `env:NAME` -> (key 位置, value)
    pub(super) env: HashMap<String, (SrcP, HasP<String>)>,
}

impl<'de> Deserialize<'de> for Task1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct V;

        impl<'de> Visitor<'de> for V {
            type Value = Task1;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a [[t.*]] table")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut o = Task1 {
                    env_for: None,
                    f: None,
                    dep: Vec::new(),
                    arg: None,
                    env: HashMap::new(),
                };
                while let Some(k) = map.next_key::<String>()? {
                    let ks = cur_key_span();
                    match k.strip_prefix("env:") {
                        Some(name) => {
                            let value = map.next_value::<HasP<String>>()?;
                            o.env.insert(name.to_string(), (ks, value));
                        }
                        None => match k.as_str() {
                            "env_for" => o.env_for = Some(map.next_value()?),
                            "f" => o.f = Some(map.next_value()?),
                            "dep" => o.dep = map.next_value()?,
                            "arg" => o.arg = Some(map.next_value()?),
                            _ => {
                                return Err(<A::Error as de::Error>::unknown_field(&k, &[]));
                            }
                        },
                    }
                }
                Ok(o)
            }
        }

        deserializer.deserialize_map(V)
    }
}

/// `path_eval` 可以是 字符串 或 字符串数组.
#[derive(Debug, Clone)]
pub(super) enum PathEval1 {
    S(HasP<String>),
    A(Vec<HasP<String>>),
}

impl<'de> Deserialize<'de> for PathEval1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct V;

        impl<'de> Visitor<'de> for V {
            type Value = PathEval1;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a string or string array")
            }

            fn visit_borrowed_str<E>(self, s: &'de str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(PathEval1::S(HasP::new(cur_span(), s.to_string())))
            }

            fn visit_string<E>(self, s: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(PathEval1::S(HasP::new(cur_span(), s)))
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut v = Vec::new();
                while let Some(e) = seq.next_element::<HasP<String>>()? {
                    v.push(e);
                }
                Ok(PathEval1::A(v))
            }
        }

        deserializer.deserialize_any(V)
    }
}

/// `[[f]]` 中间结构.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub(super) struct F1 {
    pub(super) name: Option<HasP<String>>,
    pub(super) once: Option<HasP<i64>>,
    pub(super) skip_file_eval: Option<HasP<String>>,
    pub(super) skip_include_eval: Option<HasP<String>>,
    #[serde(default)]
    pub(super) dep: Vec<HasP<String>>,
    pub(super) a: Option<HasP<String>>,
    #[serde(rename = "mod")]
    pub(super) mode: Option<HasP<String>>,
    pub(super) exec_eval: Option<HasP<String>>,
    pub(super) path_eval: Option<PathEval1>,
    pub(super) path_eval_a: Option<Vec<HasP<String>>>,
    pub(super) file_eval: Option<HasP<String>>,
    pub(super) text_eval: Option<HasP<String>>,
    pub(super) log_eval: Option<Vec<HasP<String>>>,
}

// ===== 入口 =====

/// 从 `DocumentMut` 反序列化 (带位置).
pub(super) fn p1_de<'de, T>(doc: &'de DocumentMut, file_id: usize) -> Result<T, ConfError>
where
    T: Deserialize<'de>,
{
    let t = doc.as_table();
    let d = D::from_table(t, file_id);
    T::deserialize(d)
}
