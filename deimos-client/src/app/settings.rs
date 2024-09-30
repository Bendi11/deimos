use std::sync::Arc;

use crate::context::Context;


#[derive(Debug)]
pub struct Settings {
    ctx: Arc<Context>,
}

impl Settings {
    pub fn new(ctx: Arc<Context>) -> Self {
        Self {
            ctx
        }
    }
}
