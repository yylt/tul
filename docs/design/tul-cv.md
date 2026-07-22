# tul_cv 页面设计提案

## 概述

`/tul_cv` 是一个纯前端 WASM 工具页面，在浏览器本地完成文本、图像的转换和处理。所有计算在客户端执行，Worker 仅提供静态 HTML/WASM 分发，不参与数据处理。

---

## 页面布局

居中单列布局，自上而下：

```
┌─────────────────────────┐
│         搜索框           │  全局搜索，输入关键词实时过滤下方能力项
├─────────────────────────┤
│  All │ 文本类 │ 图像类 │ 单位  │  tab 切换
├─────────────────────────┤
│                         │
│   能力卡片网格（2列）     │  根据 tab 或搜索结果展示对应能力
│   ┌─────┐ ┌─────┐      │
│   │ PNG │ │ JPEG│      │  每张卡片：图标 + 名称 + 描述
│   │ →JPEG│ │ →PNG│      │
│   └─────┘ └─────┘      │
│   ┌─────┐ ┌─────┐      │
│   │ OCR │ │水印 │      │
│   └─────┘ └─────┘      │
│                         │
└─────────────────────────┘
```

点击能力卡片后进入对应工具的独立面板（输入区 + 参数 + 输出预览 + 下载按钮）。

---

## Tab 分类与能力清单

### 1. 所有（默认）
展示全部能力，搜索模式下仅显示匹配项。

### 2. 图像类

| 能力 | 描述 | 实现方式 |
|------|------|---------|
| JPEG → PNG | 转换格式 | `image` crate |
| PNG → JPEG | 转换格式，可调质量 | `image` crate |
| JPEG → WebP | 转换格式 | `image` crate |
| WebP → PNG | 转换格式 | `image` crate |
| PNG → WebP | 转换格式 | `image` crate |
| WebP → JPEG | 转换格式 | `image` crate |
| 图像缩放 | 按比例或指定宽高缩放 | `image` crate |
| 图像裁剪 | 指定区域裁剪 | `image` crate |
| 添加水印 | 文字或图片水印叠加 | `image` + `imageproc` |
| 文字识别（OCR） | 从图片中提取文字 | 调用浏览器内置 OCR API（如存在）或轻量 WASM OCR |
| SVG → PNG | 矢量图转位图 | `resvg` + `tiny-skia` |

### 3. 文本类

| 能力 | 描述 | 实现方式 |
|------|------|---------|
| 文本 → PDF | 纯文本转 PDF 文档 | `pdf-writer` |
| 文本编码转换 | UTF-8/GBK/Base64/Hex 互转 | 纯 Rust std / 自定义 |
| JSON 格式化 / 压缩 | 格式化或压缩 JSON | `serde_json` |
| Markdown → HTML | Markdown 渲染 | `pulldown-cmark` |
| 文本去重 / 排序 | 行去重、字母/数字排序 | 纯 Rust |
| CSV → JSON | 格式转换 | `csv` + `serde_json` |
| Excel → CSV/JSON | 读取 .xlsx | `calamine` |
| 生成 Word | 文本/图片转 .docx | `docx-rs` |
| PDF 提取文本 | 从 PDF 中读取文字 | `lopdf` |

### 4. 单位转换

| 能力 | 描述 |
|------|------|
| 长度 | m/km/cm/mm/in/ft/yard |
| 重量 | kg/g/mg/lb/oz |
| 温度 | °C/°F/K |
| 面积 | m²/km²/ha/acre/ft² |
| 体积 | L/mL/m³/gal |
| 速度 | km/h mph m/s knot |
| 数据大小 | B/KB/MB/GB/TB/PB |
| 时间 | s/min/h/day/week/month/year |

---

## 技术架构

### 总体方案

```
┌─────────────────────────────────────────┐
│  Cloudflare Worker (tul)                 │
│  GET /tul_cv → 返回 HTML 页面            │
│  GET /tul_cv.wasm → 返回 WASM 二进制     │
│  GET /tul_cv.js → 返回 JS 胶水代码       │
└─────────────────────────────────────────┘
         │
         │ 浏览器加载
         ▼
┌─────────────────┐     ┌──────────────────┐
│  tul_cv UI      │────▶│  WASM 模块        │
│  (HTML/CSS/JS)  │     │  (Rust→wasm)     │
│                 │◀────│  - image 转换     │
│  - 搜索         │     │  - 文本处理       │
│  - Tab 切换     │     │  - 单位转换       │
│  - 拖拽上传     │     │  - PDF 生成/解析  │
│  - 预览下载     │     │  - Excel 读取     │
└─────────────────┘     └──────────────────┘
```

### 项目结构

```
tul/                          # 现有 Worker 项目
├── src/
│   ├── proxy/                # 现有代理逻辑（不动）
│   ├── html/                 # HTML 模板（统一存放）
│   │   ├── ip.html
│   │   ├── tul_dl.html
│   │   └── cv.html           # [新增] 页面模板
│   └── tools/
│       ├── mod.rs            # [新增] 工具模块入口
│       └── cv.rs             # [新增] /tul_cv 页面 handler
└── Cargo.toml                # 不新增依赖

tul-cv-wasm/                  # [新增] 独立 bin 项目，编译为 WASM
├── Cargo.toml
├── src/
│   ├── lib.rs                # wasm-bindgen 入口，暴露 JS API
│   ├── image.rs              # 图像处理（格式转换、缩放、裁剪、水印）
│   ├── text.rs               # 文本处理（编码、格式化、Markdown）
│   ├── pdf.rs                # PDF 生成（pdf-writer）/ 解析（lopdf）
│   ├── office.rs             # Excel（calamine）+ Word（docx-rs）
│   ├── unit.rs               # 单位转换
│   └── ocr.rs                # OCR（如有轻量方案）
├── pkg/                      # wasm-pack build 输出
└── Makefile
```

### WASM 模块设计

- **编译目标**: `wasm32-unknown-unknown`
- **构建工具**: `wasm-pack build --target web`
- **JS 胶水层**: 由 `wasm-bindgen` 自动生成
- **分发方式**: 构建产物（`tul_cv_bg.wasm` + `tul_cv.js`）托管到独立的 JS 仓库（如 `tul-cv-js`），通过 npm/unpkg/CDN 分发，Worker 页面通过 `<script>` 引入

### Worker 端（最小改动）

在 `src/lib.rs` 中注册 `/tul_cv` 路由：

```rust
// src/tools/cv.rs
pub async fn handler(_req: &Request) -> Result<Response> {
    let html = include_str!("../html/cv.html");
    Ok(Response::from_html(html)?)
}
```

HTML 页面中通过 `<script type="module">` 加载 WASM 模块。

### 操作流程（以图像转换为例）

```
1. 用户在卡片点击 "PNG → JPEG"
2. UI 展示该工具面板：拖拽上传区 + 质量滑块 + 预览区
3. 用户拖入/选择 PNG 文件
4. JS 读取文件为 ArrayBuffer → 传给 WASM
5. WASM (image crate) 解码 PNG → 编码为 JPEG
6. 返回 JPEG bytes 给 JS
7. JS 创建 Blob URL → 显示预览图 + 下载按钮
```

---

## 依赖库选型

| 库 | 用途 | WASM 兼容 | 注意事项 |
|----|------|-----------|---------|
| `image` v0.25 | 图片格式互转、缩放、裁剪 | ✅ | 默认特性全开；禁用默认 feature，按需开启 PNG/JPEG/WebP |
| `imageproc` | 水印叠加、文字渲染 | ⚠️ | 依赖 `image` + `rusttype`，注意编译体积 |
| `wasm-bindgen` | Rust↔JS 互操作 | ✅ | 核心胶水层 |
| `web-sys` | 浏览器 API 绑定 | ✅ | 如需要 FileReader 等 |
| `js-sys` | JS 类型绑定 | ✅ | ArrayBuffer/Uint8Array 互转 |
| `pdf-writer` | 生成 PDF | ✅ | 纯 Rust |
| `lopdf` | 解析 PDF | ✅ | 纯 Rust，体积较大 |
| `calamine` | 读取 Excel (.xlsx) | ✅ | 纯 Rust |
| `docx-rs` | 生成 .docx | ✅ | 纯 Rust |
| `pulldown-cmark` | Markdown→HTML | ✅ | 轻量 |
| `resvg` | SVG→PNG | ✅ | 依赖 tiny-skia，编译慢但可行 |
| `serde` + `serde_json` | JSON 处理 | ✅ | 已在 Worker 中 |
| `csv` | CSV 解析 | ✅ | 轻量 |

体积控制策略：
- `image` 仅开启 `png`, `jpeg`, `webp` codec
- 放弃 `lopdf` 若体积过大，用纯文本方案代替
- 考虑按功能拆分为多个小 WASM，按需加载（code splitting）

---

## 本地调试

### 1. WASM 模块调试

在 `tul-cv-wasm/` 目录下，使用 `wasm-pack` 构建并启动本地开发服务器：

```bash
cd tul-cv-wasm

# 安装 wasm-pack（如未安装）
cargo install wasm-pack

# 构建 WASM（开发模式，不优化）
wasm-pack build --target web --dev

# 构建产物在 pkg/ 目录：
#   pkg/tul_cv_bg.wasm   — WASM 二进制
#   pkg/tul_cv.js         — JS 胶水代码
#   pkg/tul_cv.d.ts       — TypeScript 类型声明
```

### 2. 本地前端调试

创建 `tul-cv-wasm/www/` 目录作为前端调试入口：

```
tul-cv-wasm/
├── www/
│   ├── index.html         # 带搜索框 + tab + 卡片网格的完整 UI
│   └── package.json       # 可选，使用 webpack/vite 热更新
```

`index.html` 示例：

```html
<!DOCTYPE html>
<html>
<head><meta charset="utf-8"></head>
<body>
  <input id="file" type="file">
  <script type="module">
    import init, { png_to_jpeg } from '../pkg/tul_cv.js';
    await init();
    document.getElementById('file').onchange = async (e) => {
      const buf = await e.target.files[0].arrayBuffer();
      const result = png_to_jpeg(new Uint8Array(buf), 80);
      // 创建 Blob 下载或预览
    };
  </script>
</body>
</html>
```

启动静态文件服务：

```bash
# 方式一：Python（最简单）
python3 -m http.server 8080 -d tul-cv-wasm/

# 方式二：使用 serve
cd tul-cv-wasm && npx serve .

# 访问 http://localhost:8080/www/
```

### 3. 与 Worker 联调

将 WASM 构建产物复制到 tul 项目中，通过 `wrangler dev` 联调：

```bash
# 构建 WASM（release）
cd tul-cv-wasm
wasm-pack build --target web

# 复制产物到 tul（临时联调）
cp pkg/*.wasm pkg/*.js ../tul/src/html/

# 在 cv.html 中本地引用（调试阶段）
# <script type="module">
#   import init from './tul_cv.js';
#   import initBg from './tul_cv_bg.wasm';
#   await init(initBg);
# </script>

# 终端 1：启动 Worker
cd ../tul && make dev

# 终端 2：访问测试
curl http://localhost:8787/tul_cv
```

正式部署时改为 CDN 引用路径。

### 4. 常用调试命令

```bash
# 开发模式快速构建（跳过优化）
wasm-pack build --target web --dev

# 查看 WASM 体积
ls -lh pkg/*.wasm

# 开启调试符号
wasm-pack build --target web --dev -- -g

# twiggy 分析 WASM 体积分布
cargo install twiggy
twiggy top -n 20 pkg/tul_cv_bg.wasm

# wasm-opt 进一步压缩（生产环境）
wasm-opt -Oz pkg/tul_cv_bg.wasm -o pkg/tul_cv_bg.wasm
```

---

## 风险与待确认

1. **WASM 体积**: `image` + codecs 大约 300-500KB（gzip 后 150-250KB），首屏加载需控制
2. **OCR 能力**: 浏览器原生 OCR API 尚未标准化，可靠方案需引入 `tesseract-wasm`（体积大 ~10MB），建议初期不做 OCR 或标注"开发中"
3. **SVG 渲染**: `resvg` 编译慢且增加 ~1MB 体积，可延后
4. **PDF 解析**: `lopdf` 功能强但体积大，优先做 PDF 生成（pdf-writer 较小）
5. **JS 仓库分发**: 确定使用 jsdelivr CDN 还是独立的 npm 包

---

## 实施计划

### Phase 1: 基础设施
- [ ] 创建 `tul-cv-wasm` 独立项目，配置 `wasm-pack`
- [ ] 搭建基本 UI（`cv.html`：搜索框 + tab + 卡片网格）
- [ ] 实现 image crate 核心转换（PNG↔JPEG↔WebP）
- [ ] 在 tul 中注册 `/tul_cv` 路由

### Phase 2: 核心能力
- [ ] 图像缩放、裁剪
- [ ] 水印叠加
- [ ] 文本编码转换
- [ ] JSON 格式化/压缩
- [ ] Markdown→HTML
- [ ] 单位转换

### Phase 3: 文档类
- [ ] 文本→PDF
- [ ] Excel→CSV/JSON
- [ ] 生成 Word
- [ ] CSV→JSON

### Phase 4: 进阶 & 分发
- [ ] SVG→PNG（可选）
- [ ] PDF 文本提取（可选）
- [ ] OCR（可选，如方案可行）
- [ ] 发布 WASM 到 JS 仓库，配置 CDN 分发
