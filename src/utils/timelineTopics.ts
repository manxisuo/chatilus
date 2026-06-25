export interface TopicSourceCount {
  source: string;
  count: number;
}

export interface TopicBubble {
  label: string;
  count: number;
  kind: "tag" | "keyword";
  sourceCounts: TopicSourceCount[];
}

export interface TopicFilter {
  label: string;
  kind: TopicBubble["kind"];
}

const STOPWORDS = new Set([
  "the",
  "and",
  "for",
  "with",
  "from",
  "this",
  "that",
  "what",
  "how",
  "why",
  "when",
  "where",
  "can",
  "you",
  "your",
  "please",
  "help",
  "about",
  "using",
  "use",
  "into",
  "via",
  "chatgpt",
  "cursor",
  "codex",
  "gemini",
  "claude",
  "的",
  "了",
  "在",
  "是",
  "我",
  "有",
  "和",
  "就",
  "不",
  "人",
  "都",
  "一",
  "一个",
  "上",
  "也",
  "很",
  "到",
  "说",
  "要",
  "去",
  "你",
  "会",
  "着",
  "没有",
  "看",
  "好",
  "自己",
  "这",
  "那",
  "吗",
  "呢",
  "吧",
  "啊",
  "与",
  "及",
  "等",
  "为",
  "对",
  "中",
  "请",
  "如何",
  "怎么",
  "什么",
  "为什么",
  "可以",
  "问题",
  "帮忙",
  "关于",
  "使用",
  "生成",
  "帮我",
  "看一下",
  "主题要",
  "主题",
  "一下",
  "这个",
  "那个",
  "能否",
  "是否",
  "告诉",
  "适合",
  "一篇",
  "需要",
  "想要",
  "东西",
  "内容",
  "方法",
  "方案",
  "情况",
  "意思",
  "名字",
  "文件",
  "程序",
  "功能",
  "版本",
  "模式",
  "deepseek",
]);

/** 纯中文标题片段超过此长度时视为整句碎片，不作为话题。 */
const MAX_CJK_TOKEN_LENGTH = 8;

/** 常见自动标题 / 指令前缀，匹配则丢弃该片段。 */
const LOW_QUALITY_PREFIXES = [
  "请生成",
  "请写",
  "请帮",
  "请帮我",
  "帮我看",
  "帮我",
  "看一下",
  "生成一",
  "写一篇",
  "一篇适",
  "能否帮",
  "是否可以",
  "麻烦",
];

const TITLE_TOKEN_PATTERN = /[\u4e00-\u9fff]{2,}|[a-zA-Z][a-zA-Z0-9_-]{1,}/g;

type TopicKind = TopicBubble["kind"];
type TopicTallyKey = `${TopicKind}:${string}`;

interface TopicTally {
  label: string;
  kind: TopicKind;
  sources: Map<string, number>;
}

export function topicFilterKey(filter: TopicFilter): string {
  return `${filter.kind}:${filter.label}`;
}

export function isSameTopicFilter(
  left: TopicFilter | null | undefined,
  right: TopicFilter | null | undefined,
): boolean {
  if (!left || !right) return false;
  return left.kind === right.kind && left.label === right.label;
}

function isLowQualityKeyword(token: string): boolean {
  const key = token.toLowerCase();
  if (STOPWORDS.has(key) || STOPWORDS.has(token)) {
    return true;
  }

  if (/^[\u4e00-\u9fff]+$/.test(token) && token.length > MAX_CJK_TOKEN_LENGTH) {
    return true;
  }

  if (LOW_QUALITY_PREFIXES.some((prefix) => token.startsWith(prefix))) {
    return true;
  }

  // 「小学高年级的」这类标题切片
  if (/^[\u4e00-\u9fff]+的$/.test(token) && token.length >= 4) {
    return true;
  }

  return false;
}

export function extractTitleKeywords(title: string): string[] {
  const matches = title.match(TITLE_TOKEN_PATTERN) ?? [];
  const unique = new Set<string>();

  for (const raw of matches) {
    const token = raw.trim();
    if (token.length < 2) continue;
    if (isLowQualityKeyword(token)) continue;
    unique.add(token);
  }

  return [...unique];
}

export function conversationMatchesTopic(
  conversation: { title: string; tags: string[] },
  filter: TopicFilter,
): boolean {
  if (filter.kind === "tag") {
    return conversation.tags.some((tag) => tag.trim() === filter.label);
  }

  const target = filter.label.toLowerCase();
  return extractTitleKeywords(conversation.title).some(
    (keyword) => keyword.toLowerCase() === target,
  );
}

export function filterConversationsByTopic<T extends { title: string; tags: string[] }>(
  conversations: T[],
  filter: TopicFilter | null,
): T[] {
  if (!filter) return conversations;
  return conversations.filter((conversation) => conversationMatchesTopic(conversation, filter));
}

function conversationSource(conversation: { source?: string | null }): string {
  const source = conversation.source?.trim();
  return source && source.length > 0 ? source : "chatgpt";
}

export function sourceCountsFromConversations(
  conversations: Array<{ source?: string | null }>,
): TopicSourceCount[] {
  const tallies = new Map<string, number>();
  for (const conversation of conversations) {
    const source = conversationSource(conversation);
    tallies.set(source, (tallies.get(source) ?? 0) + 1);
  }
  return [...tallies.entries()]
    .map(([source, count]) => ({ source, count }))
    .sort((left, right) => right.count - left.count || left.source.localeCompare(right.source));
}

export function formatTopicSourceBreakdown(
  sourceCounts: TopicSourceCount[],
  label: (source: string) => string,
  separator: string,
): string {
  return sourceCounts.map((item) => `${label(item.source)} ${item.count}`).join(separator);
}

function recordTopicHit(
  tallies: Map<TopicTallyKey, TopicTally>,
  kind: TopicKind,
  label: string,
  source: string,
) {
  const key = `${kind}:${label}` as TopicTallyKey;
  const tally = tallies.get(key) ?? { label, kind, sources: new Map<string, number>() };
  tally.sources.set(source, (tally.sources.get(source) ?? 0) + 1);
  tallies.set(key, tally);
}

function tallyToBubble(tally: TopicTally): TopicBubble {
  const sourceCounts = [...tally.sources.entries()]
    .map(([source, count]) => ({ source, count }))
    .sort((left, right) => right.count - left.count || left.source.localeCompare(right.source));
  const count = sourceCounts.reduce((sum, item) => sum + item.count, 0);
  return {
    label: tally.label,
    kind: tally.kind,
    count,
    sourceCounts,
  };
}

export function buildMonthTopics(
  conversations: Array<{ title: string; tags: string[]; source?: string | null }>,
  limit = 10,
): TopicBubble[] {
  const tallies = new Map<TopicTallyKey, TopicTally>();
  const tagLabels = new Set<string>();

  for (const conversation of conversations) {
    const source = conversationSource(conversation);

    for (const tag of new Set(conversation.tags.map((item) => item.trim()).filter(Boolean))) {
      tagLabels.add(tag);
      recordTopicHit(tallies, "tag", tag, source);
    }

    for (const keyword of extractTitleKeywords(conversation.title)) {
      if (tagLabels.has(keyword)) continue;
      recordTopicHit(tallies, "keyword", keyword, source);
    }
  }

  return [...tallies.values()]
    .map(tallyToBubble)
    .filter((topic) => topic.count > 0)
    .sort((left, right) => right.count - left.count || left.label.localeCompare(right.label))
    .slice(0, limit);
}
