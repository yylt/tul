# EasyTier WSS 协议文档

## 概述

tul 作为 EasyTier 的 WSS 服务端，仅提供**节点注册与发现**功能。服务端负责收集在线节点信息并通知各客户端，节点之间的实际数据通信由 EasyTier 协议层自行完成（直连或通过 EasyTier 内置中继）。

由于 Cloudflare Worker 是分布式无状态的，且单次请求有 CPU 时间限制，**不做数据转发**。

## Cloudflare Worker 限制

Worker 的以下特性决定了 tul 只能做节点发现，不能做数据转发：

| 限制 | 影响 |
|------|------|
| **分布式部署** | 客户端 A 和 B 的 WebSocket 连接可能落在不同的 Worker 实例上，Worker 之间无法直接访问对方的 WebSocket 连接 |
| **CPU 时间限制** | Free 计划每次请求 10ms CPU 时间，Paid 计划 30s（基于 Durable Object）。持续的数据转发会超出限制 |
| **无状态执行** | 单个 Worker 实例的隔离上下文中，无法访问另一个实例的内存 peer map |
| **无出站连接** | Worker 不能主动发起 TCP/UDP 连接，无法作为中继服务端 |

## EasyTier WSS 协议

### 连接建立

EasyTier 客户端通过 `wss://` 或 `ws://` 协议建立隧道连接。底层是一个标准的 WebSocket 连接（RFC 6455），握手完成后双方在 WebSocket 的二进制帧上传输 EasyTier 协议数据。

- **默认端口**：ws=80, wss=443
- **WebSocket path**：URL 中的 path 部分，默认为 `/`。支持自定义 path（如 `wss://tul.example.com/derp`），tul 服务端和客户端配置的 path 必须一致
- **TLS**：wss 模式下由客户端使用自签名证书连接（跳过证书验证）。当 URL 中无域名（如 `wss://1.2.3.4:443`）时，SNI 为 `"localhost"`

### 自定义 Path

EasyTier 的 `WsTunnelConnector` 通过 `ClientBuilder::from_uri()` 将完整 URL（含 path）传递给 WebSocket 握手请求。因此 URL 中的 path 会被原样用于 WebSocket Upgrade 请求。

```
wss://tul.example.com/derp  →  GET /derp HTTP/1.1 + Upgrade: websocket
wss://tul.example.com/      →  GET / HTTP/1.1 + Upgrade: websocket
```

tul 服务端需要：

1. **在 Worker 路由中配置对应的 path**（如 `/derp`），接受该路径下的 WebSocket 升级请求
2. **支持 path 可配**：允许部署者自定义 path，同一部署下所有 EasyTier 客户端需使用相同 path

```
wss://tul.example.com/derp           ← 使用 /derp 路径
```

客户端配置示例：

```toml
[[peer]]
uri = "wss://tul.example.com/derp"
```

### 数据帧格式

每个 WebSocket 二进制帧的内容是 `ZCPacket.tunnel_payload_bytes()`，即去掉外层传输封装后的隧道有效载荷。

数据结构：

```
[PeerManagerHeader: 20 bytes][Payload: N bytes]
```

PeerManagerHeader 布局（小端序）：

| 字段 | 偏移 | 大小 | 类型 | 说明 |
|------|------|------|------|------|
| from_peer_id | 0 | 4 | u32 LE | 发送方节点 ID |
| to_peer_id | 4 | 4 | u32 LE | 目标节点 ID |
| packet_type | 8 | 1 | u8 | 数据包类型 |
| flags | 9 | 1 | u8 | 标志位 |
| forward_counter | 10 | 1 | u8 | 转发计数器（上限 7，防环路） |
| reserved | 11 | 1 | u8 | 保留 |
| len | 12 | 4 | u32 LE | payload 长度 |

**PacketType 枚举：**

| 值 | 类型 | 说明 |
|----|------|------|
| 0 | Invalid | 无效 |
| 1 | Data | 普通数据（丢包可重传） |
| 2 | HandShake | 握手 |
| 4 | Ping | 心跳探测 |
| 5 | Pong | 心跳响应 |
| 8 | RpcReq | RPC 请求 |
| 9 | RpcResp | RPC 响应 |
| 10 | ForeignNetworkPacket | 跨网络包 |
| 13-15 | NoiseHandshakeMsg1/2/3 | Noise 协议握手 |
| 20 | RelayHandshake | 中继握手 |
| 21 | RelayHandshakeAck | 中继握手确认 |

**Flags 位标志：**

| 位 | 常量 | 说明 |
|----|------|------|
| 0x01 | ENCRYPTED | 载荷已加密 |
| 0x02 | LATENCY_FIRST | 延迟优先 |
| 0x04 | EXIT_NODE | 出口节点 |
| 0x08 | NO_PROXY | 不走代理 |
| 0x10 | COMPRESSED | 载荷已压缩 |
| 0x40 | NOT_SEND_TO_TUN | 不投递到 TUN 设备 |

## 架构设计

### 整体架构

```
┌──────────────┐              ┌──────────────────────┐              ┌──────────────┐
│ EasyTier 节点A │              │   tul (Cloudflare)    │              │ EasyTier 节点B │
│ (NAT 后方)    │              │                       │              │ (NAT 后方)    │
│ peer_id=1     │              │   ┌─────────────────┐ │              │ peer_id=2     │
│               │──WSS──▶      │   │  WebSocket 接入  │ │      ◀──WSS─│               │
│               │              │   └────────┬────────┘ │              │               │
│               │              │            │          │              │               │
│               │              │   ┌────────▼────────┐ │              │               │
│               │              │   │   KV 注册/发现   │ │              │               │
│               │              │   └────────┬────────┘ │              │               │
│               │              │            │          │              │               │
│               │              │   ┌────────▼────────┐ │              │               │
│               │              │   │  通知其他节点    │ │              │               │
│               │              │   │  有新节点上线    │ │              │               │
│               │              │   └─────────────────┘ │              │               │
└──────────────┘              └──────────────────────┘              └──────────────┘
                                   │
                                   │ 节点发现结果
                                   ▼
┌──────────────────────────────────────────────────────────────────────────────┐
│  EasyTier 协议层自行处理：                                                     │
│  节点A 和 B 通过 EasyTier 内置中继或直接尝试打洞连接，tul 不参与数据转发        │
└──────────────────────────────────────────────────────────────────────────────┘
```

### 核心组件

#### 1. WebSocket 接入层

tul 接收 EasyTier 客户端的 WSS 连接。连接建立流程：

1. 接收 HTTP Upgrade 请求，升级到 WebSocket
2. 等待客户端发送 HandShake 帧（packet_type=2）
3. 从 HandShake 帧中解析出 network_name 和 peer_id
4. 将节点信息写入 KV
5. 保持 WebSocket 连接活跃，仅用于接收后续的 HandShake/Ping 等控制帧，不做数据帧转发

#### 2. 节点注册与发现（KV）

使用 Cloudflare KV 存储节点注册信息。由于 Worker 实例之间无法直接通信，KV 是唯一的跨实例数据共享方式。

**KV Key 格式：**

| Key | Value | TTL |
|-----|-------|-----|
| `peers:{network_name}` | JSON: `{"peer_id": timestamp, ...}` | 300 秒（5 分钟） |

**注册流程：**

1. 客户端建立 WSS 连接后，tul 从 HandShake 帧提取 peer_id 和 network_name
2. 写入 KV：`peers:{network_name}` key 下追加/更新该 peer_id 的记录
3. 客户端定期发送 Ping 帧，tul 收到后刷新 KV TTL
4. WebSocket 断开时从 KV 中删除对应记录

**发现流程：**

1. 新节点上线时，tul 读取 KV 中同 network_name 下的所有在线 peer_id
2. 排除节点自身，将其他节点的 peer_id 列表通过 WebSocket 发送给新节点
3. 同时，向同网络下其他在线节点发送新节点上线的通知

#### 3. 不负责数据转发

tul **不转发** EasyTier 数据帧（packet_type=Data(1) 等业务数据包）。原因：

- Worker 分布式部署：A 和 B 的 WebSocket 连接在不同实例上，无法互写
- CPU 时间限制：持续转发会超出限制
- 数据转发由 EasyTier 内置中继机制完成，不需要外部服务端参与

tul 仅处理以下控制帧：

| PacketType | tul 行为 |
|------------|---------|
| HandShake(2) | 注册节点到 KV，查询并返回同网络其他节点列表 |
| Ping(4) | 刷新 KV TTL，保持注册有效 |
| 其他 | 忽略（不解析、不转发） |

## EasyTier 客户端配置

### 配置方式

EasyTier 客户端通过 `peer` 配置项连接到 tul 的 WSS 端点。

**配置文件（TOML）：**

```toml
[network_identity]
network_name = "my-network"
network_secret = "your-secret"

# 将 tul 作为 peer，连接后获取同网络下其他节点信息
[[peer]]
uri = "wss://tul.example.com/"

# 本地监听，允许其他节点直连
[listeners]
listeners = ["tcp://0.0.0.0:11010", "udp://0.0.0.0:11010"]
```

**命令行参数：**

```bash
easytier --peer "wss://tul.example.com/" --network-name my-network --network-secret your-secret
```

### 典型部署场景

```
节点A (NAT后) ──WSS──┐
                     │    ┌─────────────────────────┐
节点B (NAT后) ──WSS──┼───▶│  tul Worker (节点发现)    │
                     │    │  仅做注册/发现，不转发    │
节点C (NAT后) ──WSS──┘    └──────────┬──────────────┘
                                     │
                                     │ 各节点通过发现结果
                                     │ 自行建立连接
                                     ▼
                          ┌─────────────────────┐
                          │ EasyTier 内置中继     │
                          │ 或 NAT 打洞直连       │
                          └─────────────────────┘
```

所有节点配置相同的 peer URL。节点通过 tul 发现彼此后，EasyTier 协议层会自动：
1. 尝试 UDP/TCP NAT 打洞建立直连
2. 打洞失败时通过 EasyTier 内置中继节点（如有公网节点）转发数据

配置：
```toml
[network_identity]
network_name = "default"
network_secret = ""

[[peer]]
uri = "wss://tul.example.com/"
```

## 限制与注意事项

### Cloudflare Worker 限制

| 限制 | 影响 | 应对 |
|------|------|------|
| 分布式部署 | 不同客户端可能连接不同 Worker 实例 | 仅通过 KV 做节点发现，不跨实例转发数据 |
| CPU 时间限制 | Free: 10ms/请求, Paid: 30s(Durable Object) | 仅处理控制帧，不处理业务数据 |
| 内存限制 10MB (Free) / 50MB (Paid) | 活跃连接数受限 | 每个 WS 连接约消耗数 KB |
| 无出站连接 | 无法作为中继服务端 | 数据转发由 EasyTier 内置中继完成 |
| KV 最终一致性 | 节点注册信息可能有短暂延迟（最多 60s） | 可接受，节点发现允许一定延迟 |

### EasyTier 协议限制

| 限制 | 说明 |
|------|------|
| 自签名 TLS 证书 | wss 连接跳过证书验证，公网部署建议绑定真实域名并配置有效证书 |
| peer_id 动态分配 | peer_id 由 EasyTier 内部生成，服务端从 HandShake 帧中获取 |
| 中继 hop 上限 7 跳 | 多层中继场景下需注意跳数限制 |
| 仅 binary frame | 非 binary 的 WebSocket 帧将被 EasyTier 客户端拒绝 |
| 节点发现依赖内置中继 | EasyTier 客户端从 tul 获取节点列表后，仍需要有中继节点或打洞能力才能互通 |

### 服务端实现要点

1. **自定义 path 支持**：tul Worker 路由需支持可配置的 path（如 `/derp`），客户端 URL 中的 path 会原样用于 WebSocket Upgrade 请求的 `GET` 路径，tul 必须在对应路径上接受 WebSocket 升级
2. **只做控制帧**：仅处理 HandShake(2) 和 Ping(4) 帧，忽略 Data(1) 等业务数据帧
3. **KV 作为共享状态**：所有 Worker 实例通过 KV 读写节点注册信息，解决分布式无状态问题
4. **HandShake 注册**：连接建立后从第一个 HandShake 帧提取 network_name 和 peer_id
5. **Ping 保活**：收到 Ping 帧时刷新 KV TTL，保持节点在线标记
6. **连接断开清理**：WebSocket 断开时从 KV 中移除对应节点记录
7. **上线通知**：新节点注册后，向同网络下已在线节点发送新节点 peer_id 通知
