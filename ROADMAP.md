# ChatLens 技术路线

> **ChatLens**：一个轻量、本地优先的 AI 对话历史浏览器。
>
> 长期愿景：从「导出数据查看器」演进为本地 **AI Conversation OS**——核心是 **Import + Index + Browse**，而非云平台或插件生态。

## 当前重点（v0.3）

架构收敛（PR 1–5）**已完成**。下一阶段以**产品能力**为主，详见 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)。

### 产品功能

- [x] 直接导入 zip（含嵌套 zip）
- [x] 导入进度 UI（本地 `ImportJob` + 进度条）
- [ ] 多导出包合并 / 去重
- [ ] Claude / Gemini / DeepSeek 等导出格式（按需新增 Importer）

### 技术配套（与产品功能同步推进）

- [x] `meta` 表 + `schema_version`（统一 DB 迁移）
- [x] `ImportJob` 最小模型（`id` / `source_path` / `status` / `progress` / `error`；不做分布式任务队列）
- [x] 导入元数据 `SourceInfo`（`source` + `export_label` + `importer_version`，写入 `imports` 或 `ImportJob`）
- [ ] 按需 DB 迁移：`conversations.source`、`conversations.source_id`

---

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

> 详细架构图、数据流与目录说明见 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)。

### 核心原则

> **外部数据源可以变，内部模型尽量稳定。**

核心层只认：`Conversation`、`Message`、`Asset`、`Importer`、`Repository`、`SearchEngine`。

以下概念**不得渗入** domain / application 层：

- ChatGPT `mapping` / `current_node`
- Cursor SQLite 表结构
- Claude / Gemini 导出字段
- OpenAI response 格式
- FTS5 SQL 细节

演进规则：

| 变化类型 | 改哪里 |
|----------|--------|
| 外部导出格式 | Importer |
| 存储后端 | Repository |
| 搜索能力（向量、混合检索） | SearchEngine |
| LLM 总结 / 打标签 | `application/ai/`（非 domain 核心） |

### 分层（当前形态）

```txt
Vue 3 前端（薄层：api.ts + components）
        │  invoke
        ▼
  commands.rs（Tauri 适配，不含业务逻辑）
        │
        ▼
  application/（用例：import_data、list_conversations、search_all …）
        │                    └── ai/（预留，v0.4+）
        ▼
  domain/（稳定模型 + 端口 trait）
        ▲
        │  实现
  infrastructure/
    · importers/chatgpt/     外部格式 → 内部模型
    · db/                    SQLite Repository
    · search/                SQLite FTS5 SearchEngine
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
    ports/           importer, repository, search_engine
  application/
    import_data, conversations, messages, assets, search, tags
    ai/              AIService（预留，v0.4+）
  infrastructure/
    importers/chatgpt/
    db/              sqlite + *_repository.rs
    search/          sqlite_fts.rs
  commands.rs        薄适配层
```

前端保持现有结构（`api.ts`、`types.ts`、`components/`），不急于拆 `presentation/` 子目录。

### 核心内部模型

长期稳定的领域概念（Rust 为 source of truth，经 commands 序列化给前端）：

| 模型 | 说明 |
|------|------|
| `Conversation` | 会话元数据：`source`、`sourceId`、`title`、时间、计数、收藏、标签 |
| `Message` | 消息：`role`、`content: MessageContent[]`、`plainText`、父子关系、`assetIds` |
| `Asset` | 统一资产：`AssetType` 已含 `Image` / `File` / `Code` / `Prompt` / `Link` |
| `Tag` | 标签（保持独立表） |
| `SearchQuery` / `SearchResult` | 搜索抽象 |
| `ImportJob` | 导入任务（v0.3 轻量实现：状态 + 进度，非完整任务平台） |
| `SourceInfo` | 导入元数据：`source`、`export_label`、`importer_version`（v0.3） |

`MessageContent` 第一版从简，导入时以 `Text` / `ImageRef` 为主，后续再补 `Code`、`Link` 等结构化块。

收藏暂用 `is_favorite` 字段（DB 列可继续叫 `is_starred`，mapper 转换），不必单独建 `Favorite` 实体表。

### 端口抽象

| 端口 | 职责 | 当前实现 |
|------|------|----------|
| `Importer` | `detect` / `preview` / `import` → `NormalizedImportResult` | `ChatGptImporter` |
| `ConversationRepository` 等 | 持久化 domain 模型 | SQLite |
| `SearchEngine` | 全文检索，与 FTS 实现解耦 | `SqliteFtsSearchEngine` |

AI 能力（总结、打标签、嵌入）拟放在 **`application/ai/`**，不作为 domain port；若未来需跨端复用再考虑下沉 trait。

### 架构维护原则

架构收敛已完成，进入**维护模式**。新增能力遵循：

- 新外部格式 → 新 `Importer` 实现
- 新存储后端 → 新 `Repository` 实现
- 新检索方式 → 新 `SearchEngine` 实现
- 新 AI 能力 → `application/ai/` 编排，不污染 domain 模型

### 架构收敛（已完成，PR 1–5）

轻量收敛，每步可独立合并、UI 行为不变。双轨运行：domain 模型与 View DTO 并存，经 mapper 转换。

- [x] PR 1：领域模型 + 映射
- [x] PR 2：ChatGPT Importer
- [x] PR 3：Repository 拆分 + application 层
- [x] PR 4：SearchEngine trait
- [x] PR 5：Asset 统一

### 暂不过度设计

以下只在架构上**留位置或文档愿景**，当前不实现：

- 插件化 Importer 动态加载（`*.dll` / `*.wasm`）
- 服务端 / 多端同步
- 向量数据库、Hybrid 搜索
- LLM API 配置中心
- 分布式 / 可重试的完整 `ImportJob` 任务平台（v0.3 仅做本地同步 job + 进度条）
- `Indexer` 编排层（FTS + 缩略图 + Embedding + Summary 统一重建，v0.5+ 再评估）
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

### meta（v0.3 新增）

| 字段 | 类型 | 说明 |
|------|------|------|
| key | TEXT PK | 如 `schema_version`、`app_version` |
| value | TEXT | 版本号或元数据 |

用于统一 `v4 → v5` 等 schema 迁移，替代零散 `ALTER TABLE`。

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
| source | TEXT | 数据源（v0.3 可选迁移） |
| source_id | TEXT | 源系统内 ID（v0.3 可选迁移） |

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

### v0.1 — 最小闭环 ✅

- [x] 项目脚手架（Tauri 2 + Vue 3 + TS）
- [x] 导入解压后的导出目录（`conversations*.json`）
- [x] 会话列表 + 单会话消息浏览
- [x] 基础 Markdown 渲染
- [x] 按更新时间排序
- [x] 关键词全文搜索（FTS5）

### v0.2 — 体验增强 ✅

- [x] 代码块语法高亮
- [x] 收藏会话 / 收藏消息
- [x] 标签
- [x] 导出为 Markdown
- [x] 图片附件展示（`file-*` 本地路径解析）

### v0.3 — 导入与多源（当前）

**架构收敛** — 已完成，见上文 PR 1–5。

**产品 + 技术配套** — 见本文档顶部「当前重点」。

### v0.4+ — 智能分析（可选）

- [ ] `application/ai/`：AI 总结历史对话
- [ ] 向量检索（新 `SearchEngine` 实现或扩展）
- [ ] 按主题聚类
- [ ] 本地知识库 / RAG

### v0.5+ — 索引编排（远期愿景）

若索引种类增多（FTS、缩略图、Embedding、摘要），可评估 `Indexer` 编排层，与现有 `SearchEngine` 分工：SearchEngine 负责查询，Indexer 负责导入后重建各类索引。

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
