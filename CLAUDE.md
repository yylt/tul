# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**tul** is a lightweight Cloudflare Worker proxy written in Rust/WASM that provides multiple proxy modes:
- Trojan over WebSocket protocol for secure proxying
- Universal API proxy for routing any HTTP/HTTPS requests
- Docker registry proxy (defaults to Docker Hub)
- DNS over HTTPS (DoH) proxy with Cloudflare IP detection
- Website mirroring with content rewriting

The project compiles Rust to WebAssembly and deploys to Cloudflare Workers using the `worker` crate.

## Development Commands

### Build and Deploy
```bash
# Build and deploy to Cloudflare Workers
make deploy
# or
npx wrangler deploy

# Run locally for development
make dev
# or
npx wrangler dev -c .wrangler.dev.toml
```

### Testing
```bash
# Run all tests
cargo test

# Run specific test
cargo test test_parse_path

# Run tests without executing (compile only)
cargo test --no-run
```

### Build Configuration
The project uses `worker-build` to compile Rust to WASM:
```bash
cargo install -q worker-build && worker-build --release
```

## Architecture

### Request Routing (src/lib.rs)
The main entry point uses a simple router that directs all requests to a single `handler` function in `src/proxy/mod.rs`. The handler performs path-based routing to different proxy modes.

### Proxy Modes (src/proxy/mod.rs)

The main `handler` function routes requests based on path patterns:

1. **Trojan WebSocket** (`/tj` or custom PREFIX): Routes to `tj()` function
   - Establishes WebSocket connection
   - Parses Trojan protocol (password hash validation)
   - Performs DNS lookup with CF IP detection
   - Proxies bidirectional traffic between WebSocket and TCP socket

2. **DNS over HTTPS** (`/dns-query`): Routes to `dns::resolve_handler()`
   - Proxies DNS queries to upstream DoH server (default: 1.1.1.1)
   - Checks if resolved IPs belong to Cloudflare network
   - Uses prefix trie for efficient CF IP range matching

3. **Docker Registry** (`/v2/*`): Routes to `api::image_handler()`
   - Supports multiple registries via `ns` query parameter (docker.io, gcr.io, quay.io, ghcr.io, registry.k8s.io)
   - Defaults to Docker Hub (registry-1.docker.io)

4. **Website Mirroring/API Proxy** (all other paths): Routes to `api::handler()`
   - Parses path as `/{domain}[:{port}][/path]`
   - Uses cookie-based domain persistence for multi-request sessions
   - Rewrites HTML content to replace absolute URLs with proxied versions
   - Removes hop-by-hop headers before forwarding

### Key Components

**src/proxy/tj.rs**: Trojan protocol parser
- Validates 56-byte SHA224 password hash
- Parses SOCKS5-like address format (IPv4 or domain)
- Returns target hostname and port

**src/proxy/dns.rs**: DNS resolution and CF IP detection
- Maintains prefix trie of Cloudflare IP ranges
- Queries DoH endpoint and parses JSON responses
- Returns whether target is behind Cloudflare

**src/proxy/api.rs**: HTTP/HTTPS proxy handler
- Forwards requests with header manipulation
- Rewrites HTML content for website mirroring
- Handles content-type specific processing

**src/proxy/websocket.rs**: WebSocket stream wrapper
- Implements AsyncRead/AsyncWrite for WebSocket
- Enables bidirectional copying with tokio::io::copy_bidirectional

### Configuration via Cloudflare Secrets

The application reads configuration from Cloudflare Worker secrets:
- `PASSWORD`: Trojan password (hashed with SHA224)
- `PREFIX`: Trojan WebSocket path prefix (default: `/tj`)
- `PROXY_DOMAINS`: Comma-separated domains for special handling (currently unused)
- `FORWARD_HOST`: Optional host for forwarding (currently unused)
- `DOH_HOST`: DoH server hostname (default: `1.1.1.1`)

These are set via `npx wrangler secret put <NAME>` or through GitHub Actions during deployment.

### Path Parsing Logic

The `parse_path()` function extracts domain, port, and path from URL patterns:
- `/{domain}` → domain only
- `/{domain}:{port}` → domain and port
- `/{domain}/path` → domain and path
- `/{domain}:{port}/path` → all three components

### Cloudflare IP Detection

The DNS module maintains a prefix trie of CF IP ranges and checks if resolved IPs belong to Cloudflare. This is critical for the Trojan proxy mode - if the target is behind CF, the connection is closed with a message to use DoH and connect directly (to avoid CF blocking CF-to-CF connections).

### Header Handling

The `get_hop_headers()` function defines headers that must be removed when proxying:
- Standard hop-by-hop headers (Connection, Upgrade, etc.)
- Proxy-specific headers (X-Forwarded-*, Via, etc.)
- Cloudflare headers (CF-Ray, CF-IPCountry, etc.)
- **Exception**: `cf-connecting-ip` is preserved to avoid CF CDN blocking

## Deployment

### GitHub Actions Workflows

**Deployment** (`.github/workflows/cf.yml`):
1. Installs Rust toolchain and wrangler
2. Checks for existing secrets and creates them if needed
3. Runs `npx wrangler deploy`
4. Redacts worker URLs in output for security

**CI Testing** (`.github/workflows/ci.yml`):
- Runs on PRs to main and pushes to main/dev/feature branches
- Builds the project in dev mode using `worker-build --dev`

**Dependabot Auto-merge** (`.github/workflows/auto-merge-dependabot.yml`):
- Automatically merges Dependabot PRs for patch and minor version updates
- Waits for CI checks to pass before merging
- Uses squash merge strategy
- For major version updates, adds a comment requesting manual review
- Requires `contents: write` and `pull-requests: write` permissions

### Manual Deployment
1. Set `CLOUDFLARE_API_TOKEN` in `.env` file
2. Run `make deploy`

### Required Secrets
Configure in GitHub repository settings under Secrets and variables → Actions:
- `CLOUDFLARE_API_TOKEN`: Cloudflare API token with Workers permissions
- `PASSWORD`: Trojan password
- `PREFIX`: Trojan path prefix
- `PROXY_DOMAINS`: (optional) Comma-separated proxy domains
- `FORWARD_HOST`: (optional) Forward host configuration

## Important Notes

- The project uses aggressive optimization for WASM: `opt-level = "z"`, LTO, and wasm-opt with `-Oz`
- WebSocket early data is not supported by Cloudflare Workers
- Cloudflare-to-Cloudflare connections may be blocked, hence the CF IP detection logic
- The 10-second read/write timeout may truncate large file downloads - use resume-capable tools (curl -C, wget -c)
- Cookie-based domain persistence (`tul_host` cookie) enables multi-request website mirroring sessions
