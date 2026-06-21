# ChatLens

轻量、本地优先的 AI 对话历史浏览器。读取 ChatGPT 导出的 `conversations*.json`，导入本地 SQLite 数据库，支持浏览与全文搜索。

## 技术栈

- Tauri 2 + Vue 3 + TypeScript + Element Plus
- Rust 解析器 + SQLite FTS5

详细路线见 [ROADMAP.md](./ROADMAP.md)。

## 开发

```bash
npm install
npm run tauri dev
```

## 使用

1. 从 ChatGPT 导出数据并解压
2. 启动 ChatLens，点击「导入导出目录」
3. 在左侧选择对话，浏览消息；支持收藏、标签、全文搜索
4. 消息中的图片附件会自动解析并展示（需从原导出目录导入）
5. 可将当前对话导出为 Markdown 文件

## 打包

```bash
npm run tauri build
```
