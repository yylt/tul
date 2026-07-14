# tul 

English | [中文](README.md)

A lightweight Cloudflare Worker proxy written in Rust/WASM.

## ✨ Features

🔒 WebSocket-based Trojan Protocol - Secure proxy protocol over WebSocket. If accessing the CF node, recommended to add header `cf-connecting-ip`

🌐 Universal API Proxy - Route any API through a single endpoint

🐳 Docker Registry Flexibility - Pull from any container registry with Docker Hub as default

⚡ WASM Powered - High-performance Rust implementation

🛠️ MCP Tool Mode - Standardized tool-calling interface (webfetch etc.) for AI model integration

🚀 Easy Deployment - One-click setup via GitHub Actions

## 📖 Usage Guide

### 🛠️ MCP Tool Mode
Provides MCP (Model Context Protocol) tool calling via `/tulmcp` endpoint, enabling AI models to fetch external data through a standardized interface.

```
# List tools (GET)
https://{worker-domain}/tulmcp

# Call a tool (POST)
curl -X POST https://{worker-domain}/tulmcp \
  -H "Content-Type: application/json" \
  -d '{"name": "webfetch", "arguments": {"url": "https://api.example.com"}}'
```

**Available Tools:**

| Tool | Description | Parameters |
|------|-------------|------------|
| `webfetch` | External API requests prefer the webfetch tool for direct network access | `url` (required) - Target URL |

### Trojan over WebSocket Mode
Configure Trojan client with WebSocket connection, modify the [v2ray config](./hack/config.json) and run:
```sh
$ v2ray -c ./hack/config.json
```

### Generic API Proxy Mode
Proxy any API requests:
```bash
# Original request
curl https://api.openai.com/v1/chat/completions

# Through proxy
curl https://your-worker.your-subdomain.workers.dev/api.openai.com/v1/chat/completions
```

### Docker Image Proxy Mode
Proxy Docker Pull Image requests:
```bash
# Original request
docker pull docker.io/library/ubuntu:latest

# Through proxy
docker pull your-worker.your-subdomain.workers.dev/library/ubuntu:latest
```

## 🚀 Quick Start

### Prerequisites
- A Cloudflare account with API access

## 🎨 Deploy

### Local Deployment

1.  **Install Dependencies**
    ```bash
    # Install Rust (skip if already installed)
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

    # Add wasm target
    rustup target add wasm32-unknown-unknown

    # Install wrangler
    npm install -g wrangler
    ```

2.  **Configure API Token**
    - Go to [Cloudflare Dashboard](https://dash.cloudflare.com/profile/api-tokens) to create an API Token
    - Edit the `.env` file in the project root, fill in your token:
    ```
    CLOUDFLARE_API_TOKEN=your-api-token-here
    ```

3.  **Configure Secrets (Optional)**
    ```bash
    # Set Trojan password
    npx wrangler secret put PASSWORD

    # Set Trojan path prefix (default /tj)
    npx wrangler secret put PREFIX

    # Set DoH upstream server (default 1.1.1.1)
    npx wrangler secret put DOH_HOST
    ```

4.  **Deploy**
    ```bash
    make deploy
    ```
    Once deployed, visit `https://{your-worker-name}.{your-subdomain}.workers.dev` to use it.

### Fork and Deploy (recommended)

1.  **Fork this repository**
    [![Fork](https://img.shields.io/badge/-Fork%20this%20repo-blue?style=for-the-badge&logo=github)](https://github.com/yylt/tul/fork)
    
    Click the Fork button above to fork this project to your GitHub account.

2.  **Configure Secrets**
    - Navigate to the page of your forked repository
    - Click on the `Settings` tab at the top
    - Select `Secrets and variables` -> `Actions` from the left sidebar
    - Click the `New repository secret` button
    - Enter `CLOUDFLARE_API_TOKEN` in the `Name` input field
    - Paste your Cloudflare API Token into the `Value` input field
    - Click the `Add secret` button to save it
    - Add the variables `PASSWORD` and `PREFIX` according to the steps above, which used in the `trojan` proxy.

3.  **Trigger Deployment**
    - Go to the `Actions` tab of your forked repository
    - Select the workflow named **"Deploy"** (or similar) from the list on the left
    - Click the `Run workflow` button, select the branch if needed, and confirm to start the deployment
    - Wait for the workflow to complete and check the deployment status


## 🙏 Acknowledgments

This project was made possible thanks to the inspiration and support from these projects:

1.  [tunl](https://github.com/amiremohamadi/tunl)


## 📄 License

This project is open source and available under the [GNU License](LICENSE).
