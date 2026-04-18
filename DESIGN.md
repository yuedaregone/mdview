# 极简 Markdown 阅读器 — 实现方案

> Rust + egui，参考 Ferrite 思路，纯阅读器，不照搬代码
>
> **项目路径**：`/workspace/mdview`
> **窗口**：系统原生标题栏（decorated: true，不自绘）

---

## 1. 项目结构

```
mdview/
├── Cargo.toml
├── build.rs                    # Windows 图标嵌入
├── assets/
│   └── icon.ico                # 应用图标
└── src/
    ├── main.rs                 # 入口：CLI 参数 → 启动 eframe
    ├── app.rs                  # eframe::App 实现，应用状态
    ├── markdown/
    │   ├── mod.rs
    │   ├── parser.rs           # comrak MD → 自定义 AST
    │   ├── renderer.rs         # AST → egui UI 映射（核心）
    │   ├── highlight.rs        # syntect 代码高亮
    │   └── cache.rs            # AST 缓存 + block 高度缓存
    ├── theme/
    │   ├── mod.rs              # 主题管理器 + Theme struct
    │   └── presets.rs          # 8-10 套预设主题定义
    ├── widgets/
    │   ├── mod.rs
    │   ├── code_block.rs       # 代码块 widget
    │   ├── table.rs            # 表格 widget
    │   ├── image.rs            # 图片 widget（本地 + 网络）
    │   └── quote.rs            # 引用块 widget
    ├── selection.rs            # 文本选择 + 复制
    ├── context_menu.rs         # 右键菜单
    ├── image_loader.rs         # 异步图片加载器
    ├── file_handler.rs         # 文件关联注册 + 拖拽处理
    └── viewport.rs             # 视口裁剪
```

**预估代码量**：~6,000-8,000 行

---

## 2. 核心数据结构

### 2.1 自定义 AST（简化版，只读不写）

```rust
/// Markdown 文档的顶层结构
struct MarkdownDoc {
    nodes: Vec<DocNode>,        // 顶层 block 节点
    source_hash: u64,           // 内容哈希，用于缓存校验
}

/// 文档节点（只包含阅读器需要的类型）
enum DocNode {
    Heading { level: u8, children: Vec<InlineNode> },
    Paragraph(Vec<InlineNode>),
    CodeBlock { lang: String, code: String },
    Table { headers: Vec<TableCell>, rows: Vec<Vec<TableCell>>, aligns: Vec<Align> },
    BlockQuote(Vec<DocNode>),
    OrderedList { start: u64, items: Vec<ListItem> },
    UnorderedList(Vec<ListItem>),
    TaskList { items: Vec<TaskItem> },
    ThematicBreak,
    Image { url: String, alt: String, title: String },
    HtmlBlock(String),          // 原始 HTML 块（简单展示）
    FootnoteDef { label: String, content: Vec<DocNode> },
}

enum InlineNode {
    Text(String),
    Bold(Vec<InlineNode>),
    Italic(Vec<InlineNode>),
    Strikethrough(Vec<InlineNode>),
    Code(String),
    Link { url: String, children: Vec<InlineNode> },
    Image { url: String, alt: String },
    SoftBreak,
    HardBreak,
    FootnoteRef(String),
}
```

**设计要点**：
- 不用 Ferrite 的 `Vec<MarkdownNode>` 索引树方案（太复杂），用简单的嵌套 enum
- 去掉所有编辑相关节点（Callout、Wikilink 等编辑器特有类型）
- `DocNode` 只有 block 级别，`InlineNode` 只有 inline 级别，分层清晰

### 2.2 主题

```rust
struct Theme {
    name: &'static str,
    is_dark: bool,
    // 基础色
    background: Color32,
    foreground: Color32,
    // 语义色
    heading: Color32,
    link: Color32,
    link_hover: Color32,
    code_bg: Color32,
    code_fg: Color32,
    quote_border: Color32,
    quote_fg: Color32,
    table_border: Color32,
    table_header_bg: Color32,
    table_stripe_bg: Option<Color32>,
    hr_color: Color32,
    list_marker: Color32,
    task_checked: Color32,
    task_unchecked: Color32,
    selection_bg: Color32,      // 文本选区背景
    // 代码高亮（syntect theme name）
    syntax_theme: &'static str,
}
```

### 2.3 应用状态

```rust
struct App {
    // 文件
    file_path: Option<PathBuf>,
    doc: Option<MarkdownDoc>,
    
    // 渲染
    theme: &'static Theme,
    font_size: f32,             // 基准字体大小
    scroll_offset: f32,
    
    // 缓存
    ast_cache: AstCache,
    block_cache: BlockHeightCache,
    image_cache: ImageCache,
    highlight_cache: HighlightCache,
    
    // 选择
    selector: TextSelector,
    
    // 视口裁剪
    viewport: ViewportState,
    
    // 异步
    image_loader: ImageLoader,
}
```

---

## 3. 渲染管线详细设计

### 3.1 启动流程

```
main.rs:
  1. 解析 CLI 参数，获取文件路径
  2. 如果有文件路径，立即读取 + comrak 解析 → MarkdownDoc
  3. 配置 eframe::NativeOptions:
     - initial_window_size: [800, 600]
     - decorated: true（使用系统标题栏，不做自定义）
     - follow_system_theme: false（自己管理主题）
     - visible: false（窗口初始隐藏）
  4. run_native() 启动
```

```
app.rs update():
  首帧:
    1. 渲染完整文档到 egui layout
    2. 调用 ctx.send_viewport_cmd(Visible(true)) 显示窗口
  后续帧:
    1. 检查是否有新文件需要加载（拖拽/CLI）
    2. 视口裁剪：只渲染可见 block
    3. 绘制滚动条
    4. 处理右键菜单
```

### 3.2 AST→egui 渲染映射

核心函数签名：

```rust
fn render_doc(
    ui: &mut Ui,
    doc: &MarkdownDoc,
    theme: &Theme,
    font_size: f32,
    viewport: &mut ViewportState,
    selector: &mut TextSelector,
    image_loader: &mut ImageLoader,
)
```

渲染策略：
- **顶层 block 遍历**：遍历 `doc.nodes`，每个 DocNode 对应一个渲染函数
- **视口裁剪**：跳过不在可见区域的 block（基于缓存的 Y 位置 + 高度）
- **inline 渲染**：`InlineNode` 通过 `egui::RichText` + `LayoutJob` 渲染

关键渲染函数：

```rust
fn render_heading(ui, node, theme, font_size)
fn render_paragraph(ui, inlines, theme, font_size, selector)
fn render_code_block(ui, lang, code, theme, font_size)
fn render_table(ui, headers, rows, aligns, theme, font_size)
fn render_block_quote(ui, children, theme, font_size, selector)
fn render_list(ui, items, ordered, theme, font_size, selector)
fn render_image(ui, url, alt, theme, image_loader)
fn render_inline(ui, node, theme, font_size) -> RichText
```

### 3.3 视口裁剪

```rust
struct ViewportState {
    /// 每个顶层 block 的 Y 位置和高度
    block_layouts: Vec<BlockLayout>,
    /// 文档内容总高度
    total_height: f32,
    /// 内容哈希，变化时清除缓存
    content_hash: u64,
    /// 当前视口内需要渲染的 block 索引范围
    visible_range: Range<usize>,
}

struct BlockLayout {
    y: f32,
    height: f32,
    measured: bool,  // 是否已经过真实渲染测量
}
```

**流程**：
1. 首次渲染：所有 block 用估算高度（行数 × 行高）
2. 每帧只真实测量可见 block，逐步替换估算值
3. 二分查找定位可见范围，上下各扩展 500px overscan
4. 测量完成后请求重绘（`ctx.request_repaint()`）

### 3.4 文本选择

简化方案（对比 Ferrite 的多光标选择）：

```rust
struct TextSelector {
    selecting: bool,
    /// 选区起点（文档坐标）
    start: Option<Pos2>,
    /// 选区终点
    end: Option<Pos2>,
    /// 选中的纯文本
    selected_text: String,
}
```

**实现思路**：
1. 不在 AST 层面做选择，而是在**渲染层**记录每个文本段的屏幕位置
2. 维护一个 `Vec<TextSegment>` 记录本帧渲染的所有文本的位置和内容
3. 鼠标拖拽时，用矩形范围匹配 TextSegment，提取纯文本
4. 渲染选区时，对选中 TextSegment 绘制半透明背景

```rust
struct TextSegment {
    rect: Rect,           // 屏幕位置
    text: String,         // 纯文本内容
    block_index: usize,   // 所属 block 索引
}
```

### 3.5 图片加载

```rust
struct ImageLoader {
    /// 已加载的图片纹理
    textures: HashMap<String, ImageState>,
    /// 文档所在目录（用于解析相对路径）
    base_dir: PathBuf,
    /// 异步加载的消息通道
    rx: mpsc::Receiver<ImageLoadResult>,
}

enum ImageState {
    Loading,
    Ready(TextureHandle),
    Failed,
}
```

**加载流程**：
1. 渲染时调用 `image_loader.get(url)` 
2. 如果缓存中有 → 直接返回
3. 如果没有 → 启动线程加载（本地文件 or HTTP 下载），返回 Loading
4. 每帧检查 `rx` 通道，将加载完成的图片转为 TextureHandle 存入缓存
5. Loading 状态显示灰色占位 + 动画，Failed 显示错误提示

**支持的图片源**：
- 本地文件：相对路径（基于 .md 所在目录）、绝对路径
- HTTP/HTTPS：异步下载到内存，不解码到磁盘
- data URI：解码 base64 内嵌数据

---

## 4. 各模块实现要点

### 4.1 markdown/parser.rs

- 调用 comrak `parse_document` 获取 comrak AST
- 递归遍历 comrak AST，转换为自定义 `MarkdownDoc`
- 只提取阅读器需要的节点类型，忽略编辑器特有的
- GFM 扩展全部开启：table, strikethrough, autolink, tasklist, footnotes

### 4.2 markdown/renderer.rs

- `render_doc()` 作为顶层入口
- `render_block()` 处理单个 block 节点
- `render_inlines()` 处理 inline 节点列表，返回 `LayoutJob`
- 每个渲染函数接收 `&Theme` 和 `font_size`，颜色和字号全部参数化
- 收集 `TextSegment` 到 `Vec`，供文本选择使用

### 4.3 markdown/highlight.rs

- 全局 `OnceLock<SyntaxHighlighter>`
- 使用 `syntect::SyntaxSet::load_defaults_nonewlines()` + `two-face::SyntaxSetBuilder`
- 预加载默认 ThemeSet
- 高亮结果缓存：以 `(code_hash, lang, theme_name)` 为键

### 4.4 markdown/cache.rs

```rust
/// AST 缓存（key = 文件路径 + 内容哈希）
struct AstCache {
    entries: HashMap<PathBuf, CachedAst>,
    max_entries: usize,  // 默认 16
}

/// Block 高度缓存（key = block 内容哈希 + 渲染参数哈希）
struct BlockHeightCache {
    entries: HashMap<u64, f32>,  // hash → height
    max_entries: usize,          // 默认 256
}
```

### 4.5 widgets/code_block.rs

- 接收 syntect 高亮后的 `Vec<StyledSpan>` 
- 用 `egui::RichText` 逐 span 渲染
- 代码块有圆角背景色
- 横向可滚动（代码行过长时）
- 右上角显示语言标签
- 点击代码块内部时选中全部代码文本（方便复制）

### 4.6 widgets/table.rs

- 使用 `egui::Grid` 布局
- 支持列对齐（left/center/right）
- 表头背景色 + 斑马纹
- 横向可滚动
- 单元格内支持 inline 渲染（加粗、链接等）

### 4.7 widgets/image.rs

- 三种状态渲染：Loading / Ready / Failed
- Ready 状态：按比例缩放，不超过视口宽度 90%
- hover 时显示 tooltip（alt + url）
- 点击图片可放大查看（可选，优先级低）

### 4.8 widgets/quote.rs

- 左侧 3px 彩色边框线
- 内容缩进 16px
- 嵌套引用使用不同颜色边框
- 内部支持任意 DocNode 渲染

### 4.9 theme/presets.rs

预设 8 套主题：

| 主题 | 亮/暗 | 说明 |
|------|:---:|------|
| GitHub Light | 亮 | GitHub 风格 |
| GitHub Dark | 暗 | GitHub Dark 风格 |
| One Dark | 暗 | Atom/VS Code One Dark |
| Solarized Light | 亮 | Solarized 经典 |
| Solarized Dark | 暗 | Solarized Dark |
| Dracula | 暗 | Dracula 配色 |
| Nord | 暗 | Nord 配色 |
| Catppuccin Latte | 亮 | Catppuccin 浅色 |

每个主题约 30 行代码定义所有颜色值。

### 4.10 context_menu.rs

使用 egui 原生 `context_menu()`：

```
右键菜单
├── 复制文本              → selector.selected_text → arboard 剪贴板
├── ─────────────────
├── 字体大小 ▶
│   ├── 12px
│   ├── 14px
│   ├── 16px ✓
│   ├── 18px
│   └── 20px
├── 切换主题 ▶
│   ├── GitHub Light ✓
│   ├── GitHub Dark
│   ├── One Dark
│   ├── ...
│   └── Nord
├── ─────────────────
└── 打开文件目录          → Explorer /select,"path"
```

### 4.11 file_handler.rs

**文件关联**（Windows）：
- 写入注册表 `HKCU\Software\Classes\.md\shell\open\command`
- 值为 `"path\to\mdview.exe" "%1"`
- 提供 `--register` 和 `--unregister` 命令行参数

**拖拽**：
- eframe 通过 winit 支持 `ctx.input().raw.dropped_files`
- 支持 .md / .markdown / .txt 文件
- 拖入时重新加载文档

---

## 5. 分步实现计划

### Step 1: 项目骨架（1天）

- `cargo init mdview`
- 配置 Cargo.toml（依赖：eframe, comrak, syntect, two-face, image, open, arboard）
- 配置 release profile（lto="thin", codegen-units=1, panic="abort", strip=true）
- main.rs：CLI 参数解析 + eframe 启动 + 窗口初始隐藏
- app.rs：空壳 App trait 实现

**验证**：`cargo run` 弹出空白窗口

### Step 2: Markdown 解析（2-3天）

- parser.rs：comrak AST → MarkdownDoc 转换
- 覆盖所有 GFM 节点类型
- 编写单元测试：用各种 .md 文件测试解析正确性

**验证**：CLI 模式打印 MarkdownDoc 结构

### Step 3: 基础渲染（3-5天）

- renderer.rs：AST → egui UI 映射
- 先实现最基础的节点：Heading, Paragraph, ThematicBreak, CodeBlock(无高亮)
- 内联节点：Text, Bold, Italic, Code, Link, SoftBreak, HardBreak
- ScrollArea 包裹，支持滚动
- 首帧渲染完成后显示窗口

**验证**：打开 .md 文件，能看到标题、段落、粗体、斜体、代码块

### Step 4: 代码高亮（2天）

- highlight.rs：syntect 集成
- widgets/code_block.rs：高亮代码块 widget
- 缓存高亮结果

**验证**：代码块有语法着色

### Step 5: 表格 + 列表 + 引用（2-3天）

- widgets/table.rs：Grid 布局表格
- 有序/无序列表渲染
- 任务列表（checkbox 显示）
- widgets/quote.rs：引用块

**验证**：GFM 全特性基本可渲染

### Step 6: 主题系统（2天）

- theme/mod.rs：Theme struct + 切换逻辑
- theme/presets.rs：8 套预设主题
- 主题切换时同步更新 egui Visuals

**验证**：可以切换 8 套主题

### Step 7: 图片加载（3天）

- image_loader.rs：异步图片加载器
- widgets/image.rs：图片 widget
- 支持本地文件 + HTTP + data URI

**验证**：Markdown 中的图片能正常显示

### Step 8: 文本选择 + 复制（3-5天）

- selection.rs：TextSelector 实现
- 收集 TextSegment
- 鼠标拖拽选择
- arboard 剪贴板复制

**验证**：可以选中文本并复制

### Step 9: 右键菜单（1天）

- context_menu.rs：egui popup 菜单
- 复制文本、字体大小、切换主题、打开文件目录

**验证**：右键菜单功能正常

### Step 10: 视口裁剪（2天）

- viewport.rs：ViewportState
- block 位置缓存 + 二分查找
- 渐进式测量

**验证**：大文件滚动流畅

### Step 11: 文件关联 + 拖拽（1-2天）

- file_handler.rs：注册表写入 + 拖拽处理
- `--register` / `--unregister` 参数

**验证**：双击 .md 文件直接打开，拖拽文件到窗口

### Step 12: 缓存 + 打磨（3-5天）

- cache.rs：AST 缓存 + block 高度缓存
- 性能测试和优化
- 边界情况修复
- Windows 图标嵌入

**预估总周期**：4-6 周

---

## 6. 性能优化策略

| 策略 | 说明 |
|------|------|
| 窗口初始隐藏 | 渲染完成前不显示，用户感知为"瞬间打开" |
| 视口裁剪 | 只渲染可见 block + 500px overscan |
| AST 缓存 | 文件内容不变时复用解析结果 |
| 高亮缓存 | 同一段代码不重复高亮 |
| 图片异步加载 | 不阻塞 UI 线程 |
| 渐进式测量 | 首帧用估算，逐步替换为精确值 |
| comrak 提前解析 | 在 eframe 初始化之前完成 MD 解析 |
| release 优化 | LTO + codegen-units=1 + panic=abort + strip |

---

## 7. 关键依赖版本

```toml
[dependencies]
eframe = "0.28"           # GUI 框架
egui = "0.28"             # 即时模式 UI
comrak = "0.22"           # Markdown 解析
syntect = "5.1"           # 代码高亮
two-face = "0.5"          # 扩展语法集
image = "0.25"            # 图片解码
open = "5"                # 系统浏览器打开链接
arboard = "3"             # 剪贴板
clap = { version = "4", features = ["derive"] }  # CLI

[profile.release]
lto = "thin"
codegen-units = 1
panic = "abort"
opt-level = 3
strip = true
```
