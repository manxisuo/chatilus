export interface TopicBubble {
  label: string;
  count: number;
  kind: "tag" | "keyword";
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
]);

const TITLE_TOKEN_PATTERN = /[\u4e00-\u9fff]{2,}|[a-zA-Z][a-zA-Z0-9_-]{1,}/g;

export function extractTitleKeywords(title: string): string[] {
  const matches = title.match(TITLE_TOKEN_PATTERN) ?? [];
  const unique = new Set<string>();

  for (const raw of matches) {
    const token = raw.trim();
    if (token.length < 2) continue;
    const key = token.toLowerCase();
    if (STOPWORDS.has(key) || STOPWORDS.has(token)) continue;
    unique.add(token);
  }

  return [...unique];
}

export function buildMonthTopics(
  conversations: Array<{ title: string; tags: string[] }>,
  limit = 10,
): TopicBubble[] {
  const tagCounts = new Map<string, number>();
  const keywordCounts = new Map<string, number>();

  for (const conversation of conversations) {
    for (const tag of new Set(conversation.tags)) {
      const label = tag.trim();
      if (!label) continue;
      tagCounts.set(label, (tagCounts.get(label) ?? 0) + 1);
    }

    for (const keyword of extractTitleKeywords(conversation.title)) {
      keywordCounts.set(keyword, (keywordCounts.get(keyword) ?? 0) + 1);
    }
  }

  const topics: TopicBubble[] = [];
  for (const [label, count] of tagCounts) {
    topics.push({ label, count, kind: "tag" });
  }
  for (const [label, count] of keywordCounts) {
    if (tagCounts.has(label)) continue;
    topics.push({ label, count, kind: "keyword" });
  }

  return topics
    .filter((topic) => topic.count > 0)
    .sort((left, right) => right.count - left.count || left.label.localeCompare(right.label))
    .slice(0, limit);
}
