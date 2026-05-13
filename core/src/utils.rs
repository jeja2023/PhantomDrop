/// 对敏感 URL（如包含用户名密码的代理地址）进行脱敏处理
pub fn mask_url(url: &str) -> String {
    if let Ok(mut parsed) = url::Url::parse(url) {
        if parsed.username() != "" || parsed.password().is_some() {
            let _ = parsed.set_username("***");
            let _ = parsed.set_password(Some("***"));
            return parsed.to_string();
        }
    }
    // 降级正则处理：针对某些非标准 URL 或解析失败的情况
    if let Ok(re) = regex::Regex::new(r"([^:/]+://)([^:/]+):([^@/]+)@") {
        return re.replace(url, "$1***:***@").to_string();
    }
    url.to_string()
}

/// 对日志消息进行通用的脱敏处理，过滤 Token、密码、代理凭据等敏感信息
pub fn redact_log_message(message: &str) -> String {
    let mut output = message.to_string();
    let patterns = [
        (
            r"(?i)(password|密码|密令)\s*[:：]\s*[^\s,，|]+",
            "$1: ******",
        ),
        (
            r"(?i)(access_token|refresh_token|session_token|id_token|api[_-]?key|secret|token)\s*[:：=]\s*[^\s,，|]+",
            "$1: ******",
        ),
        (r"eyJ[A-Za-z0-9_\-\.]{20,}", "eyJ***"),
        (r"sess_[A-Za-z0-9_\-]{8,}", "sess_***"),
    ];

    for (pattern, replacement) in patterns {
        if let Ok(regex) = regex::Regex::new(pattern) {
            output = regex.replace_all(&output, replacement).to_string();
        }
    }

    // 处理代理 URL 脱敏
    if output.contains("://") && output.contains('@') {
        if let Ok(re) = regex::Regex::new(r"([^:/]+://)([^:/]+):([^@/]+)@") {
            output = re.replace_all(&output, "$1***:***@").to_string();
        }
    }

    output
}
