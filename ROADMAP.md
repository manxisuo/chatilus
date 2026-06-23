# ChatLens 技术路线

> **ChatLens**：一个轻量、本地优先的 AI 对话历史浏览器。
>
> 长期愿景：从「导出数据查看器」演进为本地 **AI Conversation Browser / Personal AI Memory Browser**——核心是 **Import + Index + Browse**；智能分析（总结、RAG）为后续增强，不替代浏览闭环。

## 当前重点（v0.4）

v0.3（导入、进度、多包合并去重）**已完成**。v0.4 目标：从「单源查看器」升级为**多源 AI 对话历史浏览器**。

产品原则：**底层统一，视图分离**——同一 SQLite 库存储所有来源；UI 用 `source` 分组、过滤、标注，不拆库。

详见 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)。

### 多源 Importer（PR4a–c）

- [x] `ImporterRegistry`：自动 `detect` / 选择 Importer（PR4a）
- [x] Cursor 本地 `state.vscdb` 导入（PR4b）
- [ ] Claude 导出格式（按需新增 Importer，PR4c+，有样本再做）
- [x] Gemini Google Takeout（`My Activity / Gemini Apps` HTML 活动记录）

### 多源 UI（PR4d，与 Importer 同属 v0.4）

存储与搜索层已统一；下列为**视图层**，让「All Conversations」成为主入口：

- [x] 左侧 **Sources** 导航：`All` / `ChatGPT` / `Cursor` / …（含各源计数）
- [x] 对话列表 **来源徽标**（如 ChatGPT / Cursor 小标签）
- [x] `ConversationListQuery` 增加 `source` 过滤
- [x] 搜索结果展示 **来源**（`SearchHit` + UI）
- [x] 会话列表 `source` 字段贯通 API（已完成部分：消息角色标签已按源显示）
- [x] （可选）图片画廊按对话来源筛选

### 技术配套

- [x] 导入流程去 ChatGPT 硬编码（`import_data` / `zip` 解压后根目录探测）
- [x] 各 Importer 独立 `importer_version`（`Importer::version()`）
- [x] 导入文件选择器支持 `.vscdb`（Cursor）
- [x] 内部 `id` 规范为 `{source}::{source_id}`（schema v4 迁移）

### 性能索引（PR5，schema v5）

围绕真实查询路径补充 SQLite 索引（`CURRENT_SCHEMA_VERSION = 5`）：

- [x] `conversations(source, update_time)`：Sources 导航按来源过滤 + 时间排序
- [x] `UNIQUE (source, source_id)`：导入去重业务唯一键
- [x] `messages(create_time)`：为 Timeline 预埋
- [x] `conversation_tags(tag_id, conversation_id)`：按标签筛会话

已有、不重复建设：`messages(conversation_id, sort_order)`、`conversations(update_time)`、`conversations(is_starred, update_time)`、`messages_fts`（FTS5）。

EXPLAIN 抽检（本机）：`idx_messages_conversation`、`idx_conversation_tags_tag` 已命中；来源列表因 `COALESCE(source)` + 先排 `is_starred` 暂未用到 `idx_conversations_source_update`（见「后续任务分层」可选优化项）。

**明确不在 PR5 范围**（见下文「后续任务分层」）：`assets` 表索引（当前无独立表）、`imports` / `import_jobs` 低优先级索引、FTS `bm25()` 排序改造。

### 明确不做进 v0.4

- **PR4e 智能分析**（总结、向量检索、聚类、RAG）→ **延后至 v0.7+**，不阻塞多源浏览、Timeline 与 Workspace。见下文版本规划说明。

---

## 后续任务分层

> 排序原则：先把「多源 AI 对话浏览器」做扎实，再做 Timeline / Workspace 等知识管理能力，最后再做 AI 总结、插件化和索引平台。AI 能力不进入 domain 核心。

### 接下来要做（近期：产品化浏览体验）

- [x] 顶部统计增强：展示对话、图片、来源、收藏、标签数量，以及最近导入时间，让首页更像 Dashboard。
- [x] 左侧过滤区整理：将收藏、有图片、有代码、有附件等过滤项归到 Filter 区，Tags 独立展示，降低后台管理感。
- [x] 消息阅读区优化：宽屏下限制正文最大宽度或增加右侧 Conversation Info（来源、模型、时间跨度、图片数、标签）。
- [x] 图片画廊增强：按时间分组（今天 / 昨天 / 本周 / 月份），并提供「打开所属对话」入口。
- [x] 搜索结果体验小步增强：更清晰展示来源、时间、会话上下文和命中片段。
- [x] 产品定位文案收敛：围绕「Browse your AI memory」验证主页说明、空状态和 README 表述。
- [x] 核心查询 `EXPLAIN QUERY PLAN` 抽检：会话列表、打开对话、标签筛选、图库扫描，确认索引命中情况（2026-06：打开对话 / 标签筛选命中 v5 索引；图库仍为 `SCAN messages`，见下文 assets 物化）。

### 后面要做（中期：知识管理与索引深化）

- [ ] （可选）来源列表查询优化：`conversation_repository.list` 将 `COALESCE(source, …)` 改为 `source = ?`（导入已保证非 NULL），或评估复合索引 `(source, is_starred, update_time)`；当前约 1.5k 会话体量可暂缓。

- [ ] Timeline：按时间混排 ChatGPT / Cursor / Gemini 等来源，支持按来源过滤与月份导航。
- [ ] Global Search++：在现有 FTS5 基础上将搜索改为显式 `bm25()` 排序，并逐步考虑时间、来源、收藏等权重。
- [ ] Workspace / Project：将不同来源的会话归到同一工作项目下，例如 ChatLens、Plum、USV。
- [ ] 项目视图：在一个 Workspace 内聚合相关对话、图片、标签与搜索结果。
- [ ] **图片资产物化**：导入时写入独立 `assets` 表，替代 `json_each(attachments)` 全表扫描；再为 `(asset_type, source, created_at)` 等建索引。
- [ ] Asset 模型增强：为图片资产预留 `thumbnail_path`，为大量图片场景准备缩略图缓存。
- [ ] 结构化内容块：逐步从纯文本扩展到 Code、Link、File 等 `MessageContent` 类型，提高过滤与搜索能力。
- [ ] 导入历史索引：`imports(source, imported_at)`、`import_jobs(status, created_at)` 等（排错 / 增量导入，非热路径）。

### 很久后要做（远期：智能化与生态）

- [ ] `application/ai/`：对话总结、自动标签、主题聚类，保持在 application 层编排，不污染 domain。
- [ ] 向量检索 / Hybrid Search：作为新的 `SearchEngine` 实现或扩展，不替代 FTS5。
- [ ] 本地知识库 / RAG：在 Import + Index + Browse 稳定后再评估。
- [ ] `Indexer` 编排层：统一管理 FTS、缩略图、Embedding、摘要等导入后重建任务。
- [ ] 插件化 Importer：长期可探索动态加载 `*.dll` / `*.wasm`，支持社区贡献 Claude、Cursor 等 Importer。
- [ ] 多端同步 / 服务端形态：仅在本地桌面闭环成熟后考虑。

---

## v0.3 交付清单（已完成）

### 产品功能

- [x] 直接导入 zip（含嵌套 zip）
- [x] 导入进度 UI（本地 `ImportJob` + 进度条）
- [x] 多导出包合并 / 去重

### 技术配套

- [x] `meta` 表 + `schema_version`（统一 DB 迁移）
- [x] `ImportJob` 最小模型（`id` / `source_path` / `status` / `progress` / `error`；不做分布式任务队列）
- [x] 导入元数据 `SourceInfo`（`source` + `export_label` + `importer_version`，写入 `imports` 或 `ImportJob`）
- [x] 按需 DB 迁移：`conversations.source`、`conversations.source_id`

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
  View DTO → Vue（All / 按源视图 · 消息 · 跨源搜索 · 收藏/标签 · 图片）
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
- `Indexer` 编排层（FTS + 缩略图 + Embedding + Summary 统一重建，v1.0+ 再评估）
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

### v0.3 — 导入增强 ✅

**架构收敛** — PR 1–5，已完成。

**产品 + 技术配套** — 见上文「v0.3 交付清单」。

- zip 直导、导入进度、`ImportJob`
- 多导出包合并 / 去重
- `schema_version`、`SourceInfo`、`conversations.source` / `source_id`

### v0.4 — 多源浏览（当前）

**目标**：用户可在同一库中导入、浏览、搜索 ChatGPT / Cursor 等对话；左侧可按来源切换视图，默认 **All** 混排。

**PR 拆分**

| PR | 内容 | 状态 |
|----|------|------|
| PR4a | `ImporterRegistry` + 去硬编码 | ✅ |
| PR4b | Cursor Importer | ✅ |
| PR4c | Claude / Gemini 等（有样本再做） | Gemini ✅ / Claude 待定 |
| PR4d | 多源 UI：Sources 导航、列表徽标、搜索来源、按源筛选 | ✅ |
| PR5 | schema v5 性能索引（来源 / 标签 / Timeline 预埋 / 导入唯一键） | ✅ |

**不在 v0.4 范围内**

- PR4e 智能分析 → **v0.7+**（见下），避免 Importer + UI 与 AI 基础设施并行，拖慢「多源浏览」这一核心差异化。

### v0.5 — 产品化浏览与 Timeline（下一里程碑）

v0.4 交付「Import + Index + Browse」多源闭环后，下一步先增强浏览体验，而不是直接进入 AI 总结：

- [ ] 顶部统计 / Dashboard 感增强
- [ ] 左侧 Filter / Tags 区域整理
- [ ] 消息阅读区宽屏优化或 Conversation Info 侧栏
- [ ] 图片画廊按时间分组，增加「打开所属对话」
- [ ] Timeline：跨来源按时间混排对话和关键资产
- [ ] Global Search++ 第一阶段：显式 `bm25()` 排序 + 更好的命中上下文 + 时间加权

> **关于原 PR4e**：智能分析不必在 v0.4 内做完，也不必作为 v0.5 的首要目标；先把多源浏览、时间线和搜索体验打磨到可长期使用。

### v0.6 — Workspace / Project 与结构化资产

- [ ] Workspace / Project：将多源会话按真实工作项目聚合
- [ ] 项目视图：项目下的会话、图片、标签、搜索结果统一浏览
- [ ] 图片资产物化：导入时写入 `assets` 表，为图库查询建立 `(asset_type, source, created_at)` 等索引
- [ ] Asset 缩略图：`original_path` / `thumbnail_path`
- [ ] `MessageContent` 扩展：Code / Link / File 等结构化块
- [ ] 搜索权重扩展：时间、来源、收藏、标签等信号

### v0.7+ — 智能分析（后续增强）

全部可选、可逐项启用；AI 能力放在 `application/ai/`，不进入 domain 核心：

- [ ] 对话总结（需用户配置 API Key 或本地模型）
- [ ] 自动标签
- [ ] 按主题聚类
- [ ] 向量检索（新 `SearchEngine` 实现或扩展 FTS）
- [ ] 本地知识库 / RAG

### v1.0+ — 索引编排与生态（远期愿景）

若索引种类增多（FTS、缩略图、Embedding、摘要），可评估 `Indexer` 编排层，与现有 `SearchEngine` 分工：SearchEngine 负责查询，Indexer 负责导入后重建各类索引。

- [ ] 插件化 Importer 动态加载（`*.dll` / `*.wasm`）
- [ ] 服务端 / 多端同步
- [ ] LLM API 配置中心

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
