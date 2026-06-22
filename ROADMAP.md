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

### 核心原则

> **外部数据源可以变，内部模型尽量稳定。**

核心层只认：`Conversation`、`Message`、`Asset`、`Importer`、`Repository`、`SearchEngine`、`AIProvider`。

以下概念**不得渗入** domain / application 层：

- ChatGPT `mapping` / `current_node`
- Cursor SQLite 表结构
- Claude / Gemini 导出字段
- OpenAI response 格式
- FTS5 SQL 细节

外部格式变化 → 改 Importer；存储变化 → 改 Repository；搜索升级 → 改 SearchEngine；LLM 接入 → 加 AIProvider。

### 分层（目标形态）

```txt
Vue 3 前端（薄层：api.ts + components）
        │  invoke
        ▼
  commands.rs（Tauri 适配，不含业务逻辑）
        │
        ▼
  application/（用例：import_data、list_conversations、search_all …）
        │
        ▼
  domain/（稳定模型 + 端口 trait）
        ▲
        │  实现
  infrastructure/
    · importers/chatgpt/     外部格式 → 内部模型
    · db/                    SQLite Repository
    · search/                SQLite FTS5 SearchEngine
    · ai/                    NullAIProvider（预留）
```

数据流：

```txt
ChatGPT Export（zip / 解压目录）
        │
        ▼
  ChatGptImporter（detect → preview → import）
        │
        ▼
  Conversation / Message / Asset（domain 模型）
        │
        ├──► Repository → SQLite
        └──► SearchEngine → FTS5
        │
        ▼
  View DTO → Vue（对话列表 · 消息 · 搜索 · 收藏/标签 · 图片）
```

### Rust 目录结构（收敛后）

```txt
src-tauri/src/
  domain/
    models/          conversation, message, asset, source, search
    ports/           importer, repository, search_engine, ai_provider
  application/
    usecases/        import_data, list_conversations, search_all, …
  infrastructure/
    importers/chatgpt/
    db/              sqlite + *\_repository.rs
    search/          sqlite_fts_search_engine.rs
    ai/              null_ai_provider.rs
  commands.rs        薄适配层
```

前端保持现有结构（`api.ts`、`types.ts`、`components/`），不急于拆 `presentation/` 子目录。

### 核心内部模型

长期稳定的领域概念（Rust 为 source of truth，经 commands 序列化给前端）：

| 模型 | 说明 |
|------|------|
| `Conversation` | 会话元数据：`source`、`sourceId`、`title`、时间、计数、收藏、标签 |
| `Message` | 消息：`role`、`content: MessageContent[]`、`plainText`、父子关系、`assetIds` |
| `Asset` | 统一资产：图片、文件、代码片段、链接等（合并现有 `AttachmentView` / `ImageGalleryItem`） |
| `Tag` | 标签（保持独立表） |
| `SearchQuery` / `SearchResult` | 搜索抽象 |
| `ImportJob` | 导入任务（接口预留，当前可用 `ImportResult` 扩展） |

`MessageContent` 第一版从简，导入时以 `Text` / `ImageRef` 为主，后续再补 `Code`、`Link` 等结构化块。

收藏暂用 `is_favorite` 字段（DB 列可继续叫 `is_starred`，mapper 转换），不必单独建 `Favorite` 实体表。

### 端口抽象

| 端口 | 职责 | 当前实现 |
|------|------|----------|
| `Importer` | `detect` / `preview` / `import` → `NormalizedImportResult` | `ChatGptImporter` |
| `ConversationRepository` 等 | 持久化 domain 模型 | SQLite |
| `SearchEngine` | 全文检索，与 FTS 实现解耦 | `SqliteFtsSearchEngine` |
| `AIProvider` | 总结、打标签、抽资产、嵌入（可选） | `NullAIProvider`（空实现） |

### 架构收敛（v0.3 前置，分 PR 实施）

轻量收敛，**每步可独立合并、UI 行为不变、测试保持通过**。双轨运行：domain 模型与现有 View DTO 并存，通过 mapper 转换，不一次性删旧类型。

#### PR 1 — Phase A：领域模型 + 映射

- [x] 新增 `domain/models/`（`DataSource`、`Conversation`、`Message`、`Asset`）
- [x] `MessageContent` 枚举（首版 `Text` + `ImageRef`）
- [x] `impl From<Domain> for ViewDto` 映射，**旧 Tauri command 返回类型不变**
- [x] 单元测试覆盖 mapper

#### PR 2 — Phase B：抽出 ChatGPT Importer

- [x] 定义 `Importer` trait（`domain/ports/importer.rs`）
- [x] `infrastructure/importers/chatgpt/` 收纳现有 `parser/` + `media/` 编排
- [x] `import_export_dir` 改为：`importer.import()` → `persist_import()`
- [x] `Database` 不再直接 `use parser::`

#### PR 3 — Phase C：拆 Repository

- [x] 定义 Repository trait（`domain/ports/repository.rs`）
- [x] 从 `db/mod.rs` 切出 `conversation_repo`、`message_repo`、`asset_repo`
- [x] `application/` 用例层 + `commands` 经 application 调用

#### PR 4 — Phase D：SearchEngine trait

- [x] 定义 `SearchEngine` trait
- [x] FTS 读写迁入 `infrastructure/search/sqlite_fts.rs`
- [x] `search_messages` 经 application 层调用

#### PR 5 — Phase E：Asset 统一（可与图片功能迭代合并）

- [ ] `list_images` 改为 `AssetRepository::list`
- [ ] 逐步弃用 `ImageGalleryItem` 专用结构，统一为 `Asset` + View 投影

**同步小项（任意 PR 可附带）：**

- [ ] `NullAIProvider` + trait 定义（零调用路径）
- [ ] 按需 DB 迁移：`source`、`source_id` 列（非阻塞）

### 暂不过度设计

以下只在架构上**留位置**，当前不实现：

- 插件化 Importer 动态加载
- 服务端 / 多端同步
- 向量数据库、Hybrid 搜索
- LLM API 配置中心
- 完整 `ImportJob` 进度系统
- 前端完整 Clean Architecture 目录

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

### v0.3 — 架构收敛 + 导入与多源

**架构收敛（优先，见上文「架构收敛」5 个 PR）**

- [x] PR 1：领域模型 + 映射
- [x] PR 2：ChatGPT Importer
- [x] PR 3：Repository 拆分
- [x] PR 4：SearchEngine trait
- [ ] PR 5：Asset 统一

**功能（收敛完成后再做）**

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
