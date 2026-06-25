# ChatLens 技术路线

> **ChatLens**：面向个人的 **AI 工作历史浏览器**（Personal AI Work History Browser）。
>
> 回答的问题不是「我记了什么」，而是 **「我和 AI 一起工作过什么？」**
>
> 对外标语可继续用 *Browse your AI memory*；对内产品边界是 **多源 AI 对话史的本地索引与回顾**，不是 ChatGPT 导出工具、不是 Obsidian 式 PKM、也不是「Memory OS」。

## 产品定位与价值路径

### 北极星

当用户需要找回 AI 记忆时，**ChatLens 是默认入口**。

工具类产品不必追求日活；目标是「没有它就难受」——换电脑、找旧讨论、跨平台回忆时第一个想到它。

### 价值四层（演进模型）

```text
Archive（归档）  →  Retrieve（检索）  →  Organize（组织）  →  Insight（理解）
```

| 层次 | 含义 | 典型能力 | 状态（粗估） |
|------|------|----------|--------------|
| **Archive** | 多源数据进库、可读可搜 | 导入、浏览、FTS、图库、过滤、导出 | ~90% |
| **Retrieve** | 跨源、跨时间找到那条记忆 | 全文搜索、来源/时间上下文、钻取到对话 | ~60% |
| **Organize** | 按时间/项目/主题组织记忆 | Timeline、Workspace、Topic、Tag | ~35% |
| **Insight** | 在组织之上理解与回顾 | 可溯源总结、模式发现、主题演变 | 0%（远期） |

检索（Retrieve）贯穿各层：搜索是检索，Timeline 按月浏览也是检索，Workspace 按项目看仍是检索。

### Insight 层原则（远期，不可妥协）

任何 AI 结论必须 **Summary + Evidence** 同时存在：

- 每条总结可点击回到原始会话/消息
- 不可溯源的总结不做

AI 能力放在 `application/ai/`，不进入 domain 核心。

### 与竞品的关系

| 对比 | ChatLens | Obsidian / Logseq |
|------|----------|-------------------|
| 数据性质 | 被动汇聚各平台 AI 对话 | 主动书写笔记 |
| 核心问题 | 「我以前在哪儿和 AI 讨论过这个？」 | 「我的知识网络是什么？」 |

产品原则（技术）：**底层统一，视图分离**——同一 SQLite 库存储所有来源；UI 用 `source` 分组、过滤、标注，不拆库。

详见 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)。

## 当前重点（Timeline B+ → v0.6 Workspace）

v0.5 **Timeline**、收藏消息列表、图库 **assets 物化**、Search++ **bm25** 与来源索引 **v7** **已完成**。

**进行中 — Timeline B+（钻取与信息密度，先于 Workspace）**

> 在 Timeline 上把「月报」做透：Topic 可点击筛选、来源维度、左侧月份摘要等；**完成后再开 v0.6 Workspace**。

下一里程碑（B+ 之后）：

> **Workspace / Project** —— 将多源会话归入同一工作项目，在项目视图内聚合对话、图片与搜索。

### 多源 Importer（PR4a–c）

- [x] `ImporterRegistry`：自动 `detect` / 选择 Importer（PR4a）
- [x] Cursor 本地 `state.vscdb` 导入（PR4b）
- [x] Codex 本地 `~/.codex`（`state_*.sqlite` + `sessions/**/*.jsonl`）
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

### 性能索引（PR5 + schema v6 assets）

**PR5（schema v5）** — 围绕会话 / 消息 / 标签查询路径：

- [x] `conversations(source, update_time)`：Sources 导航按来源过滤 + 时间排序
- [x] `UNIQUE (source, source_id)`：导入去重业务唯一键
- [x] `messages(create_time)`：为 Timeline 预埋
- [x] `conversation_tags(tag_id, conversation_id)`：按标签筛会话

**schema v6 — 图片资产物化 `assets` 表**

- [x] 物化表 + `created_at` / `conversation_source` / `conversation_id` 索引
- [x] 迁移全库回填；导入时按会话 `reindex_conversation_assets`
- [x] 图库列表 / 统计 / Timeline 月份图片数改查 `assets`（消除 `json_each` 与 `raw_json` 运行时水合）
- [x] Timeline / 对话列表 `has_images` 改查 `assets`

已有、不重复建设：`messages(conversation_id, sort_order)`、`conversations(update_time)`、`conversations(is_starred, update_time)`、`messages_fts`（FTS5）。

EXPLAIN 抽检（本机）：`idx_messages_conversation`、`idx_conversation_tags_tag` 已命中；图库 / Timeline 图片统计已走 `assets` 索引；来源列表过滤已改为 `source = ?` 并新增 `idx_conversations_source_starred_update`（schema v7）。

**明确不在 PR5/v6 范围**：`imports` / `import_jobs` 低优先级索引。

### 明确不做进 v0.4

- **PR4e 智能分析**（总结、向量检索、聚类、RAG）→ **延后至 v0.8+**，不阻塞 Timeline 与 Workspace。见下文版本规划。

### v0.4 收尾与 v0.5 产品化（已完成）

多源闭环之外，近期浏览体验与交互打磨：

- [x] 顶部统计 / Dashboard 条（含最近导入时间）
- [x] 左侧 Filter 区压缩（来源 pill、筛选芯片、标签内联）
- [x] 消息阅读区 Conversation Info 侧栏（宽屏）
- [x] 图片画廊按时间分组 + 灯箱内「打开所属对话」
- [x] 搜索结果增强；搜索模式下保留结果列表、可连续点击命中项
- [x] 搜索场景 `get_conversation` 补全对话信息侧栏
- [x] 外观：浅色 / 深色 / 跟随系统（含 Element Plus 与原生窗口主题）
- [x] 产品定位文案：*Browse your AI memory*
- [x] 核心查询 `EXPLAIN QUERY PLAN` 抽检（2026-06：打开对话 / 标签筛选命中 v5 索引；图库 / Timeline 图片统计已走 `assets` v6 索引）

---

## 后续任务分层

> 排序原则：**Archive 做扎实 → Organize（Timeline / Workspace / Topic）→ Insight（可溯源 AI）**。不做十个浏览小功能替代 Timeline。

### 接下来要做（v0.6：Workspace，待 Timeline B+ 完成）

**主里程碑 — Workspace / Project**

- [ ] 手动或规则将多源会话归入 Workspace（ChatLens、Plum、USV…）
- [ ] 项目视图：会话、图片、标签、搜索在项目内聚合

**v0.5 已完成 — Timeline**

- [x] 新视图：按月份混排 ChatGPT / Cursor / Gemini 等来源的会话
- [x] 月份导航与来源过滤；左侧栏与对话页同宽，内容区限宽居中
- [x] 月 → 日 → 会话三级结构（按活动时间分组）
- [x] 每月来源分布统计（`source_counts`）；月份导航显示图片数
- [x] 从 Timeline 钻取到对话、最新消息、本月/对话图片
- [x] 本月图片数与图库 scoped 统计对齐（按消息 `create_time` 月份）
- [x] 月份跳转滚动定位修复；会话卡片元数据排版
- [x] 收藏消息列表（独立入口，与收藏对话拆分）
- [x] 利用 `conversations(update_time)` 与 `messages(create_time)` 索引

**v0.5 顺延 / 支撑（已完成）**

- [x] Global Search++（第一阶段）：显式 `bm25()` 排序；时间、来源、收藏等权重可后续迭代
- [x] （可选）来源列表查询优化：`COALESCE(source)` → `source = ?`，复合索引 `(source, is_starred, update_time)`（schema v7）

**v0.5 可选增强（Timeline，非阻塞 v0.6）**

- [x] 右侧月度洞察面板（统计摘要 + 来源分布 + Topic 区域）
- [x] Topic Bubble：标题关键词 / 标签聚合（无 AI）

**Timeline B+（钻取，先于 Workspace）**

- [x] **B1** Topic 点击筛选当月对话（月 → Topic → 会话）
- [x] **B2** Topic 来源 breakdown（如 `Docker 3 · ChatGPT 2 / Cursor 1`）
- [ ] **B3** 左侧月份栏来源摘要（`source_counts` 已有，补 UI）
- [ ] **B4**（可选）中间 feed 与右侧洞察分工、减少重复
- [ ] **B5**（延后）热度日历、Topic 环比、活跃时段分布

**Source Context（先于 Workspace，schema v8）**

- [x] `source_contexts` + `conversation_source_contexts` 数据模型
- [x] 导入时保存 ChatGPT Project / Cursor Repository·Folder / Codex Folder·Repository / Gemini Notebook
- [x] 会话列表、时间线、详情轻量展示来源上下文
- [x] 预留 `workspaces` / `workspace_items` / `workspace_source_context_mappings` 表结构（无 UI）

### 再往后（v0.6–v0.7：Organize）

- [ ] **Workspace / Project**：将多源会话归到同一工作项目（如 ChatLens、Plum、USV）；基于 Source Context 映射，而非导入时自动创建
- [ ] 项目视图：项目内聚合对话、图片、标签、搜索结果
- [ ] **Topic（主题）**：从标题/内容统计或提取高频主题（与用户 **Tag** 区分：Tag 手动，Topic 系统发现）
- [ ] Asset 模型增强：`thumbnail_path`，大量图片场景的缩略图缓存
- [ ] 结构化内容块：`MessageContent` 扩展 Code / Link / File，强化「有代码」等过滤
- [ ] 导入历史索引：`imports(source, imported_at)` 等（排错 / 增量，非热路径）

### 将来（v0.8+：Insight 与生态）

- [ ] **AI Insight**（可溯源）：对话总结、主题演变、模式发现——每条结论带证据链跳转
- [ ] 自动标签建议（可选，仍须用户确认）
- [ ] 向量检索 / Hybrid Search：新 `SearchEngine` 实现，不替代 FTS5
- [ ] 本地知识库 / RAG：Organize 层稳定后再评估
- [ ] `Indexer` 编排层：FTS、缩略图、Embedding、摘要等导入后重建
- [ ] Claude 等 Importer（有样本再做）
- [ ] 插件化 Importer、多端同步：仅本地闭环成熟后考虑

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

### 导入后去源化（维护中）

读路径只依赖 DB 已物化字段；各源解析与附件路径解析留在 **Importer + 导入写入** 阶段。

- [x] `message_repository` / `asset_index` 不再按源重建 `MediaIndex` 或解析 `raw_json`
- [x] 移除 `refresh_codex_media` 启动回填
- [x] 共享附件解析迁至 `infrastructure/attachments/`（`resolve_imported_attachments` 等）
- [ ] 前端展示统一：`MessageView` 角色标签改用 `sourceLabel`；`sourceContext` 中 ChatGPT `g-p` 项目名等特殊文案收拢到 `dataSource` / `sourceContext` 工具（去掉 Cursor system 等散落分支）

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

用于统一 `v4 → v6` 等 schema 迁移，替代零散 `ALTER TABLE`。当前版本：**6**（含 `assets` 物化）。

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

### assets（schema v6，物化图片索引）

| 字段 | 类型 | 说明 |
|------|------|------|
| message_id, file_key | TEXT PK | 所属消息 + 文件键 |
| conversation_id | TEXT FK | 所属对话 |
| conversation_source | TEXT | 来源（chatgpt / cursor / …） |
| role | TEXT | 消息角色 |
| conversation_title | TEXT | 会话标题（冗余，便于列表） |
| local_path | TEXT | 本地图片路径 |
| image_source | TEXT | generated / upload 等 |
| prompt | TEXT | 生成提示（可选） |
| created_at | REAL | 消息时间（用于月份统计与排序） |

导入时从已物化的 `messages.attachments` 写入；图库与 Timeline 图片统计只读 `assets` 表。

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

### v0.4 — 多源浏览 ✅

**目标**：用户可在同一库中导入、浏览、搜索 ChatGPT / Cursor / Gemini 等对话；左侧可按来源切换，默认 **All** 混排。

**PR 拆分**

| PR | 内容 | 状态 |
|----|------|------|
| PR4a | `ImporterRegistry` + 去硬编码 | ✅ |
| PR4b | Cursor Importer | ✅ |
| PR4c | Claude / Gemini 等（有样本再做） | Gemini ✅ / Claude 待定 |
| PR4d | 多源 UI：Sources 导航、列表徽标、搜索来源、按源筛选 | ✅ |
| PR5 | schema v5 性能索引（来源 / 标签 / Timeline 预埋 / 导入唯一键） | ✅ |
| PR5+ | schema v6 `assets` 物化（图库 / Timeline 图片性能） | ✅ |

产品化浏览与交互打磨见上文「v0.4 收尾与 v0.5 产品化」。

### v0.5 — Timeline + 图库性能（已完成）

- [x] **Timeline 视图**：跨来源按月份混排会话
- [x] 月份导航 + 来源过滤 + 日分组 + 月内来源统计
- [x] 钻取到对话 / 最新消息 / 图库（本月或所属对话）
- [x] 布局：侧栏对齐对话页、内容区限宽；本月图片数与图库对齐
- [x] 收藏消息列表与跨对话消息定位修复
- [x] **assets 物化**（schema v6）：图库 / 统计 / `has_images` 改查物化表
- [x] Global Search++ 第一阶段：`bm25()` 显式排序
- [x] （可选）来源列表 `source = ?` 查询 + `idx_conversations_source_starred_update`（schema v7）

### v0.6 — Workspace / Project

- [ ] 手动或规则将多源会话归入 Workspace（ChatLens、Plum、USV…）
- [ ] 项目视图：会话、图片、标签、搜索在项目内聚合
- [ ] 搜索权重扩展：时间、来源、收藏、标签
- [ ] `MessageContent` 结构化块；Asset 缩略图

### v0.7 — Topic（主题层）

- [ ] 主题发现：从标题/内容统计高频主题（如 Rust、副业、grpc）
- [ ] 主题页：各来源讨论次数、时间分布、一键钻取
- [ ] 与用户 **Tag** 并存：Tag = 手动标注，Topic = 系统归纳

### v0.8+ — Insight（可溯源智能分析）

全部可选；**每条 Insight 必须可回溯到原始会话**：

- [ ] 对话/时期总结（需用户配置 API Key 或本地模型）
- [ ] 主题演变、关注点位移（如「副业」相关讨论跨年变化）
- [ ] 自动标签建议（须用户确认）
- [ ] 向量检索 / Hybrid Search；本地 RAG（Organize 稳定后再评估）

### v1.0+ — 索引编排与生态（远期愿景）

若索引种类增多，可评估 `Indexer` 编排层。以下仅在本地闭环成熟后考虑：

- [ ] 插件化 Importer（`*.dll` / `*.wasm`）
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
