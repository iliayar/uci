use super::parser;
use crate::prelude::*;
use std::path::PathBuf;

#[async_trait::async_trait]
pub trait Evaluate {
    async fn eval(self, state: &mut State) -> Result<Value>;
}

#[async_trait::async_trait]
impl<'a> Evaluate for parser::LValue<'a> {
    async fn eval(self, _state: &mut State) -> Result<Value> {
        match self {
            parser::LValue::String(s) => Ok(Value::String(snailquote::unescape(s)?.to_string())),
            parser::LValue::QuoteString(s) => Ok(Value::String(s.to_string())),
            parser::LValue::Integer(i) => Ok(Value::Integer(i)),
            parser::LValue::Boolean(b) => Ok(Value::Boolean(b)),
            parser::LValue::Null => Ok(Value::Null),
        }
    }
}

#[async_trait::async_trait]
impl<'a> Evaluate for parser::LExpression<'a> {
    async fn eval(self, state: &mut State) -> Result<Value> {
        match self {
            parser::LExpression::Value(v) => v.eval(state).await,
            parser::LExpression::Path(path, scope) => {
                let mut keys: Vec<String> = Vec::new();
                for segment in path.into_iter() {
                    let key = match segment {
                        parser::LPathSegment::Expression(e) => {
                            let k = e.eval(state).await?;
                            match k {
                                Value::String(s) => s,
                                _ => {
                                    return Err(anyhow!(
                                        "Can value on by string keys. Got {}",
                                        k.typename()
                                    ));
                                }
                            }
                        }
                        parser::LPathSegment::String(s) => s.to_string(),
                    };
                    keys.push(key);
                }

                match *scope {
                    parser::PathScope::Global => get_by_path(
                        state
                            .get_global()
                            .ok_or_else(|| anyhow!("Global is not set"))?,
                        &keys,
                    ),
                    parser::PathScope::Expression(expr) => {
                        get_by_path(&expr.eval(state).await?, &keys)
                    }
                }
            }
            parser::LExpression::FsPath(segs, path_type) => {
                let mut segments: Vec<String> = Vec::new();
                for seg in segs.into_iter() {
                    let segment = match seg {
                        parser::LFsPathSegment::String(s) => s.to_string(),
                        parser::LFsPathSegment::Expression(e) => {
                            e.eval(state).await?.try_to_string()?
                        }
                    };

                    segments.push(segment);
                }

                let path: PathBuf = match path_type {
                    parser::LFsPathType::Absolute => segments.into_iter().collect(),
                    parser::LFsPathType::Relative => state
                        .get_current_dir()
                        .ok_or_else(|| anyhow!("Current directory is not set"))?
                        .join(segments.into_iter().collect::<PathBuf>()),
                    parser::LFsPathType::FromHome => PathBuf::from(
                        std::env::var("HOME")
                            .map_err(|err| anyhow!("Cannot expand path from home: {}", err))?,
                    )
                    .join(segments.into_iter().collect::<PathBuf>()),
                };

                Ok(Value::String(path.to_string_lossy().to_string()))
            }
            parser::LExpression::FunctionCall(func_name, args) => {
                state.call_function(func_name, args).await
            }
        }
    }
}

#[async_trait::async_trait]
impl<'a> Evaluate for parser::LFormatString<'a> {
    async fn eval(self, state: &mut State) -> Result<Value> {
        let mut res = String::new();

        for frag in self.0 {
            match frag {
                parser::LFormatStringFragment::Expression(expr) => {
                    res.push_str(&expr.eval(state).await?.try_to_string()?)
                }
                parser::LFormatStringFragment::Raw(s) => res.push_str(s),
            }
        }

        Ok(Value::String(res))
    }
}

fn get_by_path(mut scope: &Value, path: &[String]) -> Result<Value> {
    for key in path.iter() {
        let obj = match scope {
            Value::Dict(obj) => obj,
            _ => {
                return Err(anyhow!(
                    "Can get value by path only in objects. Got {}",
                    scope.typename()
                ))
            }
        };

        scope = obj
            .get(key.as_str())
            .ok_or_else(|| anyhow!("No key {} in global or subobject", key))?;
    }

    Ok(scope.clone())
}
