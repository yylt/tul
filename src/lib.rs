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

#[event(scheduled)]
async fn scheduled(event: ScheduledEvent, env: Env, _ctx: ScheduleContext) -> Result<()> {
    let domains = env.var("domains")?.to_string();

    for domain in domains
        .split(',')
        .map(str::trim)
        .filter(|domain| !domain.is_empty())
    {
        console_log!("scheduled {} for {}", event.cron(), domain);
    }

    Ok(())
}
