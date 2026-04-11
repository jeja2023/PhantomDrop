use regex::Regex;
use std::sync::LazyLock;

/**
 * 幻影中台 - AI 智能邮件解析器
 * 职责：从复杂的 HTML/文本邮件中精准提取验证码、链接或操作指令
 * 增强：OpenAI 注册邮件的精准 OTP 提取能力
 */

// 预热正则表达式，加速提取
static KEYWORD_NUMERIC_CODE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:verification\s*code|code|otp|验证码)\s*(?::|is|为)?\s*([0-9]{4,8})\b")
        .expect("验证码关键字数字正则初始化失败")
});

static KEYWORD_ALNUM_CODE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:verification\s*code|otp|验证码)\s*(?::|is|为)?\s*([A-Z0-9]{4,8})\b")
        .expect("验证码关键字字母数字正则初始化失败")
});

static FALLBACK_CODE_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b([0-9]{4,8})\b").expect("验证码兜底正则初始化失败"));

// OpenAI 专用：精准匹配独立的 6 位数字验证码
static OPENAI_OTP_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?<![0-9])([0-9]{6})(?![0-9])").expect("OpenAI OTP 正则初始化失败")
});

static LINK_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:link|url|点击链接|打开网址)[:\s]*((?:https?://|www\.)[^\s<]+)")
        .expect("正则表达式初始化失败")
});

static RAW_LINK_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)((?:https?://|www\.)[^\s"'<>]+)"#).expect("链接兜底正则初始化失败")
});

static HTML_TAG_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)<script[^>]*>.*?</script>|<style[^>]*>.*?</style>|<[^>]+>")
        .expect("HTML 标签正则初始化失败")
});

static WHITESPACE_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s+").expect("空白字符正则初始化失败"));

pub struct ParsedContent {
    pub code: Option<String>,
    pub link: Option<String>,
    pub custom_text: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseDepth {
    FullDeepScan,
    HeadersOnly,
    RawTextOnly,
}

pub struct NeuralParser;

impl NeuralParser {
    /// 全面解析邮件正文，提取验证码、重要链接以及按需提取定制文案
    pub fn parse_all(text: &str, html: &str, depth: ParseDepth) -> ParsedContent {
        if depth == ParseDepth::HeadersOnly {
            return ParsedContent {
                code: None,
                link: None,
                custom_text: None,
            };
        }

        let normalized_html = if depth == ParseDepth::RawTextOnly {
            String::new()
        } else {
            Self::html_to_text(html)
        };
        let merged_text = Self::merge_sources(text, &normalized_html);

        let code = KEYWORD_NUMERIC_CODE_REGEX
            .captures(&merged_text)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().to_string())
            .or_else(|| {
                KEYWORD_ALNUM_CODE_REGEX
                    .captures(&merged_text)
                    .and_then(|caps| caps.get(1))
                    .map(|m| m.as_str().to_string())
            })
            .or_else(|| {
                FALLBACK_CODE_REGEX
                    .captures(&merged_text)
                    .and_then(|caps| caps.get(1))
                    .map(|m| m.as_str().to_string())
            });

        let link = LINK_REGEX
            .captures(&merged_text)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().to_string())
            .or_else(|| {
                (depth == ParseDepth::FullDeepScan)
                    .then_some(())
                    .and_then(|_| RAW_LINK_REGEX.captures(html))
                    .and_then(|caps| caps.get(1))
                    .map(|m| m.as_str().to_string())
            })
            .or_else(|| {
                RAW_LINK_REGEX
                    .captures(&merged_text)
                    .and_then(|caps| caps.get(1))
                    .map(|m| m.as_str().to_string())
            });

        // 示例定制文案提取（如果是特定的业务邮件结构可以进一步增强）
        let custom_text = if merged_text.contains("重要通知") {
            Some("发现高优级别通知".to_string())
        } else {
            None
        };

        ParsedContent {
            code,
            link,
            custom_text,
        }
    }

    fn merge_sources(text: &str, html_text: &str) -> String {
        let trimmed_text = text.trim();

        match (trimmed_text.is_empty(), html_text.is_empty()) {
            (false, false) => format!("{trimmed_text}\n{html_text}"),
            (false, true) => trimmed_text.to_string(),
            (true, false) => html_text.to_string(),
            (true, true) => String::new(),
        }
    }

    fn html_to_text(html: &str) -> String {
        if html.trim().is_empty() {
            return String::new();
        }

        let without_tags = HTML_TAG_REGEX.replace_all(html, " ");
        let decoded = without_tags
            .replace("&nbsp;", " ")
            .replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&#39;", "'");

        WHITESPACE_REGEX
            .replace_all(decoded.trim(), " ")
            .to_string()
    }

    /// 判断发件人是否来自 OpenAI
    pub fn is_openai_sender(from: &str) -> bool {
        let lower = from.to_lowercase();
        lower.contains("@openai.com")
            || lower.contains("@email.openai.com")
            || lower.contains("noreply@tm.openai.com")
    }

    /// OpenAI 专用 OTP 提取：仅从 OpenAI 发件的邮件中精准提取 6 位验证码
    pub fn extract_openai_otp(text: &str, html: &str) -> Option<String> {
        let normalized_html = Self::html_to_text(html);
        let merged = Self::merge_sources(text, &normalized_html);

        OPENAI_OTP_REGEX
            .captures(&merged)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().to_string())
    }
}

impl ParseDepth {
    pub fn from_setting(value: Option<&str>) -> Self {
        match value.unwrap_or("").trim() {
            "头部解析 / HEADERS_ONLY" => ParseDepth::HeadersOnly,
            "原始文本 / RAW_TEXT_ONLY" => ParseDepth::RawTextOnly,
            _ => ParseDepth::FullDeepScan,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{NeuralParser, ParseDepth};

    #[test]
    fn extracts_code_from_plain_text() {
        let parsed = NeuralParser::parse_all("您的验证码: 123456", "", ParseDepth::FullDeepScan);
        assert_eq!(parsed.code.as_deref(), Some("123456"));
    }

    #[test]
    fn does_not_capture_keyword_word_as_code() {
        let parsed = NeuralParser::parse_all(
            "Your verification code is 123456. Open link: https://example.com/verify?id=abc123",
            "",
            ParseDepth::FullDeepScan,
        );
        assert_eq!(parsed.code.as_deref(), Some("123456"));
    }

    #[test]
    fn extracts_code_from_html_when_text_is_empty() {
        let html = "<html><body><p>验证码 <strong>654321</strong></p></body></html>";
        let parsed = NeuralParser::parse_all("", html, ParseDepth::FullDeepScan);
        assert_eq!(parsed.code.as_deref(), Some("654321"));
    }

    #[test]
    fn extracts_link_from_html() {
        let html = r#"<a href="https://example.com/verify?token=abc">点击验证</a>"#;
        let parsed = NeuralParser::parse_all("", html, ParseDepth::FullDeepScan);
        assert_eq!(
            parsed.link.as_deref(),
            Some("https://example.com/verify?token=abc")
        );
    }

    #[test]
    fn raw_text_only_skips_html_extraction() {
        let html = "<html><body><p>验证码 <strong>654321</strong></p></body></html>";
        let parsed = NeuralParser::parse_all("", html, ParseDepth::RawTextOnly);
        assert_eq!(parsed.code, None);
    }

    #[test]
    fn headers_only_skips_body_parsing() {
        let parsed = NeuralParser::parse_all("您的验证码: 123456", "", ParseDepth::HeadersOnly);
        assert_eq!(parsed.code, None);
        assert_eq!(parsed.link, None);
    }
}
