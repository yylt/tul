# tul 

English | [中文](README.md)

A lightweight Cloudflare Worker proxy written in Rust/WASM.

## ✨ Features

🔒 WebSocket-based Trojan Protocol - Secure proxy protocol over WebSocket, If accessing the CF CDN node, recommended to add header `cf-connecting-ip`

🌐 Universal API Proxy - Route any API through a single endpoint

🐳 Docker Registry Flexibility - Pull from any container registry with Docker Hub as default

⚡ WASM Powered - High-performance Rust implementation

🚀 Easy Deployment - One-click setup via GitHub Actions

## 📖 Usage Guide

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

## 🚀 Quick Start

### Prerequisites
- A Cloudflare account with API access

## 🎨 Deploy

### Easy Deploy
click on the button below:

[![Deploy to Cloudflare Workers](https://deploy.workers.cloudflare.com/button)](https://deploy.workers.cloudflare.com/)

and visit https://{YOUR-WORKERS-SUBDOMAIN}.workers.dev.

### Manually
1. [Create an API token](https://developers.cloudflare.com/fundamentals/api/get-started/create-token/) from the cloudflare dashboard.
2. Update `.env` file and fill the values based on your tokens

| Variable            | Description                                      |
|---------------------|--------------------------------------------------|
| CLOUDFLARE_API_TOKEN | The API key retrieved from Cloudflare dashboard |

3. Deploy
```sh
$ make deploy
```

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
