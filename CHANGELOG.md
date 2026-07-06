# Changelog

本文件记录 ChatLens 的版本变更。格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/)。

## [0.2.0] - 2026-07-07

### 新增

- 导入完成结构化报告对话框（统计摘要、「开始搜索」「打开 Timeline」）
- 应用内「隐私与数据」说明页（本地存储、无 telemetry、删除方式）
- 工具栏「反馈问题」入口（GitHub Issues）
- 图库图片类型筛选：全部 / 仅生成 / 仅上传
- 图库灯箱「下一张」在末尾可触发加载更多
- 窄窗口下对话信息 Drawer 入口
- 里程碑 A 发布文档：`CHANGELOG`、`KNOWN_ISSUES`、Issue 模板

### 改进

- 导入数据源成熟度标签：Stable / Beta / Experimental
- 消息区 Markdown 字体层级与侧栏密度统一
- 图片灯箱左右切换按钮固定于屏幕两侧
- ChatGPT 会话时间戳解析与归一化（修复导出元数据 stale / 毫秒异常）

### 数据源支持状态（0.2.0）

| 来源 | 状态 |
|------|------|
| ChatGPT | Stable |
| Cursor | Beta |
| Gemini | Beta |
| Codex / Copilot / Grok / DeepSeek | Experimental |

## [0.1.0] - 2026-06 及更早

### 新增

- 多源 AI 对话导入（ChatGPT、Cursor、Gemini、Codex 等）
- 会话列表、详情、全文搜索（FTS5）
- 图片图库、Timeline 月度回顾
- 收藏对话/消息、标签、Markdown 导出
- 浅色/深色/跟随系统主题，中英文界面

[0.2.0]: https://github.com/manxisuo/ChatLens/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/manxisuo/ChatLens/releases/tag/v0.1.0
