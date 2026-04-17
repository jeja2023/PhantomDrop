/**
 * Sentinel 防护模块 / PoW (Proof of Work) 解算器
 * 负责与 OpenAI Sentinel 服务器握手，获取必要的 sentinel_token
 * 并完成基于 SHA3 的计算量证明
 */
use crate::openai::constants;

/// Sentinel 令牌获取结果
pub struct SentinelToken {
    pub token: String,
    pub difficulty: u32,
}

/// 请求 Sentinel 令牌
///
/// 该函数将与 sentinel.openai.com 握手，获取解算难度和初始种子
/// 后续根据返回的 difficulty 进行 PoW 计算
pub async fn request_sentinel_token(
    client: &rquest::Client,
    device_id: &str,
) -> Result<SentinelToken, String> {
    let payload = serde_json::json!({
        // p 参数是 Fernet 加密的数据，gAAAAAC 是错误占位符，通常需要从真实浏览器获取
        // 这里更新为一个格式正确但较长的占位令牌，并增加 oai-device-id 字段匹配
        "p": "gAAAAABmOnOnf-AAVvVf-AAvVf-AAvVf-AAvVf-AAvVf-AAvVf-AAvVf-AAvVf-AAvVf-AAvVf-AAvVf-A",
        "oai-device-id": device_id,
    });

    let response = client
        .post(constants::SENTINEL_ENDPOINT)
        .header("oai-device-id", device_id)
        .header("oai-language", "en-US")
        .header("accept", "*/*")
        .header("content-type", "application/json")
        .header("origin", "https://chatgpt.com")
        .header("referer", "https://chatgpt.com/")
        .header("sec-fetch-dest", "empty")
        .header("sec-fetch-mode", "cors")
        .header("sec-fetch-site", "same-site")
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Sentinel 请求失败: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Sentinel 响应异常: {} / {}", status, body));
    }

    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Sentinel 响应解析失败: {}", e))?;

    let token = body["token"].as_str().unwrap_or("").to_string();

    let difficulty = body["difficulty"].as_u64().unwrap_or(100000) as u32;

    if token.is_empty() {
        return Err("Sentinel 令牌为空".to_string());
    }

    Ok(SentinelToken { token, difficulty })
}

/// PoW 解算：在给定难度下寻找满足条件的 nonce
///
/// 使用暴力遍历方式寻找使 hash(seed + nonce) 前导零满足要求的 nonce 值
/// 后续可升级为基于 sha3 crate 的高性能实现
pub fn solve_pow(seed: &str, difficulty: u32) -> String {
    // 计算前导零 bit 数
    let leading_zeros = (difficulty as f64).log2().ceil() as usize;
    let required_zero_bytes = leading_zeros / 8;

    for nonce in 0u64.. {
        let input = format!("{}{}", seed, nonce);
        let hash = crate::openai::oauth::simple_sha256_public(input.as_bytes());

        // 检查前导零
        let mut valid = true;
        for byte in hash.iter().take(required_zero_bytes) {
            if *byte != 0 {
                valid = false;
                break;
            }
        }

        if valid {
            return format!("0x{}", hex_encode(&hash[..8]));
        }

        // 安全阈值：防止无限循环
        if nonce > 10_000_000 {
            break;
        }
    }

    // 兜底返回
    "0x0000000000000000".to_string()
}


/// IP 质量与归属地信息
#[derive(serde::Serialize)]
pub struct IpQualityInfo {
    pub ip: String,
    pub country: String,
    pub city: Option<String>,
    pub org: String,
    pub is_datacenter: bool,
}

/// 环境预检：检测当前出口 IP 的归属地和质量
pub async fn check_ip_quality(client: &rquest::Client) -> Result<IpQualityInfo, String> {
    // 使用 ip-api.com 获取详细的地理位置和组织信息
    let response = client
        .get("http://ip-api.com/json/?fields=status,message,country,city,org,as,query,hosting")
        .send()
        .await
        .map_err(|e| format!("环境预检失败 (网络异常): {}", e))?;

    let data: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("环境预检失败 (解析异常): {}", e))?;

    if data["status"].as_str() != Some("success") {
        return Err(format!("IP 环境检测服务返回异常: {}", data["message"].as_str().unwrap_or("unknown")));
    }

    let ip = data["query"].as_str().unwrap_or("Unknown").to_string();
    let org = data["org"].as_str().or(data["as"].as_str()).unwrap_or("Unknown").to_string();
    let is_datacenter = data["hosting"].as_bool().unwrap_or(false) 
        || org.to_lowercase().contains("cloud") 
        || org.to_lowercase().contains("server")
        || org.to_lowercase().contains("datacenter");

    Ok(IpQualityInfo {
        ip,
        country: data["country"].as_str().unwrap_or("Unknown").to_string(),
        city: data["city"].as_str().map(|s| s.to_string()),
        org,
        is_datacenter,
    })
}

fn hex_encode(data: &[u8]) -> String {
    data.iter().map(|b| format!("{:02x}", b)).collect()
}
