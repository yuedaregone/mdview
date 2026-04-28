# mdview

一个超轻量级的 Markdown 阅读器，用 Rust + egui 构建。

## 特性

- 超快速启动和渲染
- 语法高亮（支持多种编程语言）
- 8+ 套主题切换
- 文本选择和复制
- 图片显示（本地/网络）
- 文件拖拽打开
- 右键菜单操作

## 运行

```bash
# Debug 模式
cargo run -- [file.md]

# Release 模式
cargo run --release -- [file.md]
```

## 快捷操作

| 操作 | 说明 |
|------|------|
| 鼠标滚轮 | 滚动文档 |
| Ctrl+C | 复制选中文本 |
| 右键菜单 | 切换主题、调整字体大小 |

## 依赖

- Rust 1.70+
- Windows

## License

MIT