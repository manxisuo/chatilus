# Chatilus

**Browse your AI memory** — 轻量、本地优先的多源 AI 对话历史浏览器。支持 ChatGPT 导出、Cursor、Gemini Takeout 等数据导入本地 SQLite，统一浏览、搜索与图片回顾。

> **早期内测 v0.2.x** — Windows 安装包见 [Releases](https://github.com/manxisuo/ChatLens/releases)。欢迎试用并通过 [Issues](https://github.com/manxisuo/ChatLens/issues) 反馈（请勿粘贴完整对话内容）。

## 下载与安装

| 平台 | 状态 | 说明 |
|------|------|------|
| **Windows 10/11** | 内测 | 从 [Releases](https://github.com/manxisuo/ChatLens/releases) 下载 `.msi` 或 `.exe` 安装包 |
| macOS / Linux | 未正式支持 | 可自行克隆仓库后 `npm run tauri build` |

安装后首次使用：点击「导入数据」→ 选择 **ChatGPT** → 选择官方导出 ZIP 或解压目录。

## 使用

1. 从 ChatGPT / Cursor / Gemini 等处导出或定位本地数据
2. 启动 Chatilus，点击「导入数据」
3. 按来源浏览对话、搜索消息、回顾图片；支持收藏与标签
4. 可将当前对话导出为 Markdown 文件

**数据源成熟度（0.2.0）**

| 来源 | 状态 |
|------|------|
| ChatGPT | Stable |
| Cursor、Gemini | Beta |
| Codex、Copilot、Grok、DeepSeek | Experimental |

## 隐私与数据

- **本地优先**：对话内容默认不上传；浏览与搜索在本机 SQLite 完成
- **无外部 AI / 无 telemetry**：当前版本不调用大模型 API，不收集使用统计
- **存储位置**：Windows 下约为 `%APPDATA%\com.manxi.chatlens\chatlens.db`（应用内「⋯ → 隐私与数据」可查看实际路径）
- **导入缓存**：部分导入会在数据库同级 `imports/` 目录缓存导出副本
- **删除数据**：退出应用后删除 `chatlens.db` 与 `imports/` 文件夹；卸载应用不会自动删除

详见应用内 **隐私与数据**，或 [发布清单](docs/RELEASE_CHECKLIST.md)。

## 反馈

- [报告 Bug / 问题](https://github.com/manxisuo/ChatLens/issues/new?template=bug_report.yml)
- [已知问题](KNOWN_ISSUES.md)
- [更新日志](CHANGELOG.md)

## 开发

```bash
npm install
npm run tauri dev
```

## 打包

```bash
npm run tauri build
```

产物位于 `src-tauri/target/release/bundle/`。

## 文档

- [ROADMAP.md](./ROADMAP.md) — 产品与技术路线
- [docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md) — 架构说明
- [docs/RELEASE_CHECKLIST.md](./docs/RELEASE_CHECKLIST.md) — 发布任务清单
- [docs/BETA_RECRUITMENT.md](./docs/BETA_RECRUITMENT.md) — 内测招募文案

## 技术栈

- Tauri 2 + Vue 3 + TypeScript + Element Plus
- Rust 解析器 + SQLite FTS5
