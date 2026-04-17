/**
 * OpenAI OAuth/PKCE 工具模块
 * 实现 PKCE (Proof Key for Code Exchange) 的 S256 模式，
 * 用于 OAuth 2.0 授权码流程中的安全增强
 */
use rand::Rng;

/// PKCE 参数对
pub struct PkceParams {
    pub code_verifier: String,
    pub code_challenge: String,
}

/// 生成 PKCE code_verifier（43-128 字符的随机 URL-safe 字符串）
fn generate_code_verifier() -> String {
    let mut rng = rand::thread_rng();
    let length = rng.gen_range(43..=128);
    let charset = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
    (0..length)
        .map(|_| {
            let idx = rng.gen_range(0..charset.len());
            charset[idx] as char
        })
        .collect()
}

/// 根据 code_verifier 生成 S256 code_challenge
/// code_challenge = BASE64URL(SHA256(code_verifier))
fn generate_code_challenge(verifier: &str) -> String {
    let hash = simple_sha256(verifier.as_bytes());
    base64url_encode(&hash)
}

/// 生成完整的 PKCE 参数对
pub fn generate_pkce() -> PkceParams {
    let code_verifier = generate_code_verifier();
    let code_challenge = generate_code_challenge(&code_verifier);
    PkceParams {
        code_verifier,
        code_challenge,
    }
}

/// 生成随机状态参数（防 CSRF）
pub fn generate_state() -> String {
    let mut rng = rand::thread_rng();
    let charset = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    (0..32)
        .map(|_| {
            let idx = rng.gen_range(0..charset.len());
            charset[idx] as char
        })
        .collect()
}

/// 生成设备指纹 ID（UUID v4 格式）
pub fn generate_device_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct CodexAuthData {
    pub access_token: String,
    pub refresh_token: String,
    pub id_token: String,
    pub expires_in: u64,
    pub token_type: String,
}

impl CodexAuthData {
    pub fn get_email(&self) -> Option<String> {
        let parts: Vec<&str> = self.id_token.split('.').collect();
        if parts.len() >= 2 {
            use base64::Engine;
            if let Ok(decoded) = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(parts[1]) {
                if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&decoded) {
                    return json.get("email").and_then(|v| v.as_str()).map(|s| s.to_string());
                }
            }
        }
        None
    }
}

/// 将回调 URL 转换为授权码并进行令牌交换
pub async fn exchange_codex_code(
    callback_url: &str,
    code_verifier: &str,
) -> Result<CodexAuthData, String> {
    let url = url::Url::parse(callback_url).map_err(|e| format!("URL 解析失败: {}", e))?;
    let code = url
        .query_pairs()
        .find(|(k, _)| k == "code")
        .map(|(_, v)| v.to_string())
        .ok_or("回调 URL 中未找到授权码 (code)")?;

    let client = reqwest::Client::new();
    let params = [
        ("client_id", crate::openai::constants::OPENAI_CLIENT_ID),
        ("grant_type", "authorization_code"),
        ("code", &code),
        ("redirect_uri", "http://localhost:1455/auth/callback"),
        ("code_verifier", code_verifier),
    ];

    let response = client
        .post(crate::openai::constants::AUTH_TOKEN_URL)
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("令牌交换请求失败: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("令牌交换失败 ({}): {}", status, error_text));
    }

    let auth_data = response
        .json::<CodexAuthData>()
        .await
        .map_err(|e| format!("解析令牌响应失败: {}", e))?;

    Ok(auth_data)
}

// --- 辅助函数：Base64 URL 编码（无 padding） ---

fn base64url_encode(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut result = String::with_capacity((data.len() * 4 + 2) / 3);
    let chunks = data.chunks(3);
    for chunk in chunks {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let n = (b0 << 16) | (b1 << 8) | b2;
        result.push(TABLE[((n >> 18) & 0x3f) as usize] as char);
        result.push(TABLE[((n >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            result.push(TABLE[((n >> 6) & 0x3f) as usize] as char);
        }
        if chunk.len() > 2 {
            result.push(TABLE[(n & 0x3f) as usize] as char);
        }
    }
    result
}

// --- 辅助函数：简易 SHA-256 实现 ---

/// 供其他模块调用的 SHA-256 公开入口
pub fn simple_sha256_public(data: &[u8]) -> [u8; 32] {
    simple_sha256(data)
}

fn simple_sha256(data: &[u8]) -> [u8; 32] {
    // SHA-256 初始哈希值
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];

    // SHA-256 轮常量
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    // 消息预处理：填充
    let bit_len = (data.len() as u64) * 8;
    let mut msg = data.to_vec();
    msg.push(0x80);
    while (msg.len() % 64) != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    // 逐块处理
    for block in msg.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                block[i * 4],
                block[i * 4 + 1],
                block[i * 4 + 2],
                block[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }

    let mut result = [0u8; 32];
    for i in 0..8 {
        let bytes = h[i].to_be_bytes();
        result[i * 4..i * 4 + 4].copy_from_slice(&bytes);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_generates_valid_params() {
        let params = generate_pkce();
        assert!(params.code_verifier.len() >= 43);
        assert!(params.code_verifier.len() <= 128);
        assert!(!params.code_challenge.is_empty());
        // code_challenge 不应包含 = + / 等传统 Base64 字符
        assert!(!params.code_challenge.contains('='));
        assert!(!params.code_challenge.contains('+'));
        assert!(!params.code_challenge.contains('/'));
    }

    #[test]
    fn state_is_correct_length() {
        let state = generate_state();
        assert_eq!(state.len(), 32);
    }

    #[test]
    fn sha256_known_vector() {
        // SHA-256("") = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        let hash = simple_sha256(b"");
        assert_eq!(hash[0], 0xe3);
        assert_eq!(hash[1], 0xb0);
        assert_eq!(hash[31], 0x55);
    }
}
