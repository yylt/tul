# tul 

[English](README_en.md) | 中文

轻量级的基于 Cloudflare Worker 代理，使用 Rust/WASM 编写。

## ✨ 特性

🔒 Trojan 代理 - 基于 WebSocket 的 Trojan 协议，后端基于 IP 分流，CF 目标地址会 block

🌐 通用网站镜像 - 支持绝大多数网址的镜像，如遇访问失败网站，建议通过代理访问

🐳 镜像仓库支持 - 默认从 Docker Hub 拉取，并支持任意容器镜像仓库

🔍 安全 DNS 解析 - 支持 DoH（DNS over HTTPS）协议，并对 CF 地址优选

## ⚠️ 注意 
CF worker 读写超时时间 10s, 建议使用断点续传下载大文件。命令如下：

```bash
curl -C - -O [URL]	 # -C - 自动续传，-O 保存为原始文件名
wget -c [URL]	# -c 启用断点续传
```

## 📖 使用指南

### Trojan 代理模式
配置支持 WebSocket 连接的 Trojan 客户端，修改 [v2ray 配置](./hack/config.json) 并运行，注意目标地址是 CF 地址建议配置浏览器直连，详见下文 DoH 模式
```sh
$ v2ray -c ./hack/config.json
```

<details>
<summary><strong>🚨 访问故障排除指南（必读）</strong></summary>

💡 **解决方案**：通常因为目标为 CloudFlare 节点, 或有源地址检查，建议配置 DoH 并直连 CF 节点，如 linux.do, v2ex.com 等

⚠️ **特别注意**：国内 DoH 被污染，请谨慎选择 DoH 服务商

📖 **技术原理**：浏览器使用 ECH 建立 TLS 连接前，会用 DoH 查询 HTTPS 记录

</details>

### 通用网站镜像
代理任何 API/网站 请求

```
# 例子
https://{worker-domain}/github.com/yylt/tul/archive/refs/heads/main.zip
https://{worker-domain}/zh.wikipedia.org
https://{worker-domain}/duckduckgo.com
```

<details>
<summary><strong>⚠️ 访问注意事项</strong></summary>

💡 **问题**：站点无法访问或资源无法加载

⚠️ **原因**：在 JS 检查域名或跨域请求资源

📖 **技术原理**：设置站点 cookie；且对 text/html 的内容进行替换

</details>


### 镜像仓库模式
代理 docker.io 拉取镜像请求
```bash
# 原始请求
docker pull docker.io/library/ubuntu:latest

# 通过代理
docker pull {worker-domain}/library/ubuntu:latest
```

### DoH(安全 DNS 解析) 模式
代理 DNS 查询请求，如访问 cloudflare 代理网站可直连，如 linux.do, v2ex.com 等
```bash

# 测试请求
curl -s "https://1.1.1.1/dns-query?name=v2ex.com&type=A" -H "accept: application/dns-json" 

# 通过代理测试请求
curl -s "https://{worker-domain}/dns-query?name=v2ex.com&type=A" -H "accept: application/dns-json" 
```

配置浏览器使用 DoH 代理，参考[这里](https://help.aliyun.com/document_detail/2868691.html)

建议使用 smartdns + v2ray-rules-dat 对不同域名的解析分流，参考[这里](https://pymumu.github.io/smartdns/config/basic-config/)


## 🚀 快速开始

### 先决条件
拥有 API 访问权限的 Cloudflare 账户密钥

## 🎨 部署

### Fork 并部署（推荐）

1.  **Fork 此仓库**
    [![Fork](https://img.shields.io/badge/-Fork%20this%20repo-blue?style=for-the-badge&logo=github)](https://github.com/yylt/tul/fork)
    
    点击上方的 Fork 按钮将此项目 fork 到您的 GitHub 账户。

2.  **配置 Secrets**
    - 导航到您 fork 的仓库页面
    - 点击顶部的 `Settings` 标签
    - 从左侧边栏选择 `Secrets and variables` -> `Actions`
    - 点击 `New repository secret` 按钮，见[图例](./img/action3.png)
    - 在 `Name` 输入框中输入 `CLOUDFLARE_API_TOKEN`
    - 将您的 Cloudflare API 令牌粘贴到 `Value` 输入框中
    - 点击 `Add secret` 按钮保存
    - (可选)按上述步骤添加密钥 `PASSWORD`(默认 password), `PREFIX`(默认 /tj), `DOH_HOST`(默认 1.1.1.1)

3.  **触发部署**
    - 转到您 fork 仓库的 `Actions` 标签
    - 从左侧列表中选择名为 **"Deploy"**（或类似名称）的工作流，见[图例](./img/action1.png)
    - 点击 `Run workflow` 按钮，如果需要请选择分支，然后确认以开始部署，见[图例](./img/action2.png)
    - 等待工作流完成并检查部署状态

## 🙏 致谢

此项目的实现得益于以下项目的启发和支持：

1.  [tunl](https://github.com/amiremohamadi/tunl)

## 📄 许可证

此项目是开源的，可在 [GNU 许可证](LICENSE) 下使用。