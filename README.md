# ChatLens

**Browse your AI memory** — 轻量、本地优先的多源 AI 对话历史浏览器。支持 ChatGPT 导出、Cursor、Gemini Takeout 等数据导入本地 SQLite，统一浏览、搜索与图片回顾。

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

1. 从 ChatGPT / Cursor / Gemini 等处导出或定位本地数据
2. 启动 ChatLens，点击「导入数据」
3. 按来源浏览对话、搜索消息、回顾图片；支持收藏与标签
4. 可将当前对话导出为 Markdown 文件

## 打包

```bash
npm run tauri build
```
