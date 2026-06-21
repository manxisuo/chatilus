# ChatLens 技术路线

> **ChatLens**：一个轻量、本地优先的 AI 对话历史浏览器。

## 技术栈

| 层级 | 选型 | 说明 |
|------|------|------|
| 桌面壳 | **Tauri 2** | 轻量跨平台，Rust 后端处理大文件与本地 IO |
| 前端 | **Vue 3 + TypeScript + Vite + Element Plus** | 列表/搜索/布局开发效率高 |
| 渲染 | **markdown-it + highlight.js** | Markdown 与代码块高亮（v0.2） |
| 存储 | **SQLite + FTS5** | 本地索引与全文检索 |
| 解析 | **Rust (serde_json)** | 线性化 ChatGPT `mapping` 消息树 |

### 为什么不是 Electron？

ChatLens 只需浏览本地历史数据，不需要内置完整 Chromium。**Tauri 2** 体积更小、内存占用更低，且 Rust 适合处理 GB 级 JSON 分片与 zip 解压。

---

## 架构

```txt
ChatGPT Export（zip / 解压目录）
        │
        ▼
  Rust 导入解析器
  · 检测 conversations.json / conversations-NNN.json
  · 线性化 mapping 树（current_node → parent 链）
  · 提取 role / content / 时间 / 模型
        │
        ▼
  SQLite 数据库
  · conversations
  · messages
  · messages_fts（FTS5 全文索引）
        │
        ▼
  Vue 3 前端
  · 对话列表 · 消息浏览 · 搜索 · 收藏/标签/导出/图片
```

---

## 参考数据格式

基于本地导出样本（`2026-05-16-12-08-35`）：

- **分片 JSON**：`conversations-000.json` … `conversations-013.json`
- **元数据**：`export_manifest.json`、`user.json`、`user_settings.json`
- **媒体**：根目录 `file-*.jpg/png`、UUID 子目录、`dalle-generations/` 等
- **单条对话结构**：`mapping`（DAG 消息树）+ `current_node`（活跃分支指针）

解析策略：从 `current_node` 沿 `parent` 回溯至根，反转后得到线性消息序列；导入阶段完成线性化，UI 不再遍历树。

---

## 数据库设计

### conversations

| 字段 | 类型 | 说明 |
|------|------|------|
| id | TEXT PK | 对话 UUID |
| title | TEXT | 标题 |
| create_time | REAL | 创建时间（Unix 秒） |
| update_time | REAL | 更新时间 |
| source_path | TEXT | 导入来源目录 |
| model | TEXT | default_model_slug |
| message_count | INTEGER | 消息条数 |

### messages

| 字段 | 类型 | 说明 |
|------|------|------|
| id | TEXT PK | 消息节点 ID |
| conversation_id | TEXT FK | 所属对话 |
| role | TEXT | user / assistant / tool |
| content | TEXT | 纯文本内容 |
| create_time | REAL | 消息时间 |
| sort_order | INTEGER | 线性顺序 |
| raw_json | TEXT | 原始 message JSON（预留扩展） |

### messages_fts（FTS5 虚拟表）

| 字段 | 说明 |
|------|------|
| message_id | 消息 ID（UNINDEXED） |
| conversation_id | 对话 ID（UNINDEXED） |
| conversation_title | 对话标题（可搜索） |
| content | 消息正文 |

---

## 版本规划

### v0.1 — 最小闭环（当前）

- [x] 项目脚手架（Tauri 2 + Vue 3 + TS）
- [x] 导入解压后的导出目录（`conversations*.json`）
- [x] 会话列表 + 单会话消息浏览
- [x] 基础 Markdown 渲染
- [x] 按更新时间排序
- [x] 关键词全文搜索（FTS5）

### v0.2 — 体验增强

- [x] 代码块语法高亮
- [x] 收藏会话 / 收藏消息
- [x] 标签
- [x] 导出为 Markdown
- [x] 图片附件展示（`file-*` 本地路径解析）

### v0.3 — 导入与多源

- [ ] 直接导入 zip（含嵌套 zip）
- [ ] 导入进度与增量更新
- [ ] 多导出包合并 / 去重
- [ ] Claude / Gemini / DeepSeek 导出格式

### v0.4+ — 智能分析（可选）

- [ ] AI 总结历史对话
- [ ] 向量检索
- [ ] 按主题聚类
- [ ] 本地知识库 / RAG

---

## 备选方案

| 方案 | 适用场景 | 评价 |
|------|----------|------|
| **Tauri + Vue + SQLite** | 本项目首选 | 轻、快、可扩展 |
| Electron + Vue + SQLite | 最快出 Demo | 简单但偏重 |
| Qt 6 + QML + C++ | 纯原生桌面 | UI 迭代慢 |
| Python + PySide6 | 内部工具 | 发布体验一般 |
| Web + IndexedDB | 免安装 | 文件访问受限 |

---

## 开发命令

```bash
npm install
npm run tauri dev      # 开发
npm run tauri build    # 打包
```

数据库默认位置：`%APPDATA%/com.manxi.chatlens/chatlens.db`（Windows）
