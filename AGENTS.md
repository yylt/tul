## AI Coding Rules

### 1. Think Before Coding

**Don't assume. Don't hide confusion. Surface tradeoffs.**

Before implementing:
- State your assumptions explicitly. If uncertain, ask.
- If multiple interpretations exist, present them - don't pick silently.
- If a simpler approach exists, say so. Push back when warranted.
- If something is unclear, stop. Name what's confusing. Ask.

### 2. Simplicity First

**Minimum code that solves the problem. Nothing speculative.**

- No features beyond what was asked.
- No abstractions for single-use code.
- No "flexibility" or "configurability" that wasn't requested.
- No error handling for impossible scenarios.
- If you write 200 lines and it could be 50, rewrite it.

Ask yourself: "Would a senior engineer say this is overcomplicated?" If yes, simplify.

### 3. Surgical Changes

**Touch only what you must. Clean up only your own mess.**

When editing existing code:
- Don't "improve" adjacent code, comments, or formatting.
- Don't refactor things that aren't broken.
- Match existing style, even if you'd do it differently.
- If you notice unrelated dead code, mention it - don't delete it.

When your changes create orphans:
- Remove imports/variables/functions that YOUR changes made unused.
- Don't remove pre-existing dead code unless asked.

The test: Every changed line should trace directly to the user's request.

### 4. Goal-Driven Execution

**Define success criteria. Loop until verified.**

Transform tasks into verifiable goals:
- "Add validation" → "Write tests for invalid inputs, then make them pass"
- "Fix the bug" → "Write a test that reproduces it, then make it pass"
- "Refactor X" → "Ensure tests pass before and after"

For multi-step tasks, state a brief plan:
```
1. [Step] → verify: [check]
2. [Step] → verify: [check]
3. [Step] → verify: [check]
```

---

## Project Overview

Tul is a multi-purpose Cloudflare Worker reverse proxy written in Rust (compiled to WASM). Provides web proxy, Trojan/WebSocket, Docker registry proxy, DoH/ECH injection, MCP tool server, and EasyTier DERP relay.

**Target**: `wasm32-unknown-unknown` | **SDK**: `worker` 0.8.5 | **Build**: `worker-build`

## Source Tree

```
src/
├── lib.rs                  # Entry point: Router -> proxy::handler
└── proxy/
    ├── mod.rs              # Main router, path parsing, Trojan WS handler
    ├── api.rs              # HTTP proxy (handler), Docker registry proxy (image_handler/v2_handler)
    ├── ip.rs               # /tul_ip and / info endpoints
    ├── ip.html             # HTML template for /
    ├── dns.rs              # DoH proxy, DNS wire-format parsing, CF IP detection, ECH injection
    ├── mcp.rs              # MCP tool server (webfetch)
    ├── tj.rs               # Trojan protocol parser
    ├── websocket.rs        # WsStream: WebSocket as AsyncRead/AsyncWrite
    ├── easytier/
    │   ├── mod.rs          # pub mod derp; pub mod store;
    │   ├── derp.rs         # DERP relay, packet routing, KV queue
    │   └── store.rs        # In-memory peer registry + KV persistence
```

## Route Map

| Path | Handler | Purpose |
|------|---------|---------|
| `/dns-query` | `dns::resolve_handler` + `process_response` | DoH proxying with ECH injection |
| `/tulmcp` | `mcp::handler` | MCP tool server (GET=listing, POST=invoke) |
| `/tj*` | `tj()` (mod.rs) | Trojan over WebSocket |
| `/v2` | `api::image_handler` | Docker registry proxy |
| `/tul_s` | `api::handler` / `ip::handler_s` | Search proxy (ddg/sp) + search UI |
| `/tul_dl` | `ip::handler_dl` | Download accelerator page |
| `/tul_ip` | `ip::handler_text` | Plaintext client IP |
| `/` | `ip::handler_html` | HTML info page |
| `*` | `parse_path` → `api::handler` | Generic web proxy |

## Key Data Flows

### Generic Proxy (`api::handler`)
1. Strip hop-by-hop headers (`HOP_HEADERS`: authorization, connection, host, transfer-encoding, etc.)
2. Set `host` to target, clear `referer`
3. Fetch upstream → rewrite redirect `Location` headers → if HTML: rewrite src/href URLs, set `tul_host` cookie → stream non-HTML

### Docker Registry (`v2_handler`)
1. Same hop headers, then **re-add `authorization`** (registry auth requires it)
2. Strip CSP, rewrite `Location` + `www-authenticate` headers
3. No HTML rewriting, no cookie

### Trojan
WebSocket upgrade → parse Trojan protocol (56B password hash + cmd + addr + port) → DoH resolve → TCP connect → bidirectional copy

### DoH + ECH
Forward query to upstream → parse HTTPS (type 65) record → if no ECH and all IPs are Cloudflare: replace with `ECH_DOMAIN`'s HTTPS record

## Hop Headers

`api.rs::HOP_HEADERS` — stripped from all proxied requests

## Configuration

**Worker secrets** (`wrangler secret put`):
- `PASSWORD` — Trojan password (SHA-224 hashed, default: `password`)
- `PREFIX` — Trojan WS path (default: `/tj`)
- `DOH_HOST` — upstream DoH (default: `dns.google`)
- `ECH_DOMAIN` — ECH record source (default: `linux.do`)

**Build**: `opt-level = "z"`, `lto = true`, `strip = true`, `codegen-units = 1`

## Testing

- `cargo test` — unit tests: `parse_path` (mod.rs), `is_cloudflare_ip` (dns.rs)
- `make ci` — fmt-check + lint + test

### Integration Tests

Located in `integration/`:

| Script | Purpose | Requires |
|--------|---------|----------|
| `e2e-test.sh` | Generic web proxy, DoH | `wrangler dev` running |
| `mirror-test.sh` | Docker registry proxy (Docker Hub via `/v2`) | `wrangler dev` running; auto-downloads `crane` v0.21.7 |

Run locally:
```bash
# Terminal 1: start worker
make dev

# Terminal 2: run tests
WORKER_URL=http://localhost:8787 ./integration/e2e-test.sh
WORKER_URL=http://localhost:8787 ./integration/mirror-test.sh
```

### GitHub Actions

| Workflow | Trigger | What it does |
|----------|---------|--------------|
| `.github/workflows/ci.yml` | PR to `main` | `make ci` (fmt + clippy + test) + dev build |
| `.github/workflows/e2e-test.yml` | PR to `main` | Builds worker, starts `wrangler dev`, runs `e2e-test.sh` + `mirror-test.sh` |

## Commands

| Command | Purpose |
|---------|---------|
| `make build` | Release WASM build |
| `make dev` | Local dev server |
| `make deploy` | Deploy to Cloudflare |
| `make ci` | CI checks (fmt + clippy + test) |
| `cargo check` | Fast compile check |
