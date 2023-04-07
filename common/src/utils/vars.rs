use std::{collections::HashMap, iter::Peekable, str::Chars};

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

    #[error("Not a custom value")]
    NotACustom,

    #[error("Cannot set, unmatched types")]
    InvalidSet,

    #[error("Indexes is not allowd in set")]
    SetIndex,

    #[error("Keys is not allowd in append")]
    AppendKey,

    #[error("get_var not imlemented for custom var")]
    GetCustomVarNotImplemented,
}

#[derive(serde::Deserialize, serde::Serialize, Clone)]
pub enum Vars {
    Object { values: HashMap<String, Vars> },
    List { values: Vec<Vars> },
    String(String),
    None,
}

pub enum Value<T = ()> {
    Object(HashMap<String, T>),
    List(Vec<T>),
    String(String),
    None,
}

impl Default for Vars {
    fn default() -> Self {
        Vars::None
    }
}

impl Into<Vars> for () {
    fn into(self) -> Vars {
        Vars::None
    }
}

impl<E: Into<Vars>> Into<Vars> for Value<E> {
    fn into(self) -> Vars {
        match self {
            Value::Object(m) => Vars::Object {
                values: m.into_iter().map(|(k, v)| (k, v.into())).collect(),
            },
            Value::List(l) => Vars::List {
                values: l.into_iter().map(Into::into).collect(),
            },
            Value::String(s) => Vars::String(s),
            Value::None => Vars::None,
        }
    }
}

pub trait IntoVar {}

impl<E: Into<Vars>> From<HashMap<String, E>> for Vars {
    fn from(m: HashMap<String, E>) -> Self {
        let values = m.into_iter().map(|(k, v)| (k, v.into())).collect();
        Vars::Object { values }
    }
}

impl<E: Into<Vars>> From<Vec<E>> for Vars {
    fn from(v: Vec<E>) -> Self {
        let values = v.into_iter().map(|v| v.into()).collect();
        Vars::List { values }
    }
}

impl From<String> for Vars {
    fn from(s: String) -> Self {
        Vars::String(s)
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

impl Vars {
    pub fn get(&self, path: Path) -> Result<&Vars, VarsError> {
        let mut result: &Vars = &self;
        for item in path.items.into_iter() {
            result = match item {
                PathItem::Key(key) => {
                    if let Vars::Object { values } = result {
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
                    if let Vars::List { values } = result {
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

    pub fn get_mut(&mut self, path: Path) -> Result<&mut Vars, VarsError> {
        let mut result: &mut Vars = self;
        for item in path.items.into_iter() {
            result = match item {
                PathItem::Key(key) => {
                    if let Vars::Object { values } = result {
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
                    if let Vars::List { values } = result {
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
        if let Vars::String(s) = self.get(path)? {
            Ok(s.to_string())
        } else {
            Err(VarsError::NotAString)
        }
    }

    pub fn eval(&self, text: &str) -> Result<String, SubstitutionError> {
        substitute_vars(self, text)
    }

    pub fn assign<S: AsRef<str>>(&mut self, path: S, value: Vars) -> Result<(), SubstitutionError> {
        let path = parse_path_simple(path.as_ref())?;
	self.assign_path(path, value)?;
	Ok(())
    }

    pub fn assign_path(&mut self, path: Path, value: Vars) -> Result<(), VarsError> {
        let mut result: &mut Vars = self;
        for item in path.items.into_iter() {
            result = match item {
                PathItem::Key(key) => match result {
                    Vars::Object { values } => {
                        if !values.contains_key(&key) {
                            values.insert(key.clone(), Vars::None);
                        }

                        values.get_mut(&key).unwrap()
                    }
                    Vars::None => {
                        *result = Vars::Object {
                            values: HashMap::from_iter([(key.clone(), Vars::None)]),
                        };
                        if let Vars::Object { values } = result {
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
                    Vars::List { values } => {
                        while values.len() <= index {
                            values.push(Vars::None);
                        }
                        &mut values[index]
                    }
                    Vars::None => {
                        *result = Vars::List {
                            values: vec![Vars::None; index + 1],
                        };
                        if let Vars::List { values } = result {
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

    pub fn from_list<S: AsRef<str>>(assignes: &[(S, Vars)]) -> Result<Vars, SubstitutionError> {
        let mut vars = Vars::default();
        for (path, value) in assignes {
            vars.assign(path, value.clone())?;
        }
        Ok(vars)
    }

    pub fn from_list_strings<S: AsRef<str>, V: ToString>(
        assignes: &[(S, V)],
    ) -> Result<Vars, SubstitutionError> {
        let mut nassigned = Vec::new();
        for (path, value) in assignes {
            nassigned.push((path.clone(), Vars::String(value.to_string())));
        }
        Vars::from_list(&nassigned)
    }
}

pub fn substitute_vars(vars: &Vars, text: &str) -> Result<String, SubstitutionError> {
    let mut chars = text.chars().peekable();
    parse_raw(vars, &mut chars)
}

fn parse_raw(vars: &Vars, chars: &mut Peekable<Chars>) -> Result<String, SubstitutionError> {
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

fn try_parse_expr(vars: &Vars, chars: &mut Peekable<Chars>) -> Result<String, SubstitutionError> {
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
                Some(ch) => match ch {
                    '$' => {
                        chars.next().unwrap();
                        State::WasDollar(1)
                    }
                    _ => {
                        unreachable!("parse_expr should be only called with first dolalr");
                    }
                },
                None => {
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
                Some(ch) => match ch {
                    '}' => {
                        chars.next().unwrap();
                        State::End
                    }
                    _ => {
                        return Err(SubstitutionError::MissingClosingBracket);
                    }
                },
                None => {
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
    let vars = Vars::default();
    parse_path(&vars, &mut chars)
}

fn parse_path(vars: &Vars, chars: &mut Peekable<Chars>) -> Result<Path, SubstitutionError> {
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
    use std::collections::HashMap;

    #[test]
    fn test_vars_from_string() -> Result<(), anyhow::Error> {
        let values = String::from("123");
        let vars: super::Vars = super::Vars::from(values);

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
        let vars: super::Vars = super::Vars::from(values);

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
        let vars: super::Vars = super::Vars::from(values);

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
                super::Value::List(vec![
                    String::from("ab"),
                    String::from("o"),
                    String::from("ba"),
                ]),
            ),
            (
                String::from("c"),
                super::Value::Object(HashMap::from_iter([(
                    String::from("a"),
                    String::from("lul"),
                )])),
            ),
        ]);
        let vars: super::Vars = super::Vars::from(values);

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
        let vars = super::Vars::None;

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
                super::Value::List(vec![
                    String::from("ab"),
                    String::from("o"),
                    String::from("ba"),
                ]),
            ),
            (
                String::from("c"),
                super::Value::Object(HashMap::from_iter([(
                    String::from("a"),
                    String::from("lul"),
                )])),
            ),
            (
                String::from("keys"),
                super::Value::List(vec![
                    String::from("a"),
                    String::from("b"),
                    String::from("c"),
                    String::from("0"),
                ]),
            ),
        ]);
        let vars = super::Vars::from(values);

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
        let mut vars = super::Vars::default();
        vars.assign(
            super::parse_path_simple("a.b")?,
            super::Value::<()>::String("123".to_string()).into(),
        )?;
        vars.assign(
            super::parse_path_simple("a.g")?,
            super::Value::<()>::String("aboba".to_string()).into(),
        )?;
        vars.assign(
            super::parse_path_simple("b[5]")?,
            super::Value::<()>::String("lol".to_string()).into(),
        )?;
        vars.assign(
            super::parse_path_simple("c")?,
            super::Value::<()>::String("booba".to_string()).into(),
        )?;

        let content = "this ${a.b} ${a.g} ${b[5]} ${c}";
        assert_eq!(
            super::substitute_vars(&vars, content)?,
            "this 123 aboba lol booba"
        );

        Ok(())
    }

    #[test]
    fn test_assign_strings() -> Result<(), anyhow::Error> {
        let vars = super::Vars::from_list_strings(&[
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
