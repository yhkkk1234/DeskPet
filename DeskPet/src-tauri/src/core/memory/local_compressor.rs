use serde::{Deserialize, Serialize};

/// 本地压缩的结果，替代设计文档中的 CompressedMessage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressedMessage {
    pub summary: String,
    pub entities: Vec<String>,
    pub sentiment: f64,
}

/// 中文常见填充词 / 语气词，逐条匹配去除
const FILLER_WORDS: &[&str] = &[
    "那个", "这个", "嗯", "啊", "呃", "哦", "嘛", "吧", "呢", "呀", "哈",
    "就是说", "然后", "就是", "反正", "怎么说呢", "那个啥", "其实",
    "你知道的", "你懂的", "说实话", "说起来", "我觉得吧",
];

/// 常见中文正面情感词（含）— 出现即 +倾向
const POSITIVE_WORDS: &[&str] = &[
    "喜欢", "开心", "高兴", "爱", "感谢", "谢谢", "棒", "好", "赞",
    "厉害", "佩服", "感动", "温暖", "幸福", "惊喜", "期待", "好看",
    "有趣", "可爱", "帅", "美", "精彩", "成功", "加油", "支持",
    "哈哈哈", "哈哈", "嘻嘻", "嘿嘿", "耶",
];

/// 常见中文负面情感词
const NEGATIVE_WORDS: &[&str] = &[
    "讨厌", "烦", "生气", "难过", "伤心", "失望", "无聊", "累", "困",
    "痛苦", "焦虑", "担心", "害怕", "讨厌", "糟糕", "垃圾", "失败",
    "受不了", "无语", "呵呵", "唉", "真是的", "醉了", "服了",
];

/// 提取中文实体（人名/地名/时间/数字）的简化正则
/// 人名：常见姓氏 + 常见名尾字
const SURNAMES: &[&str] = &[
    "王", "李", "张", "刘", "陈", "杨", "赵", "黄", "周", "吴",
    "徐", "孙", "胡", "朱", "高", "林", "何", "郭", "马", "罗",
];

/// 时间词匹配模式
const TIME_PATTERNS: &[(&str, &str)] = &[
    ("今天", "今天"), ("明天", "明天"), ("昨天", "昨天"),
    ("早上", "早上"), ("下午", "下午"), ("晚上", "晚上"),
    ("周一", "周一"), ("周二", "周二"), ("周三", "周三"),
    ("周四", "周四"), ("周五", "周五"), ("周六", "周六"), ("周日", "周日"),
    ("星期一", "星期一"), ("星期二", "星期二"), ("星期三", "星期三"),
    ("星期四", "星期四"), ("星期五", "星期五"), ("星期六", "星期六"), ("星期日", "星期日"),
    ("上周", "上周"), ("下周", "下周"), ("本月", "本月"), ("上月", "上月"),
];

/// 对原始用户消息执行零 API 本地压缩
pub fn local_compress_message(raw: &str) -> CompressedMessage {
    let cleaned = remove_fillers(raw);
    let sentiment = analyze_sentiment_local(&cleaned);
    let entities = extract_entities(&cleaned);
    let summary = extract_key_points(&cleaned);

    CompressedMessage {
        summary,
        entities,
        sentiment,
    }
}

/// 1. 去除填充词和语气词
fn remove_fillers(text: &str) -> String {
    let mut result = text.to_string();
    for filler in FILLER_WORDS {
        result = result.replace(filler, "");
    }
    // 合并连续空白
    let mut cleaned = String::with_capacity(result.len());
    let mut prev_was_space = false;
    for ch in result.chars() {
        if ch.is_whitespace() || ch == '\u{3000}' {
            if !prev_was_space {
                cleaned.push(' ');
                prev_was_space = true;
            }
        } else {
            cleaned.push(ch);
            prev_was_space = false;
        }
    }
    cleaned.trim().to_string()
}

/// 2. 提取关键实体（人名、地名、时间、数字）
fn extract_entities(text: &str) -> Vec<String> {
    let mut entities = Vec::new();

    // 时间词
    for (pat, label) in TIME_PATTERNS {
        if text.contains(pat) {
            entities.push(label.to_string());
        }
    }

    // 提取数字（连续数字串）
    let mut num_buf = String::new();
    for ch in text.chars() {
        if ch.is_ascii_digit() {
            num_buf.push(ch);
        } else {
            if !num_buf.is_empty() {
                entities.push(format!("数字:{}", num_buf));
                num_buf.clear();
            }
        }
    }
    if !num_buf.is_empty() {
        entities.push(format!("数字:{}", num_buf));
    }

    // 简单人名识别（姓氏 + 1-2 个字）
    let chars: Vec<char> = text.chars().collect();
    let mut j = 0;
    while j < chars.len() {
        let ch = chars[j];
        if SURNAMES.contains(&&*ch.to_string()) {
            if j + 1 < chars.len() {
                let next = chars[j + 1];
                let next_str = next.to_string();
                // 排除明显不是名字尾字的字符
                if !FILLER_WORDS.iter().any(|f| f.starts_with(&*next_str))
                    && next != '的' && next != '了' && next != '是' && next != '在'
                    && next != '不' && next != '很' && next != '都' && next != '也'
                    && next != '就' && next != '和' && next != '与'
                    && !next.is_ascii_digit()
                {
                    let name_len = if j + 2 < chars.len() {
                        let third = chars[j + 2];
                        let third_str = third.to_string();
                        if third != '的' && third != '了' && third != '是' && third != '在'
                            && third != '不' && third != '很' && third != '都' && third != '也'
                            && third != '就' && third != '和' && third != '与'
                            && !third.is_ascii_digit()
                            && !FILLER_WORDS.iter().any(|f| f.starts_with(&*third_str))
                        {
                            3
                        } else {
                            2
                        }
                    } else {
                        2
                    };
                    let name: String = chars[j..j + name_len].iter().collect();
                    if name.chars().count() >= 2 {
                        entities.push(format!("人名:{}", name));
                    }
                }
            }
        }
        j += 1;
    }

    entities.dedup();
    entities
}

/// 3. 保留情感倾向标记（-1.0 ~ +1.0）
fn analyze_sentiment_local(text: &str) -> f64 {
    let mut score: f64 = 0.0;
    for pw in POSITIVE_WORDS {
        if text.contains(pw) {
            score += 0.15;
        }
    }
    for nw in NEGATIVE_WORDS {
        if text.contains(nw) {
            score -= 0.15;
        }
    }
    score.clamp(-1.0, 1.0)
}

/// 提取要点摘要：去填充词后取前 200 字符
fn extract_key_points(text: &str) -> String {
    let cleaned = remove_fillers(text);
    let count = cleaned.chars().count();
    if count <= 200 {
        cleaned
    } else {
        let truncated: String = cleaned.chars().take(200).collect();
        format!("{}…", truncated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_fillers() {
        let result = remove_fillers("嗯我觉得吧这个电影真的很好看呢");
        assert!(!result.contains("嗯"));
        assert!(!result.contains("吧"));
        assert!(result.contains("好看"));
    }

    #[test]
    fn test_extract_entities() {
        let result = extract_entities("今天下午张三买了3本书");
        assert!(result.iter().any(|e| e == "今天"));
        assert!(result.iter().any(|e| e == "下午"));
        assert!(result.iter().any(|e| e.contains("张三")));
        assert!(result.iter().any(|e| e == "数字:3"));
    }

    #[test]
    fn test_sentiment_positive() {
        let result = analyze_sentiment_local("好开心啊 谢谢 太棒了");
        assert!(result > 0.0);
    }

    #[test]
    fn test_sentiment_negative() {
        let result = analyze_sentiment_local("好烦 无聊 生气");
        assert!(result < 0.0);
    }

    #[test]
    fn test_full_compress() {
        let result = local_compress_message("嗯那个我今天下午跟张三去看了一部电影，真的很好看呢！");
        assert!(!result.summary.contains("嗯"));
        assert!(!result.summary.contains("那个"));
        assert!(result.entities.iter().any(|e| e == "下午" || e == "今天"));
        assert!(result.sentiment > 0.0);
    }
}
