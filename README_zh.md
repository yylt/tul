# tul 

[English](README.md) | [中文](README_zh.md)

一个轻量级的基于 Cloudflare Worker 代理，使用 Rust/WASM 编写。

## ✨ 特性

🔒 基于 WebSocket 的 Trojan 协议 - 通过 WebSocket 传输的安全代理协议

🌐 通用 API 代理 - 通过单一端点路由任何 API

⚡ WASM 驱动 - 高性能 Rust 实现

🚀 简易部署 - 通过 GitHub Actions 一键设置

## 📖 使用指南

### Trojan over WebSocket 模式
配置支持 WebSocket 连接的 Trojan 客户端，修改 [v2ray 配置](./hack/config.json) 并运行：
```sh
$ v2ray -c ./hack/config.json
```

### 通用 API 代理模式
代理任何 API 请求：
```bash
# 原始请求
curl https://api.openai.com/v1/chat/completions

# 通过代理
curl https://your-worker.your-subdomain.workers.dev/api.openai.com/v1/chat/completions
```

## 🚀 快速开始

### 先决条件
- 拥有 API 访问权限的 Cloudflare 账户

## 🎨 部署

### 简易部署
点击下方按钮：

[![Deploy to Cloudflare Workers](https://deploy.workers.cloudflare.com/button)](https://deploy.workers.cloudflare.com/)

并访问 https://{YOUR-WORKERS-SUBDOMAIN}.workers.dev 

### 手动部署
1. 从 Cloudflare 仪表板 [创建 API 令牌](https://developers.cloudflare.com/fundamentals/api/get-started/create-token/)。
2. 更新 `.env` 文件并根据您的令牌填写值

| 变量名               | 描述                                           |
|---------------------|--------------------------------------------------|
| CLOUDFLARE_API_TOKEN | 从 Cloudflare 仪表板获取的 API 密钥            |

3. 部署
```sh
$ make deploy
```

### Fork 并部署（推荐）

1.  **Fork 此仓库**
    [![Fork](https://img.shields.io/badge/-Fork%20this%20repo-blue?style=for-the-badge&logo=github)](https://github.com/yylt/tul/fork)
    
    点击上方的 Fork 按钮将此项目 fork 到您的 GitHub 账户。

2.  **配置 Secrets**
    - 导航到您 fork 的仓库页面
    - 点击顶部的 `Settings` 标签
    - 从左侧边栏选择 `Secrets and variables` -> `Actions`
    - 点击 `New repository secret` 按钮
    - 在 `Name` 输入框中输入 `CLOUDFLARE_API_TOKEN`
    - 将您的 Cloudflare API 令牌粘贴到 `Value` 输入框中
    - 点击 `Add secret` 按钮保存

3.  **触发部署**
    - 转到您 fork 仓库的 `Actions` 标签
    - 从左侧列表中选择名为 **"Deploy"**（或类似名称）的工作流
    - 点击 `Run workflow` 按钮，如果需要请选择分支，然后确认以开始部署
    - 等待工作流完成并检查部署状态

## 🙏 致谢

此项目的实现得益于以下项目的启发和支持：

1.  [tunl](https://github.com/amiremohamadi/tunl)

## 📄 许可证

此项目是开源的，可在 [GNU 许可证](LICENSE) 下使用。