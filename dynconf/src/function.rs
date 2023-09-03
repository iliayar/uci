use crate::prelude::*;

#[async_trait::async_trait]
pub trait Function: Send + Sync {
    async fn call<'a>(&self, state: &mut State, args: Vec<LExpression<'a>>) -> Result<Value>;
}
