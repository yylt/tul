use worker::*;

mod proxy;
mod tools;

#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    proxy::handler(req, &env).await
}
