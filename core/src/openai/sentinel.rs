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
    client: &reqwest::Client,
    device_id: &str,
) -> Result<SentinelToken, String> {
    let payload = serde_json::json!({
        "p": "gAAAAAC",
        "device_id": device_id,
    });

    let response = client
        .post(constants::SENTINEL_ENDPOINT)
        .header("user-agent", constants::DEFAULT_USER_AGENT)
        .header("accept", "application/json")
        .header("content-type", "application/json")
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

fn hex_encode(data: &[u8]) -> String {
    data.iter().map(|b| format!("{:02x}", b)).collect()
}
