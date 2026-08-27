use worker::*;

mod proxy;
#[cfg(feature = "tul_cv")]
mod tools;

#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    proxy::handler(req, &env).await
}
