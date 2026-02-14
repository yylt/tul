use worker::*;

mod proxy;

use crate::proxy::*;

#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    Router::new()
        .on_async("/*path", handler)
        .on_async("/", handler)
        .run(req, env)
        .await
}
