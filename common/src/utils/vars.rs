use std::{collections::HashMap, iter::Peekable, path::PathBuf, str::Chars};

use log::*;

#[derive(Debug, thiserror::Error)]
pub enum SubstitutionError {
    #[error("Missing closed bracket")]
    MissingClosingBracket,

    #[error("Failed to evaluate expression: {0}")]
    VarsError(#[from] VarsError),

    #[error("Failed to parse index: {0}")]
    ParseIndexError(#[from] std::num::ParseIntError),
}

#[derive(Debug, thiserror::Error)]
pub enum VarsError {
    #[error("Trying to get a key {key} of not an object")]
    NotAnObject { key: String },

    #[error("No such key {key} in object")]
    NoSuchKey { key: String },

    #[error("Index out of bounds {index}")]
    OutOfBounds { index: usize },

    #[error("Trying to get an index {index} of not a list")]
    NotAList { index: usize },

    #[error("Not a string")]
    NotAString,

    #[error("Not a string")]
    NotABool,

    #[error("Cannot set, unmatched types")]
    InvalidSet,

    #[error("Indexes is not allowd in set")]
    SetIndex,

    #[error("Keys is not allowd in append")]
    AppendKey,

    #[error("Cannot merge values of types: {0} and {1}")]
    InvalidMerge(String, String),

    #[error("get_var not imlemented for custom var")]
    GetCustomVarNotImplemented,
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum Value {
    Object(HashMap<String, Value>),
    List(Vec<Value>),
    String(String),
    Bool(bool),
    Integer(i64),
    None,
}

impl Value {
    fn value_name(&self) -> String {
        match self {
            Value::Object(_) => "object",
            Value::List(_) => "list",
            Value::String(_) => "string",
            Value::Bool(_) => "bool",
            Value::Integer(_) => "integer",
            Value::None => "none",
        }
        .to_string()
    }
}

impl Default for Value {
    fn default() -> Self {
        Value::None
    }
}

impl From<()> for Value {
    fn from(_val: ()) -> Self {
        Value::None
    }
}

pub trait IntoVar {}

impl<E: Into<Value>> From<HashMap<String, E>> for Value {
    fn from(m: HashMap<String, E>) -> Self {
        let values = m.into_iter().map(|(k, v)| (k, v.into())).collect();
        Value::Object(values)
    }
}

impl<'a, E> From<&'a HashMap<String, E>> for Value
where
    &'a E: Into<Value>,
{
    fn from(m: &'a HashMap<String, E>) -> Self {
        let values = m.iter().map(|(k, v)| (k.clone(), v.into())).collect();
        Value::Object(values)
    }
}

impl<E: Into<Value>> From<Vec<E>> for Value {
    fn from(v: Vec<E>) -> Self {
        let values = v.into_iter().map(|v| v.into()).collect();
        Value::List(values)
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::String(s)
    }
}

impl From<&String> for Value {
    fn from(s: &String) -> Self {
        Value::String(s.to_string())
    }
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Value::Bool(v)
    }
}

impl From<PathBuf> for Value {
    fn from(path: PathBuf) -> Self {
        path.to_string_lossy().to_string().into()
    }
}

impl From<&PathBuf> for Value {
    fn from(path: &PathBuf) -> Self {
        path.to_string_lossy().to_string().into()
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::Integer(v)
    }
}

#[derive(Default)]
pub struct Path {
    items: Vec<PathItem>,
}

impl Path {
    pub fn key(&mut self, key: String) {
        self.items.push(PathItem::Key(key));
    }

    pub fn index(&mut self, index: usize) {
        self.items.push(PathItem::Index(index));
    }
}

pub enum PathItem {
    Key(String),
    Index(usize),
}

impl Value {
    pub fn get(&self, path: Path) -> Result<&Value, VarsError> {
        let mut result: &Value = self;
        for item in path.items.into_iter() {
            result = match item {
                PathItem::Key(key) => {
                    if let Value::Object(values) = result {
                        if let Some(value) = values.get(&key) {
                            value
                        } else {
                            return Err(VarsError::NoSuchKey { key });
                        }
                    } else {
                        return Err(VarsError::NotAnObject { key });
                    }
                }
                PathItem::Index(index) => {
                    if let Value::List(values) = result {
                        if values.len() <= index {
                            return Err(VarsError::OutOfBounds { index });
                        } else {
                            &values[index]
                        }
                    } else {
                        return Err(VarsError::NotAList { index });
                    }
                }
            };
        }
        Ok(result)
    }

    pub fn get_mut(&mut self, path: Path) -> Result<&mut Value, VarsError> {
        let mut result: &mut Value = self;
        for item in path.items.into_iter() {
            result = match item {
                PathItem::Key(key) => {
                    if let Value::Object(values) = result {
                        if let Some(value) = values.get_mut(&key) {
                            value
                        } else {
                            return Err(VarsError::NoSuchKey { key });
                        }
                    } else {
                        return Err(VarsError::NotAnObject { key });
                    }
                }
                PathItem::Index(index) => {
                    if let Value::List(values) = result {
                        if values.len() <= index {
                            return Err(VarsError::OutOfBounds { index });
                        } else {
                            &mut values[index]
                        }
                    } else {
                        return Err(VarsError::NotAList { index });
                    }
                }
            };
        }
        Ok(result)
    }

    pub fn get_string(&self, path: Path) -> Result<String, VarsError> {
        match self.get(path)? {
            Value::String(s) => Ok(s.to_string()),
            Value::Bool(v) => Ok(v.to_string()),
            Value::Integer(i) => Ok(i.to_string()),
            _ => Err(VarsError::NotAString),
        }
    }

    pub fn eval<S: AsRef<str>>(&self, text: S) -> Result<String, SubstitutionError> {
        substitute_vars(self, text.as_ref())
    }

    pub fn assign<S: AsRef<str>>(
        &mut self,
        path: S,
        value: Value,
    ) -> Result<(), SubstitutionError> {
        let path = parse_path_simple(path.as_ref())?;
        self.assign_path(path, value)?;
        Ok(())
    }

    pub fn assign_path(&mut self, path: Path, value: Value) -> Result<(), VarsError> {
        let mut result: &mut Value = self;
        for item in path.items.into_iter() {
            result = match item {
                PathItem::Key(key) => match result {
                    Value::Object(values) => {
                        if !values.contains_key(&key) {
                            values.insert(key.clone(), Value::None);
                        }

                        values.get_mut(&key).unwrap()
                    }
                    Value::None => {
                        *result = Value::Object(HashMap::from_iter([(key.clone(), Value::None)]));
                        if let Value::Object(values) = result {
                            values.get_mut(&key).unwrap()
                        } else {
                            unreachable!()
                        }
                    }
                    _ => {
                        return Err(VarsError::NotAnObject { key });
                    }
                },
                PathItem::Index(index) => match result {
                    Value::List(values) => {
                        while values.len() <= index {
                            values.push(Value::None);
                        }
                        &mut values[index]
                    }
                    Value::None => {
                        *result = Value::List(vec![Value::None; index + 1]);
                        if let Value::List(values) = result {
                            &mut values[index]
                        } else {
                            unreachable!()
                        }
                    }
                    _ => {
                        return Err(VarsError::NotAList { index });
                    }
                },
            };
        }
        *result = value;
        Ok(())
    }

    pub fn from_list<S: AsRef<str>>(assignes: &[(S, Value)]) -> Result<Value, SubstitutionError> {
        let mut vars = Value::default();
        for (path, value) in assignes {
            vars.assign(path, value.clone())?;
        }
        Ok(vars)
    }

    pub fn from_list_strings<S: AsRef<str>, V: ToString>(
        assignes: &[(S, V)],
    ) -> Result<Value, SubstitutionError> {
        let mut nassigned = Vec::new();
        for (path, value) in assignes {
            nassigned.push((path, Value::String(value.to_string())));
        }
        Value::from_list(&nassigned)
    }

    pub fn merge(self, other: Value) -> Result<Value, SubstitutionError> {
        match (self, other) {
            (Value::None, value) => Ok(value),
            (value, Value::None) => Ok(value),
            (Value::Object(lhs_values), Value::Object(rhs_values)) => {
                let mut values = lhs_values;
                for (k, v) in rhs_values.into_iter() {
                    let cur = values.remove(&k).unwrap_or(Value::None);
                    values.insert(k, cur.merge(v)?);
                }
                Ok(Value::Object(values))
            }
            (Value::List(lhs_values), Value::List(mut rhs_values)) => {
                let mut values = lhs_values;
                values.append(&mut rhs_values);
                Ok(Value::List(values))
            }
            (lhs, rhs) => Err(VarsError::InvalidMerge(lhs.value_name(), rhs.value_name()).into()),
        }
    }
}

pub fn substitute_vars(vars: &Value, text: &str) -> Result<String, SubstitutionError> {
    trace!("Substituting vars: {:?} into {}", vars, text);
    let mut chars = text.chars().peekable();
    parse_raw(vars, &mut chars)
}

fn parse_raw(vars: &Value, chars: &mut Peekable<Chars>) -> Result<String, SubstitutionError> {
    enum State {
        Start,
        End,
    }

    let mut res = String::new();
    let mut st = State::Start;

    loop {
        st = match st {
            State::Start => match chars.peek() {
                Some(ch) => match ch {
                    '$' => {
                        res.push_str(&try_parse_expr(vars, chars)?);
                        State::Start
                    }
                    _ => {
                        res.push(chars.next().unwrap());
                        State::Start
                    }
                },
                None => State::End,
            },
            State::End => {
                return Ok(res);
            }
        };
    }
}

fn try_parse_expr(vars: &Value, chars: &mut Peekable<Chars>) -> Result<String, SubstitutionError> {
    enum State {
        Start,
        End,
        ExprEnd,
        WasDollar(usize),
    }

    let mut res = String::new();
    let mut st = State::Start;

    loop {
        st = match st {
            State::Start => match chars.peek() {
                Some('$') => {
                    chars.next().unwrap();
                    State::WasDollar(1)
                }
                _ => {
                    unreachable!("parse_expr should be only called with first dolalr");
                }
            },
            State::WasDollar(n) => match chars.peek() {
                Some(ch) => match ch {
                    '$' => {
                        chars.next().unwrap();
                        State::WasDollar(n + 1)
                    }
                    '{' => match n {
                        0 => unreachable!("cannot be 0 dollars here"),
                        1 => {
                            chars.next().unwrap();
                            res.push_str(&vars.get_string(parse_path(vars, chars)?)?);
                            State::ExprEnd
                        }
                        n => {
                            res.push_str(&"$".repeat(n - 1));
                            State::End
                        }
                    },
                    _ => {
                        res.push_str(&"$".repeat(n));
                        State::End
                    }
                },
                None => {
                    res.push_str(&"$".repeat(n));
                    State::End
                }
            },
            State::ExprEnd => match chars.peek() {
                Some('}') => {
                    chars.next().unwrap();
                    State::End
                }
                _ => {
                    return Err(SubstitutionError::MissingClosingBracket);
                }
            },
            State::End => {
                return Ok(res);
            }
        };
    }
}

fn parse_path_simple(path: &str) -> Result<Path, SubstitutionError> {
    let mut chars = path.chars().peekable();
    let vars = Value::default();
    parse_path(&vars, &mut chars)
}

fn parse_path(vars: &Value, chars: &mut Peekable<Chars>) -> Result<Path, SubstitutionError> {
    enum State {
        Start,
        Key,
        Index,
        End,
    }

    let mut path: Path = Default::default();
    let mut current = String::new();
    let mut st = State::Start;

    loop {
        st = match st {
            State::Start => match chars.peek() {
                Some(ch) => match ch {
                    '[' => {
                        chars.next().unwrap();
                        State::Index
                    }
                    '}' => State::End,
                    _ => State::Key,
                },
                None => State::End,
            },
            State::Key => match chars.peek() {
                Some(ch) => match ch {
                    '.' => {
                        chars.next().unwrap();
                        path.key(current);
                        current = String::new();
                        State::Key
                    }
                    '[' => {
                        chars.next().unwrap();
                        path.key(current);
                        current = String::new();
                        State::Index
                    }
                    '}' => {
                        path.key(current);
                        current = String::new();
                        State::End
                    }
                    '$' => {
                        current.push_str(&try_parse_expr(vars, chars)?);
                        State::Key
                    }
                    _ => {
                        current.push(chars.next().unwrap());
                        State::Key
                    }
                },
                None => {
                    path.key(current);
                    current = String::new();
                    State::End
                }
            },
            State::Index => match chars.peek() {
                Some(ch) => match ch {
                    ']' => {
                        chars.next().unwrap();
                        path.index(current.parse()?);
                        current = String::new();
                        State::Start
                    }
                    '$' => {
                        current.push_str(&try_parse_expr(vars, chars)?);
                        State::Index
                    }
                    _ => {
                        current.push(chars.next().unwrap());
                        State::Index
                    }
                },
                None => {
                    return Err(SubstitutionError::MissingClosingBracket);
                }
            },
            State::End => {
                return Ok(path);
            }
        }
    }
}

mod tests {
    #[allow(unused_imports)]
    use std::collections::HashMap;

    #[test]
    fn test_vars_from_string() -> Result<(), anyhow::Error> {
        let values = String::from("123");
        let vars: super::Value = values.into();

        let path = super::Path::default();

        assert_eq!(vars.get_string(path)?, "123");
        Ok(())
    }

    #[test]
    fn test_vars_from_vec() -> Result<(), anyhow::Error> {
        let values = vec![
            String::from("123"),
            String::from("aboba"),
            String::from("456"),
        ];
        let vars: super::Value = values.into();

        let mut path = super::Path::default();
        path.index(1);

        assert_eq!(vars.get_string(path)?, "aboba");
        Ok(())
    }

    #[test]
    fn test_vars_from_object() -> Result<(), anyhow::Error> {
        let values = HashMap::from_iter([
            (String::from("a"), String::from("123")),
            (String::from("bb"), String::from("aboba")),
            (String::from("ccc"), String::from("456")),
        ]);
        let vars: super::Value = values.into();

        let mut path = super::Path::default();
        path.key(String::from("bb"));

        assert_eq!(vars.get_string(path)?, "aboba");
        Ok(())
    }

    #[test]
    fn test_vars_from_complex() -> Result<(), anyhow::Error> {
        let values = HashMap::from_iter([
            (String::from("a"), super::Value::String(String::from("123"))),
            (
                String::from("b"),
                vec![String::from("ab"), String::from("o"), String::from("ba")].into(),
            ),
            (
                String::from("c"),
                HashMap::from_iter([(String::from("a"), String::from("lul"))]).into(),
            ),
        ]);
        let vars: super::Value = values.into();

        let mut path = super::Path::default();
        path.key(String::from("a"));

        assert_eq!(vars.get_string(path)?, "123");

        let mut path = super::Path::default();
        path.key(String::from("b"));
        path.index(1);

        assert_eq!(vars.get_string(path)?, "o");

        let mut path = super::Path::default();
        path.key(String::from("c"));
        path.key(String::from("a"));

        assert_eq!(vars.get_string(path)?, "lul");
        Ok(())
    }

    #[test]
    fn test_parse_no_vars() -> Result<(), anyhow::Error> {
        let vars = super::Value::None;

        let content = "aboba_amogis";
        assert_eq!(super::substitute_vars(&vars, content)?, "aboba_amogis");

        let content = "aboba_amogis$$${";
        assert_eq!(super::substitute_vars(&vars, content)?, "aboba_amogis$${");

        let content = "aboba_amogis$$$";
        assert_eq!(super::substitute_vars(&vars, content)?, "aboba_amogis$$$");

        let content = "aboba_amogis$${}";
        assert_eq!(super::substitute_vars(&vars, content)?, "aboba_amogis${}");

        let content = "aboba_amogis$${}$$ $${}";
        assert_eq!(
            super::substitute_vars(&vars, content)?,
            "aboba_amogis${}$$ ${}"
        );

        Ok(())
    }

    #[test]
    fn test_parse_vars() -> Result<(), anyhow::Error> {
        let values = HashMap::from_iter([
            (String::from("a"), super::Value::String(String::from("123"))),
            (
                String::from("b"),
                vec![String::from("ab"), String::from("o"), String::from("ba")].into(),
            ),
            (
                String::from("c"),
                HashMap::from_iter([(String::from("a"), String::from("lul"))]).into(),
            ),
            (
                String::from("keys"),
                vec![
                    String::from("a"),
                    String::from("b"),
                    String::from("c"),
                    String::from("0"),
                ]
                .into(),
            ),
        ]);
        let vars = values.into();

        let content = "this ${a}";
        assert_eq!(super::substitute_vars(&vars, content)?, "this 123");

        let content = "this ${b[1]}";
        assert_eq!(super::substitute_vars(&vars, content)?, "this o");

        let content = "this ${c.a}";
        assert_eq!(super::substitute_vars(&vars, content)?, "this lul");

        let content = "this ${a} - ${b[0]} + ${c.a}";
        assert_eq!(
            super::substitute_vars(&vars, content)?,
            "this 123 - ab + lul"
        );

        let content = "this ${${keys[0]}}";
        assert_eq!(super::substitute_vars(&vars, content)?, "this 123");

        let content = "this ${c.${keys[0]}}";
        assert_eq!(super::substitute_vars(&vars, content)?, "this lul");

        let content = "this ${${keys[2]}.${keys[0]}}";
        assert_eq!(super::substitute_vars(&vars, content)?, "this lul");

        let content = "this ${b[${keys[3]}]}";
        assert_eq!(super::substitute_vars(&vars, content)?, "this ab");

        Ok(())
    }

    #[test]
    fn test_assign() -> Result<(), anyhow::Error> {
        let mut vars = super::Value::default();
        vars.assign_path(super::parse_path_simple("a.b")?, "123".to_string().into())?;
        vars.assign_path(super::parse_path_simple("a.g")?, "aboba".to_string().into())?;
        vars.assign_path(super::parse_path_simple("b[5]")?, "lol".to_string().into())?;
        vars.assign_path(super::parse_path_simple("c")?, "booba".to_string().into())?;

        let content = "this ${a.b} ${a.g} ${b[5]} ${c}";
        assert_eq!(
            super::substitute_vars(&vars, content)?,
            "this 123 aboba lol booba"
        );

        Ok(())
    }

    #[test]
    fn test_assign_strings() -> Result<(), anyhow::Error> {
        let vars = super::Value::from_list_strings(&[
            ("a.b", "123"),
            ("a.g", "aboba"),
            ("b[5]", "lol"),
            ("c", "booba"),
        ])?;

        let content = "this ${a.b} ${a.g} ${b[5]} ${c}";
        assert_eq!(
            super::substitute_vars(&vars, content)?,
            "this 123 aboba lol booba"
        );

        Ok(())
    }
}
