mod builtins;
mod eval;
mod function;
mod parser;
mod state;
mod value;

#[cfg(any(feature = "yaml", feature = "json"))]
pub mod util;

pub(crate) mod prelude {
    pub use super::eval::Evaluate;
    pub use super::function::Function;
    pub use super::parser::LExpression;
    pub use super::state::State;
    pub use super::value::Value;

    pub use anyhow::{anyhow, Result};
}

pub use eval::Evaluate;
pub use state::State;
pub use value::Value;
pub use util::DynValue;

#[cfg(test)]
mod test;

pub fn parse_expr(input: &str) -> anyhow::Result<parser::LExpression> {
    match parser::lexpression(input) {
        Err(err) => Err(anyhow::anyhow!(
            "Failed to parse input: {}. Input: {}",
            err,
            input
        )),
        Ok(("", res)) => Ok(res),
        Ok((rest, _)) => Err(anyhow::anyhow!(
            "Failed to parse all input. Reset is {}",
            rest
        )),
    }
}

pub fn parse_string(input: &str) -> anyhow::Result<parser::LFormatString> {
    match parser::lformat_string(input) {
        Err(err) => Err(anyhow::anyhow!(
            "Failed to parse input: {}. Input: {}",
            err,
            input
        )),
        Ok(("", res)) => Ok(res),
        Ok((rest, _)) => Err(anyhow::anyhow!(
            "Failed to parse all input. Reset is {}",
            rest
        )),
    }
}

pub async fn eval_expr<'a>(state: &mut State<'a>, input: &str) -> anyhow::Result<Value> {
    let expr = parse_expr(input)?;
    expr.eval(state).await
}

pub async fn eval_string<'a>(state: &mut State<'a>, input: &str) -> anyhow::Result<String> {
    let s = parse_string(input)?;

    if let Value::String(s) = s.eval(state).await? {
        Ok(s)
    } else {
        unreachable!()
    }
}
