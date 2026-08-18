# src/ Ponytail 审计提案

## 概述

对 `src/` 下全部 Rust 代码（1944 行）做过度工程（over-engineering）审计。只关注复杂度与冗余，正确性 / 安全 / 性能问题不在本提案范围（见文末备注）。

按削减量从大到小排序。结论摘要：**net: -157 行，-1 依赖（regex），easytier 保留**。

---

## 审计发现（按削减量排序）

### 1. easytier 模块 — 保留，仅清理内部死代码

**现状**：模块尚未接线 —— `src/proxy/mod.rs` 未声明 `mod easytier`，无任何路由入口，`wrangler.toml` 也未绑定 `EASYTIER_KV`。`handle_derp_connection` / `handle_heartbeat` / `list_online_peers` 均无调用方。

**提案（修改后）**：**不删除 easytier**，保留模块待接入。仅清理模块内部无调用的代码：

- `store.rs`：`has_network`（49）、`persist_peer`（61）、`unpersist_peer`（78）无调用方，删除（~-24 行）
- `derp.rs`：`drain_ws_stream`（176）是空 no-op 函数，仅被调用一次且什么都不做，删除（-4 行）
- `derp.rs` 内 `persist_peer_online` / `unpersist_peer_online` 与 `store.rs` 的 `persist_peer` / `unpersist_peer` 功能重复，接入时可合并

**备注**：`docs/design/easytier-wss.md` 设计文档明确"只做节点注册与发现，不做数据转发"，而 `derp.rs` 当前实现了数据包转发（`process_packets` / `queue_packet` / `relay_packet`）。与文档不一致，接入时需对齐。

[src/proxy/easytier/]

### 2. lib.rs Router 简化 — -14 行

两条路由 `/*path` 与 `/`（`/` 已被通配覆盖）是多余的；`proxy::handler` 内部又按 `req.path()` 自行分发，Router 只做了透传。

`#[event(fetch)] async fn fetch(req, env, _ctx) -> Result<Response> { proxy::handler(req, RouteContext::new(env, HashMap::new())).await }`

可移除 worker `Router` 的依赖面（非移除 worker crate 本身）。[src/lib.rs]

### 3. api.rs `replace_host` regex → `str::replace` — -20 行，-1 依赖

`Regex` + 反向引用（73-92 行）可替换为对 HTML body 的 `str::replace("https://", ...).replace("//", ...)`，同样效果、更少代码，并**移除 `regex` 依赖**（Cargo.toml:18）。

注意：纯字符串替换会同时命中 script/CSS 内容而不只是 `src`/`href` 属性；对本项目的 HTML 改写目的足够，需验证 ddg/SP 页面表现正常。[src/proxy/api.rs:73]

### 4. `get_hop_headers` OnceCell → 静态常量 — -40 行

`OnceCell<HashSet>` + async 初始化（api.rs:8-51）包装的是一个常量。改为 `fn is_hop(key: &str) -> bool { matches!(...) }` 或 `static` 数组即可。[src/proxy/api.rs:8]

### 5. `build_search_url` 内联 — -15 行

仅 `tul_s` 一个调用方，两个硬编码后端（"sp" 分支无人配置）。在 match 分支内直接 `let (host, mut url) = ...` 即可，省掉独立函数及其 `q` 处理。[src/proxy/mod.rs:141]

### 6. dns.rs 查询链内联 — -15 行

`is_cf_address` / `resolve_a` / `doh_query` 三层函数只有一个实际入口；`is_cf_address` 只接收 `Address::Domain`（`Ipv4` 分支实际不可达）。将 `resolve_a` 内联进 `is_cf_address`，去掉通用封装。[src/proxy/dns.rs:365]

### 7. `Address` 枚举 → `&str` — -10 行

`tj::parse` 会构造 `Address::Ipv4`（tj.rs:41），但 `tj()` 只把 `hostname`（Domain）传给 `is_cf_address`，`Ipv4` 分支实际不可达。`tj::parse` 直接返回 `&str` 可去掉枚举。[src/proxy/mod.rs:29]

### 8. `skip_name` 冗余边界检查 — -8 行

dns.rs:80-96 循环内的指针截断错误分支，所有调用方在使用前都再次检查边界，属不可能状态防御，可删。[src/proxy/dns.rs:79]

### 9. `get_or_init_env` 签名 — -5 行

返回 `&'a String` 带生命周期，三个调用方都只需要 `&str`。返回 `&str` 或直接 `get().as_deref().unwrap_or(default)` 可简化签名。[src/proxy/mod.rs:35]

### 10. `map_or(None, Some)` → 直接赋值 — -2 行

mod.rs:163-165 `req.query().map_or(None, Some)` 与 `req.query()` 等价，直接 `let query = req.query();`。[src/proxy/mod.rs:163]

---

## 不在范围（正确性相关，仅备注）

- `parse_to_peer_id` 读取 header [4..8] 为 `to_peer_id`，与 12 字节布局注释一致（正确），但处于未接线模块中
- `flush_queue` 在 WS 关闭（异步）前就删除 KV 队列，存在竞态；未接线代码
- `drain_ws_stream` 是空函数，初始 `process_packets` 拿到的是空 buffer（属未接线模块中的 bug，非复杂度）
- ip.rs `escape_html` 四个链式 `.replace()` 各分配一次，已是极简实现，无需改动

---

## 汇总

```
easytier 内部死代码清理        -28 行（模块保留）
lib.rs Router 简化            -14 行
regex replace_host → replace  -20 行，-1 依赖（regex）
get_hop_headers 静态化        -40 行
build_search_url 内联          -15 行
dns 查询链内联                -15 行
Address 枚举 → &str           -10 行
skip_name 边界检查            -8 行
get_or_init_env 签名           -5 行
map_or(None, ...) → 直接      -2 行
```

**net: -157 行，-1 依赖（regex）。** 大头是 lib.rs Router 简化（连带移除 Router 依赖面）与 `get_hop_headers` 静态化；easytier 按用户要求保留，仅清理内部死代码。本提案只列清单，未改动任何代码。
